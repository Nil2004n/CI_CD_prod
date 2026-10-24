//! P00-T08 spike harness: restricted Markdown -> PDF with pulldown-cmark (no features) and
//! printpdf (no features). Spike code, not application code.
//!
//! `orchestrate` must run as PID 1 of a dedicated user+net+PID namespace. It checks the
//! environment and the allowlisted fonts, generates synthetic fixtures, runs each conversion in
//! its own `render` child process (killed after 60 s), and verifies every output with
//! `/usr/bin/mutool info`, `draw -F txt` and `clean -d` before accepting it.
//!
//! Deliberate limits of this spike: no font subsetting, no complex-script shaping, no bidi, no
//! bold/italic faces, no images, no links/annotations. Unsupported content is made visible.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use printpdf::{
    Color, FontId, Greyscale, Line, LinePoint, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle,
    PdfPage, PdfSaveOptions, Point, Pt, TextItem,
};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

// ---------------------------------------------------------------- limits (spike values, request §6)

const MAX_INPUT: u64 = 8 * 1024 * 1024;
const MAX_NESTING: usize = 16;
const MAX_LIST_DEPTH: usize = 3;
const MAX_TABLE_COLS: usize = 32;
const MAX_CELL: usize = 64 * 1024;
const MAX_PAGES: usize = 2000;
const MAX_OUTPUT: usize = 64 * 1024 * 1024;
const CONVERSION_KILL: Duration = Duration::from_secs(60);
const TOOL_KILL: Duration = Duration::from_secs(60);
const CAPTURE_CAP: u64 = 256 * 1024;

const MUTOOL: &str = "/usr/bin/mutool";
const SHA256SUM: &str = "/usr/bin/sha256sum";

/// Allowlisted fonts (request §4): proportional, CJK fallback, monospace.
const FONTS: [(&str, &str); 3] = [
    ("DejaVuSans", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
    ("DroidSansFallbackFull", "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf"),
    ("NotoSansMono-Regular", "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf"),
];
const F_MAIN: usize = 0;
const F_CJK: usize = 1;
const F_MONO: usize = 2;

// A4 portrait in points; margins.
const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 50.0;
const FOOTER: f32 = 20.0;

const RTL_MARKER: &str = "[complex script/RTL not shaped] ";

// ---------------------------------------------------------------- minimal JSON output

#[derive(Clone)]
enum J {
    S(String),
    B(bool),
    N(i64),
    F(f64),
    Null,
    A(Vec<J>),
    O(Vec<(String, J)>),
}

impl J {
    fn s(v: impl Into<String>) -> J {
        J::S(v.into())
    }
    fn render(&self, out: &mut String) {
        match self {
            J::S(s) => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        c if (c as u32) < 0x20 || c == '\u{7f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            J::B(b) => out.push_str(if *b { "true" } else { "false" }),
            J::N(n) => out.push_str(&n.to_string()),
            J::F(f) => out.push_str(&format!("{f:.3}")),
            J::Null => out.push_str("null"),
            J::A(v) => {
                out.push('[');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    x.render(out);
                }
                out.push(']');
            }
            J::O(v) => {
                out.push('{');
                for (i, (k, x)) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    J::S(k.clone()).render(out);
                    out.push(':');
                    x.render(out);
                }
                out.push('}');
            }
        }
    }
    fn dump(&self) -> String {
        let mut s = String::new();
        self.render(&mut s);
        s
    }
}

fn obj(pairs: Vec<(&str, J)>) -> J {
    J::O(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn die(msg: &str) -> ! {
    eprintln!("t08: {msg}");
    std::process::exit(2);
}

// ---------------------------------------------------------------- character policy

fn is_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x05FF | 0x0600..=0x06FF | 0x0700..=0x074F | 0x0750..=0x077F
        | 0x08A0..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F)
}

/// Control, format and bidi-override characters that must be shown, never applied.
fn is_control_like(c: char) -> bool {
    let u = c as u32;
    (u < 0x20) || (0x7F..=0x9F).contains(&u) || matches!(u, 0x200B..=0x200F | 0x202A..=0x202E
        | 0x2060..=0x2064 | 0x2066..=0x2069 | 0xFEFF | 0xFFF9..=0xFFFB)
}

fn codepoint_label(c: char) -> String {
    format!("<U+{:04X}>", c as u32)
}

// ---------------------------------------------------------------- fonts and metrics

struct Fonts {
    parsed: Vec<ParsedFont>,
}

impl Fonts {
    fn load(fault_font: bool) -> Result<Fonts, String> {
        let mut parsed = Vec::new();
        for (i, (name, path)) in FONTS.iter().enumerate() {
            let bytes = if fault_font && i == F_MAIN {
                b"not a font".to_vec() // F1a forced font-load failure
            } else {
                fs::read(path).map_err(|e| format!("font_unavailable:{name}:{e}"))?
            };
            let mut warnings = Vec::new();
            let f = ParsedFont::from_bytes(&bytes, 0, &mut warnings)
                .ok_or_else(|| format!("font_unavailable:{name}:parse_failed"))?;
            if f.codepoint_to_glyph.is_empty() {
                return Err(format!("font_unavailable:{name}:empty_cmap"));
            }
            parsed.push(f);
        }
        Ok(Fonts { parsed })
    }

    fn has(&self, font: usize, c: char) -> bool {
        self.parsed[font].lookup_glyph_index(c as u32).map(|g| g != 0).unwrap_or(false)
    }

    fn advance(&self, font: usize, c: char, size: f32) -> f32 {
        let f = &self.parsed[font];
        let upem = f.pdf_font_metrics.units_per_em.max(1) as f32;
        let w = f
            .lookup_glyph_index(c as u32)
            .and_then(|g| f.get_glyph_width(g))
            .map(|w| w as f32)
            .unwrap_or(upem / 2.0);
        w * size / upem
    }
}

#[derive(Default)]
struct Stats {
    chars_by_font: [u64; 3],
    missing: u64,
    missing_samples: BTreeSet<String>,
    emoji: u64,
    controls: u64,
    rtl_chars: u64,
    rtl_markers: u64,
    combining: u64,
    code_continuations: u64,
    words_broken: u64,
    table_header_repeats: u64,
    raw_html: u64,
    images_omitted: u64,
    links_as_text: u64,
    lines: u64,
    max_right_overflow: f32,
    min_baseline_margin: f32,
}

/// A glyph-resolved piece of text in one font.
#[derive(Clone)]
struct Seg {
    font: usize,
    text: String,
    width: f32,
}

/// Resolve a string into font segments, expanding unsupported characters to visible labels.
fn resolve(fonts: &Fonts, s: &str, mono: bool, size: f32, st: &mut Stats) -> Vec<(char, usize, f32)> {
    let order: &[usize] = if mono { &[F_MONO, F_MAIN, F_CJK] } else { &[F_MAIN, F_CJK] };
    let mut out = Vec::new();
    let push_label = |c: char, out: &mut Vec<(char, usize, f32)>| {
        for lc in codepoint_label(c).chars() {
            let f = if mono { F_MONO } else { F_MAIN };
            out.push((lc, f, fonts.advance(f, lc, size)));
        }
    };
    for c in s.chars() {
        if c == '\t' {
            for _ in 0..4 {
                let f = order[0];
                out.push((' ', f, fonts.advance(f, ' ', size)));
            }
            continue;
        }
        if is_control_like(c) {
            st.controls += 1;
            push_label(c, &mut out);
            continue;
        }
        if is_emoji(c) {
            st.emoji += 1;
            push_label(c, &mut out);
            continue;
        }
        if is_rtl(c) {
            st.rtl_chars += 1;
        }
        if is_combining(c) {
            st.combining += 1;
        }
        match order.iter().find(|&&f| fonts.has(f, c)) {
            Some(&f) => {
                st.chars_by_font[f] += 1;
                out.push((c, f, fonts.advance(f, c, size)));
            }
            None => {
                st.missing += 1;
                if st.missing_samples.len() < 20 {
                    st.missing_samples.insert(codepoint_label(c));
                }
                push_label(c, &mut out);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- page model

enum Item {
    Text { x: f32, y: f32, font: usize, size: f32, text: String },
    Rule { x1: f32, y1: f32, x2: f32, y2: f32, w: f32 },
}

struct Layout<'a> {
    fonts: &'a Fonts,
    pages: Vec<Vec<Item>>,
    y: f32,
    st: Stats,
}

#[derive(Debug)]
struct RenderError(String);

impl<'a> Layout<'a> {
    fn new(fonts: &'a Fonts) -> Self {
        let mut l = Layout { fonts, pages: vec![Vec::new()], y: PAGE_H - MARGIN, st: Stats::default() };
        l.st.min_baseline_margin = f32::MAX;
        l
    }
    fn bottom() -> f32 {
        MARGIN + FOOTER
    }
    fn right() -> f32 {
        PAGE_W - MARGIN
    }
    fn new_page(&mut self) -> Result<(), RenderError> {
        if self.pages.len() >= MAX_PAGES {
            return Err(RenderError("limit_exceeded:pages".into()));
        }
        self.pages.push(Vec::new());
        self.y = PAGE_H - MARGIN;
        Ok(())
    }
    fn ensure(&mut self, h: f32) -> Result<(), RenderError> {
        if self.y - h < Self::bottom() {
            self.new_page()?;
        }
        Ok(())
    }
    fn gap(&mut self, h: f32) {
        self.y -= h;
    }

    /// Emit one laid-out line of glyphs at the current position.
    fn emit_line(&mut self, x0: f32, glyphs: &[(char, usize, f32)], size: f32) {
        let mut segs: Vec<Seg> = Vec::new();
        for &(c, f, w) in glyphs {
            match segs.last_mut() {
                Some(s) if s.font == f => {
                    s.text.push(c);
                    s.width += w;
                }
                _ => segs.push(Seg { font: f, text: c.to_string(), width: w }),
            }
        }
        let mut x = x0;
        let page = self.pages.last_mut().unwrap();
        for s in segs {
            if !s.text.trim().is_empty() {
                page.push(Item::Text { x, y: self.y, font: s.font, size, text: s.text.clone() });
            }
            x += s.width;
        }
        let overflow = x - Self::right();
        if overflow > self.st.max_right_overflow {
            self.st.max_right_overflow = overflow;
        }
        let m = self.y - Self::bottom();
        if m < self.st.min_baseline_margin {
            self.st.min_baseline_margin = m;
        }
        self.st.lines += 1;
    }

    /// Greedy word wrap of resolved glyphs into lines no wider than `width`.
    fn wrap(&mut self, glyphs: Vec<(char, usize, f32)>, width: f32) -> Vec<Vec<(char, usize, f32)>> {
        let mut lines: Vec<Vec<(char, usize, f32)>> = Vec::new();
        let mut line: Vec<(char, usize, f32)> = Vec::new();
        let mut line_w = 0.0f32;
        // Split into words keeping the following spaces attached.
        let mut words: Vec<Vec<(char, usize, f32)>> = Vec::new();
        let mut cur = Vec::new();
        for g in glyphs {
            let is_space = g.0 == ' ';
            cur.push(g);
            if is_space {
                words.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            words.push(cur);
        }
        for word in words {
            let ww: f32 = word.iter().map(|g| g.2).sum();
            let ww_nospace: f32 = word.iter().filter(|g| g.0 != ' ').map(|g| g.2).sum();
            if line_w + ww_nospace <= width {
                line_w += ww;
                line.extend(word);
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line_w = 0.0;
            }
            if ww_nospace <= width {
                line_w = ww;
                line = word;
                continue;
            }
            // Word longer than a line: break at character boundaries.
            self.st.words_broken += 1;
            for g in word {
                if line_w + g.2 > width && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    line_w = 0.0;
                }
                line_w += g.2;
                line.push(g);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        lines
    }

    fn paragraph(&mut self, text: &str, indent: f32, size: f32, mono: bool) -> Result<(), RenderError> {
        let mut text = text.to_string();
        if text.chars().any(is_rtl) {
            self.st.rtl_markers += 1;
            text = format!("{RTL_MARKER}{text}");
        }
        let glyphs = resolve(self.fonts, &text, mono, size, &mut self.st);
        let x0 = MARGIN + indent;
        let lines = self.wrap(glyphs, Self::right() - x0);
        let lh = size * 1.35;
        for l in lines {
            self.ensure(lh)?;
            self.y -= lh;
            self.emit_line(x0, &l, size);
        }
        self.gap(size * 0.4);
        Ok(())
    }

    fn code_block(&mut self, text: &str, indent: f32) -> Result<(), RenderError> {
        let size = 8.5;
        let lh = size * 1.3;
        let x0 = MARGIN + indent + 6.0;
        let width = Self::right() - x0;
        for src_line in text.trim_end_matches('\n').split('\n') {
            let glyphs = resolve(self.fonts, src_line, true, size, &mut self.st);
            // Character-boundary wrapping with a visible continuation mark.
            let mut chunks: Vec<Vec<(char, usize, f32)>> = Vec::new();
            let mut cur = Vec::new();
            let mut w = 0.0;
            let mark = ('\u{21AA}', F_MONO, self.fonts.advance(F_MONO, '\u{21AA}', size));
            let mark = if self.fonts.has(F_MONO, mark.0) { mark } else { ('>', F_MONO, self.fonts.advance(F_MONO, '>', size)) };
            for g in glyphs {
                if w + g.2 > width && !cur.is_empty() {
                    chunks.push(std::mem::take(&mut cur));
                    cur.push(mark);
                    w = mark.2;
                    self.st.code_continuations += 1;
                }
                w += g.2;
                cur.push(g);
            }
            chunks.push(cur);
            for c in chunks {
                self.ensure(lh)?;
                self.y -= lh;
                self.emit_line(x0, &c, size);
            }
        }
        self.gap(4.0);
        Ok(())
    }

    fn rule(&mut self) -> Result<(), RenderError> {
        self.ensure(8.0)?;
        self.y -= 4.0;
        let y = self.y;
        self.pages.last_mut().unwrap().push(Item::Rule { x1: MARGIN, y1: y, x2: Self::right(), y2: y, w: 0.5 });
        self.y -= 4.0;
        Ok(())
    }

    fn table(&mut self, header: &[String], rows: &[Vec<String>]) -> Result<(), RenderError> {
        let ncols = header.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
        if ncols > MAX_TABLE_COLS {
            return Err(RenderError("limit_exceeded:table_columns".into()));
        }
        if ncols == 0 {
            return Ok(());
        }
        let size = 8.0;
        let lh = size * 1.3;
        let pad = 3.0;
        let avail = Self::right() - MARGIN;
        // Natural column widths, capped, then scaled to the available width.
        let mut nat = vec![20.0f32; ncols];
        for r in std::iter::once(header).chain(rows.iter().map(|r| r.as_slice())) {
            for (i, c) in r.iter().enumerate() {
                let w: f32 = c.chars().map(|ch| self.fonts.advance(F_MAIN, ch, size)).sum::<f32>() + 2.0 * pad;
                nat[i] = nat[i].max(w.min(avail * 0.5));
            }
        }
        let total: f32 = nat.iter().sum();
        let widths: Vec<f32> = nat.iter().map(|w| w * avail / total).collect();
        let draw_row = |l: &mut Layout, cells: &[String]| -> Result<(), RenderError> {
            let mut wrapped = Vec::new();
            let mut max_lines = 1;
            for i in 0..ncols {
                let text = cells.get(i).cloned().unwrap_or_default();
                let g = resolve(l.fonts, &text, false, size, &mut l.st);
                let lines = l.wrap(g, widths[i] - 2.0 * pad);
                max_lines = max_lines.max(lines.len());
                wrapped.push(lines);
            }
            let h = max_lines as f32 * lh + 2.0 * pad;
            if h > PAGE_H - 2.0 * MARGIN - FOOTER - lh * 4.0 {
                return Err(RenderError("limit_exceeded:row_height".into()));
            }
            let top = l.y;
            let mut x = MARGIN;
            for (i, lines) in wrapped.iter().enumerate() {
                l.y = top - pad;
                for line in lines {
                    l.y -= lh;
                    l.emit_line(x + pad, line, size);
                }
                x += widths[i];
            }
            let bottom = top - h;
            let page = l.pages.last_mut().unwrap();
            page.push(Item::Rule { x1: MARGIN, y1: top, x2: MARGIN + avail, y2: top, w: 0.4 });
            page.push(Item::Rule { x1: MARGIN, y1: bottom, x2: MARGIN + avail, y2: bottom, w: 0.4 });
            let mut x = MARGIN;
            page.push(Item::Rule { x1: x, y1: top, x2: x, y2: bottom, w: 0.4 });
            for w in &widths {
                x += w;
                page.push(Item::Rule { x1: x, y1: top, x2: x, y2: bottom, w: 0.4 });
            }
            l.y = bottom;
            Ok(())
        };
        let header_h = {
            // Estimate header height for the page-fit check.
            let mut max_lines = 1;
            let broken_before = self.st.words_broken;
            for (i, c) in header.iter().enumerate() {
                let g = resolve(self.fonts, c, false, size, &mut Stats::default());
                max_lines = max_lines.max(self.wrap(g, widths[i] - 2.0 * pad).len());
            }
            self.st.words_broken = broken_before;
            max_lines as f32 * lh + 2.0 * pad
        };
        self.ensure(header_h + lh * 2.0)?;
        draw_row(self, header)?;
        for r in rows {
            // Measure the row first so that a page break happens before it, with header repeat.
            let mut max_lines = 1;
            let broken_before = self.st.words_broken;
            for i in 0..ncols {
                let text = r.get(i).cloned().unwrap_or_default();
                let g = resolve(self.fonts, &text, false, size, &mut Stats::default());
                max_lines = max_lines.max(self.wrap(g, widths[i] - 2.0 * pad).len());
            }
            self.st.words_broken = broken_before;
            let h = max_lines as f32 * lh + 2.0 * pad;
            if self.y - h < Self::bottom() {
                self.new_page()?;
                self.st.table_header_repeats += 1;
                draw_row(self, header)?;
            }
            draw_row(self, r)?;
        }
        self.gap(6.0);
        Ok(())
    }
}

// ---------------------------------------------------------------- Markdown walk

fn layout_markdown<'f>(md: &str, fonts: &'f Fonts) -> Result<Layout<'f>, RenderError> {
    let mut l = Layout::new(fonts);
    let mut depth = 0usize;
    let mut list_depth = 0usize;
    let mut quote_depth = 0usize;
    let mut ordered: Vec<Option<u64>> = Vec::new();
    let mut buf = String::new();
    let mut in_code = false;
    let mut code = String::new();
    let mut in_image = false;
    let mut image_alt = String::new();
    let mut link_dest: Vec<String> = Vec::new();
    let mut heading_size: Option<f32> = None;
    let mut item_prefix: Option<String> = None;
    // Tables
    let mut in_table = false;
    let mut header: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();

    let indent = |list_depth: usize, quote_depth: usize| (list_depth as f32) * 14.0 + (quote_depth as f32) * 12.0;

    let flush = |l: &mut Layout, buf: &mut String, size: f32, ind: f32, prefix: &mut Option<String>| -> Result<(), RenderError> {
        if buf.trim().is_empty() && prefix.is_none() {
            buf.clear();
            return Ok(());
        }
        let text = match prefix.take() {
            Some(p) => format!("{p}{}", buf.trim()),
            None => buf.trim().to_string(),
        };
        buf.clear();
        l.paragraph(&text, ind, size, false)
    };

    for ev in Parser::new_ext(md, Options::ENABLE_TABLES) {
        match &ev {
            Event::Start(_) => {
                depth += 1;
                if depth > MAX_NESTING {
                    return Err(RenderError("limit_exceeded:nesting".into()));
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        if in_table && !matches!(ev, Event::End(TagEnd::Table)) {
            match ev {
                Event::Start(Tag::TableHead) => row.clear(),
                Event::End(TagEnd::TableHead) => header = std::mem::take(&mut row),
                Event::Start(Tag::TableRow) => row.clear(),
                Event::End(TagEnd::TableRow) => rows.push(std::mem::take(&mut row)),
                Event::Start(Tag::TableCell) => buf.clear(),
                Event::End(TagEnd::TableCell) => {
                    if buf.len() > MAX_CELL {
                        return Err(RenderError("limit_exceeded:cell_size".into()));
                    }
                    row.push(buf.trim().to_string());
                    buf.clear();
                }
                Event::Text(t) | Event::Code(t) => buf.push_str(&t),
                Event::Html(t) | Event::InlineHtml(t) => {
                    l.st.raw_html += 1;
                    buf.push_str("[raw HTML not rendered] ");
                    buf.push_str(&t);
                }
                Event::SoftBreak | Event::HardBreak => buf.push(' '),
                Event::Start(Tag::Link { dest_url, .. }) => link_dest.push(dest_url.to_string()),
                Event::End(TagEnd::Link) => {
                    if let Some(d) = link_dest.pop() {
                        l.st.links_as_text += 1;
                        buf.push_str(&format!(" <{d}>"));
                    }
                }
                Event::Start(Tag::Image { .. }) => buf.push_str("[image omitted: "),
                Event::End(TagEnd::Image) => {
                    l.st.images_omitted += 1;
                    buf.push(']');
                }
                _ => {}
            }
            continue;
        }
        match ev {
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                heading_size = Some(match level as usize {
                    1 => 18.0,
                    2 => 15.0,
                    3 => 13.0,
                    _ => 11.0,
                });
                l.gap(4.0);
            }
            Event::End(TagEnd::Heading(_)) => {
                let s = heading_size.take().unwrap_or(11.0);
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, s, ind, &mut item_prefix)?;
            }
            Event::Start(Tag::BlockQuote(_)) => quote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote_depth = quote_depth.saturating_sub(1),
            Event::Start(Tag::CodeBlock(_)) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                in_code = true;
                code.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                let ind = indent(list_depth, quote_depth);
                l.code_block(&code, ind)?;
                code.clear();
            }
            Event::Start(Tag::List(first)) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                list_depth += 1;
                if list_depth > MAX_LIST_DEPTH {
                    return Err(RenderError("limit_exceeded:list_depth".into()));
                }
                ordered.push(first);
            }
            Event::End(TagEnd::List(_)) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                list_depth = list_depth.saturating_sub(1);
                ordered.pop();
            }
            Event::Start(Tag::Item) => {
                let ind = indent(list_depth.saturating_sub(1), quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                item_prefix = Some(match ordered.last_mut() {
                    Some(Some(n)) => {
                        let p = format!("{n}. ");
                        *n += 1;
                        p
                    }
                    _ => "\u{2022} ".to_string(),
                });
            }
            Event::End(TagEnd::Item) => {
                let ind = indent(list_depth.saturating_sub(1), quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
            }
            Event::Start(Tag::Table(_)) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                in_table = true;
                header.clear();
                rows.clear();
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                let (h, r) = (std::mem::take(&mut header), std::mem::take(&mut rows));
                l.table(&h, &r)?;
            }
            Event::Start(Tag::Link { dest_url, .. }) => link_dest.push(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(d) = link_dest.pop() {
                    l.st.links_as_text += 1;
                    buf.push_str(&format!(" <{d}>"));
                }
            }
            Event::Start(Tag::Image { .. }) => {
                in_image = true;
                image_alt.clear();
            }
            Event::End(TagEnd::Image) => {
                in_image = false;
                l.st.images_omitted += 1;
                buf.push_str(&format!("[image omitted: {}]", image_alt));
            }
            Event::Text(t) => {
                if in_code {
                    code.push_str(&t);
                } else if in_image {
                    image_alt.push_str(&t);
                } else {
                    buf.push_str(&t);
                }
            }
            Event::Code(t) => buf.push_str(&t),
            Event::Html(t) | Event::InlineHtml(t) => {
                l.st.raw_html += 1;
                if in_code {
                    code.push_str(&t);
                } else {
                    buf.push_str("[raw HTML not rendered] ");
                    buf.push_str(&t);
                    buf.push(' ');
                }
            }
            Event::Start(Tag::HtmlBlock) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
            }
            Event::End(TagEnd::HtmlBlock) => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
            }
            Event::FootnoteReference(t) => buf.push_str(&format!("[^{t}]")),
            Event::SoftBreak => buf.push(' '),
            Event::HardBreak => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
            }
            Event::Rule => {
                let ind = indent(list_depth, quote_depth);
                flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
                l.rule()?;
            }
            Event::TaskListMarker(done) => buf.push_str(if done { "[x] " } else { "[ ] " }),
            Event::InlineMath(t) | Event::DisplayMath(t) => buf.push_str(&t),
            _ => {}
        }
    }
    let ind = indent(list_depth, quote_depth);
    flush(&mut l, &mut buf, 10.0, ind, &mut item_prefix)?;
    Ok(l)
}

// ---------------------------------------------------------------- render (child process)

fn vm_hwm_kib() -> i64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse().ok()))
        })
        .unwrap_or(-1)
}

fn render(input: &Path, tmp_out: &Path, fault: &str) -> J {
    let t0 = Instant::now();
    let fail = |e: String, extra: Vec<(&str, J)>| {
        let mut v = vec![("ok", J::B(false)), ("error", J::s(e))];
        v.extend(extra);
        v.push(("vm_hwm_kib", J::N(vm_hwm_kib())));
        obj(v)
    };
    let len = match fs::metadata(input) {
        Ok(m) => m.len(),
        Err(e) => return fail(format!("input_unreadable:{e}"), vec![]),
    };
    if len > MAX_INPUT {
        return fail("limit_exceeded:input_bytes".into(), vec![("input_bytes", J::N(len as i64))]);
    }
    let md = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => return fail(format!("input_unreadable:{e}"), vec![]),
    };
    let fonts = match Fonts::load(fault == "font") {
        Ok(f) => f,
        Err(e) => return fail(e, vec![("input_bytes", J::N(len as i64))]),
    };
    let t_fonts = t0.elapsed();
    let layout = match layout_markdown(&md, &fonts) {
        Ok(l) => l,
        Err(RenderError(e)) => return fail(e, vec![("input_bytes", J::N(len as i64))]),
    };
    let t_layout = t0.elapsed();

    // Only fonts that are actually used are added to the document.
    let mut used = BTreeSet::new();
    for p in &layout.pages {
        for it in p {
            if let Item::Text { font, .. } = it {
                used.insert(*font);
            }
        }
    }
    let mut doc = PdfDocument::new("P00-T08 spike report");
    let mut ids: BTreeMap<usize, FontId> = BTreeMap::new();
    for f in &used {
        ids.insert(*f, doc.add_font(&fonts.parsed[*f]));
    }
    let npages = layout.pages.len();
    let mut pages = Vec::with_capacity(npages);
    for (pi, items) in layout.pages.iter().enumerate() {
        let mut ops = Vec::new();
        ops.push(Op::SetOutlineColor { col: Color::Greyscale(Greyscale { percent: 0.4, icc_profile: None }) });
        for it in items {
            match it {
                Item::Text { x, y, font, size, text } => {
                    ops.push(Op::StartTextSection);
                    ops.push(Op::SetFont { font: PdfFontHandle::External(ids[font].clone()), size: Pt(*size) });
                    ops.push(Op::SetTextCursor { pos: Point { x: Pt(*x), y: Pt(*y) } });
                    ops.push(Op::ShowText { items: vec![TextItem::Text(text.clone())] });
                    ops.push(Op::EndTextSection);
                }
                Item::Rule { x1, y1, x2, y2, w } => {
                    ops.push(Op::SetOutlineThickness { pt: Pt(*w) });
                    ops.push(Op::DrawLine {
                        line: Line {
                            points: vec![
                                LinePoint { p: Point { x: Pt(*x1), y: Pt(*y1) }, bezier: false },
                                LinePoint { p: Point { x: Pt(*x2), y: Pt(*y2) }, bezier: false },
                            ],
                            is_closed: false,
                        },
                    });
                }
            }
        }
        // Footer page number (main font is always loaded; add it if not yet used).
        let label = format!("Page {}", pi + 1);
        if let Some(id) = ids.get(&F_MAIN) {
            let w: f32 = label.chars().map(|c| fonts.advance(F_MAIN, c, 8.0)).sum();
            ops.push(Op::StartTextSection);
            ops.push(Op::SetFont { font: PdfFontHandle::External(id.clone()), size: Pt(8.0) });
            ops.push(Op::SetTextCursor { pos: Point { x: Pt((PAGE_W - w) / 2.0), y: Pt(MARGIN - 10.0) } });
            ops.push(Op::ShowText { items: vec![TextItem::Text(label)] });
            ops.push(Op::EndTextSection);
        }
        pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    }
    doc.with_pages(pages);
    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let bytes = doc.save(&opts, &mut warnings);
    let t_save = t0.elapsed();
    let warn_json: Vec<J> = warnings
        .iter()
        .take(20)
        .map(|w| J::s(format!("{:?}: {}", w.severity, w.msg.chars().take(200).collect::<String>())))
        .collect();
    let st = &layout.st;
    let stats = obj(vec![
        ("pages", J::N(npages as i64)),
        ("lines", J::N(st.lines as i64)),
        ("chars_DejaVuSans", J::N(st.chars_by_font[F_MAIN] as i64)),
        ("chars_DroidSansFallbackFull", J::N(st.chars_by_font[F_CJK] as i64)),
        ("chars_NotoSansMono", J::N(st.chars_by_font[F_MONO] as i64)),
        ("missing_glyph_chars_shown_as_codepoints", J::N(st.missing as i64)),
        ("missing_samples", J::A(st.missing_samples.iter().map(|s| J::s(s.clone())).collect())),
        ("emoji_shown_as_codepoints", J::N(st.emoji as i64)),
        ("control_chars_shown_as_codepoints", J::N(st.controls as i64)),
        ("rtl_chars_unshaped", J::N(st.rtl_chars as i64)),
        ("rtl_markers", J::N(st.rtl_markers as i64)),
        ("combining_marks_unpositioned", J::N(st.combining as i64)),
        ("code_continuations", J::N(st.code_continuations as i64)),
        ("words_broken_at_char", J::N(st.words_broken as i64)),
        ("table_header_repeats", J::N(st.table_header_repeats as i64)),
        ("raw_html_blocks", J::N(st.raw_html as i64)),
        ("images_omitted", J::N(st.images_omitted as i64)),
        ("links_as_text", J::N(st.links_as_text as i64)),
        ("model_max_right_overflow_pt", J::F(st.max_right_overflow as f64)),
        ("model_min_baseline_above_bottom_pt", J::F(if st.min_baseline_margin == f32::MAX { 0.0 } else { st.min_baseline_margin as f64 })),
    ]);
    let font_list: Vec<J> = used.iter().map(|f| J::s(FONTS[*f].0)).collect();
    let font_bytes: u64 = used.iter().map(|f| fs::metadata(FONTS[*f].1).map(|m| m.len()).unwrap_or(0)).sum();
    let common = vec![
        ("input_bytes", J::N(len as i64)),
        ("stats", stats),
        ("fonts_added", J::A(font_list)),
        ("font_file_bytes_added", J::N(font_bytes as i64)),
        ("save_options", J::s(format!("optimize={} subset_fonts={} (no effect without text_layout) secure={} (skip unknown ops; not encryption)", opts.optimize, opts.subset_fonts, opts.secure))),
        ("printpdf_warnings", J::A(warn_json)),
        ("printpdf_warning_count", J::N(warnings.len() as i64)),
        ("ms_fonts", J::N(t_fonts.as_millis() as i64)),
        ("ms_layout", J::N(t_layout.as_millis() as i64)),
        ("ms_total_before_write", J::N(t_save.as_millis() as i64)),
    ];
    if bytes.len() > MAX_OUTPUT {
        let mut v = common;
        v.push(("output_bytes", J::N(bytes.len() as i64)));
        return fail("limit_exceeded:output_bytes".into(), v);
    }
    let write = || -> std::io::Result<()> {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(tmp_out)?;
        f.write_all(&bytes)?;
        f.sync_all()
    };
    if let Err(e) = write() {
        let _ = fs::remove_file(tmp_out);
        let mut v = common;
        v.push(("temp_removed", J::B(!tmp_out.exists())));
        return fail(format!("write_failed:{}", e.kind()), v);
    }
    let mut v = vec![("ok", J::B(true)), ("error", J::Null), ("output_bytes", J::N(bytes.len() as i64))];
    v.extend(common);
    v.push(("vm_hwm_kib", J::N(vm_hwm_kib())));
    obj(v)
}

// ---------------------------------------------------------------- orchestrator helpers

fn run_capped(mut cmd: Command, kill_after: Duration, out_file: &Path, err_file: &Path) -> (Option<i32>, bool, u128) {
    let t = Instant::now();
    let child = cmd
        .stdin(Stdio::null())
        .stdout(fs::File::create(out_file).unwrap())
        .stderr(fs::File::create(err_file).unwrap())
        .spawn();
    let Ok(mut child) = child else { return (None, false, 0) };
    loop {
        if let Ok(Some(s)) = child.try_wait() {
            return (s.code(), false, t.elapsed().as_millis());
        }
        if t.elapsed() > kill_after {
            let _ = child.kill();
            let s = child.wait().ok();
            return (s.and_then(|s| s.code()), true, t.elapsed().as_millis());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_capped(p: &Path, cap: u64) -> Vec<u8> {
    let mut v = Vec::new();
    if let Ok(f) = fs::File::open(p) {
        let _ = f.take(cap).read_to_end(&mut v);
    }
    v
}

fn tool(root: &Path) -> Command {
    let mut c = Command::new(MUTOOL);
    c.env_clear().env("PATH", "/usr/bin:/bin").env("LANG", "C.UTF-8").env("HOME", root.join("home"));
    c
}

fn sha256(path: &Path) -> Option<String> {
    let out = Command::new(SHA256SUM).env_clear().env("PATH", "/usr/bin:/bin").arg(path).stdin(Stdio::null()).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().map(String::from)
}

/// Names (`/Name` tokens) in a PDF file, skipping strings, comments and stream bodies.
fn pdf_names(data: &[u8]) -> BTreeMap<String, u64> {
    let mut names = BTreeMap::new();
    let mut i = 0;
    let delim = |b: u8| b.is_ascii_whitespace() || b"()<>[]{}/%".contains(&b);
    while i < data.len() {
        let b = data[i];
        match b {
            b'%' => {
                while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                    i += 1;
                }
            }
            b'(' => {
                let mut level = 1;
                i += 1;
                while i < data.len() && level > 0 {
                    match data[i] {
                        b'\\' => i += 1,
                        b'(' => level += 1,
                        b')' => level -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                continue;
            }
            b'<' if data.get(i + 1) != Some(&b'<') => {
                while i < data.len() && data[i] != b'>' {
                    i += 1;
                }
            }
            b'<' => i += 1,
            b'/' => {
                let start = i + 1;
                i += 1;
                while i < data.len() && !delim(data[i]) {
                    i += 1;
                }
                let n = String::from_utf8_lossy(&data[start..i]).into_owned();
                *names.entry(n).or_insert(0) += 1;
                continue;
            }
            b's' if data[i..].starts_with(b"stream") && (i == 0 || delim(data[i - 1])) => {
                // Skip to "endstream".
                let rest = &data[i + 6..];
                match rest.windows(9).position(|w| w == b"endstream") {
                    Some(p) => i += 6 + p + 9,
                    None => break,
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    names
}

const FORBIDDEN_NAMES: &[&str] = &[
    "JavaScript", "JS", "OpenAction", "AA", "Launch", "URI", "SubmitForm", "AcroForm", "XFA",
    "EmbeddedFile", "EmbeddedFiles", "RichMedia", "Annots", "Encrypt", "Metadata", "GoTo", "GoToR",
    "ImportData", "Sound", "Movie",
];

fn normalise_extracted(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace() && *c != '\u{21AA}').collect()
}

// ---------------------------------------------------------------- fixtures

struct Fixture {
    id: String,
    group: &'static str,
    md: Vec<u8>,
    fault: &'static str,
    out_parent_missing: bool,
    expect_ok: bool,
    expect_error_prefix: Option<&'static str>,
    pages: (usize, usize),
    sentinels: Vec<String>,
    note: &'static str,
}

fn fx(id: &str, group: &'static str, md: String, pages: (usize, usize), sentinels: Vec<&str>, note: &'static str) -> Fixture {
    Fixture {
        id: id.into(),
        group,
        md: md.into_bytes(),
        fault: "none",
        out_parent_missing: false,
        expect_ok: true,
        expect_error_prefix: None,
        pages,
        sentinels: sentinels.into_iter().map(String::from).collect(),
        note,
    }
}

fn fixtures(canary: &str) -> Vec<Fixture> {
    let mut v = Vec::new();
    // U1 supported (Latin, CJK) and intentionally unsupported (RTL, emoji, combining) parts.
    v.push(fx("U1-latin-cjk", "supported", "# Unicode supported\n\nLatin: Ærøskøbing café façade naïve Ünïcödé Ångström Łódź Šibenik.\n\nCJK: 漢字テスト ひらがな カタカナ 中文测试 한국어 시험.\n\nMixed: report-ID 報告書-0042 done.\n".into(), (1, 1),
        vec!["Ærøskøbing", "café", "façade", "Łódź", "Šibenik", "漢字テスト", "ひらがな", "カタカナ", "中文测试", "한국어", "報告書-0042"], "Latin precomposed + CJK must extract and have no missing glyphs"));
    v.push(fx("U1-unsupported", "unsupported", "# Unicode unsupported\n\nArabic: مرحبا بالعالم.\n\nHebrew: שלום עולם.\n\nEmoji: 😀 🚀.\n\nCombining: e\u{0301} a\u{0308} n\u{0303}.\n\nAstral: 𝐀𝐁 𓀀.\n".into(), (1, 1),
        vec!["[complexscript/RTLnotshaped]", "<U+1F600>", "<U+1F680>"], "RTL marker and code-point labels must be visible; shaping/bidi/mark positioning not claimed"));
    v.push(fx("U2", "supported", "# Controls\n\nOverride: abc\u{202E}def\u{202C} zero\u{200B}width bom\u{FEFF}x bell\u{0007}end c1\u{0085}x.\n".into(), (1, 1),
        vec!["<U+202E>", "<U+202C>", "<U+200B>", "<U+FEFF>", "<U+0007>", "<U+0085>"], "controls shown as code points, not applied"));
    // T1 tables
    {
        let mut md = String::from("# Tables\n\n");
        md.push_str("|C1|C2|C3|C4|C5|C6|C7|C8|C9|C10|C11|C12|\n|");
        md.push_str(&"---|".repeat(12));
        md.push('\n');
        for r in 0..3 {
            md.push('|');
            for c in 0..12 {
                md.push_str(&format!("w{r}c{c}|"));
            }
            md.push('\n');
        }
        md.push_str("\n|#|Rule|Severity|Path|\n|---|---|---|---|\n");
        let long = format!("deploy/{}/manifest.yaml", "a".repeat(180));
        for r in 1..=500 {
            let p = if r % 100 == 0 { long.clone() } else { format!("svc{r:03}/k8s/deployment.yaml") };
            md.push_str(&format!("|{r}|RULE-{r:04}|high|{p}|\n"));
        }
        let mut f = fx("T1", "supported", md, (8, 60), vec!["w0c0", "w2c11", "RULE-0001", "RULE-0250", "RULE-0500", "svc499/k8s/deployment.yaml"], "every sampled cell extracted; long path not truncated; header repeated");
        f.sentinels.push(long);
        v.push(f);
    }
    // B1 page breaks
    {
        let mut md = String::from("# Page breaks\n\n```\n");
        for i in 0..150 {
            md.push_str(&format!("line {i:03}: {}\n", "x".repeat(if i % 10 == 0 { 160 } else { 30 })));
        }
        md.push_str("```\n\n");
        for i in 0..40 {
            md.push_str(&format!("Paragraph {i}: {}\n\n", "lorem ipsum dolor sit amet ".repeat(12)));
        }
        md.push_str("## Heading near the end\n\nFinal paragraph B1-END.\n");
        v.push(fx("B1", "supported", md, (3, 15), vec!["line000:", "line149:", "Paragraph39:", "B1-END"], "content continuous; code wraps with continuation mark"));
    }
    v.push(fx("R1", "supported", "# Remote\n\n![remote image](https://example.invalid/x.png)\n\n[remote link](https://example.invalid/page)\n\n<img src=\"https://example.invalid/y.png\">\n\n<link rel=\"stylesheet\" href=\"https://example.invalid/s.css\">\n".into(), (1, 1),
        vec!["[imageomitted:remoteimage]", "remotelink<https://example.invalid/page>", "[rawHTMLnotrendered]"], "no fetch; no image XObject; no URI action"));
    v.push(fx("L1", "supported", "# Local files\n\n![abs](/etc/passwd)\n\n![file](file:///etc/hostname)\n\n[rel](../../secret.txt)\n\n<img src=\"file:///etc/shadow\">\n".into(), (1, 1),
        vec!["[imageomitted:abs]", "[imageomitted:file]", "rel<../../secret.txt>"], "no local file opened or embedded (only allowlisted fonts)"));
    v.push(fx("A1", "supported", "# Active\n\n<script>alert('x')</script>\n\n<iframe src=\"https://example.invalid\"></iframe>\n\n<form action=\"https://example.invalid\"><input name=a></form>\n\n[js](javascript:alert(1))\n".into(), (1, 1),
        vec!["[rawHTMLnotrendered]<script>alert('x')</script>", "js<javascript:alert(1)>"], "escaped literal text; no actions/JS/forms"));
    {
        let mut md = String::from("# Large report\n\n");
        for i in 1..=5000 {
            md.push_str(&format!("### F-{i:05} RULE-{:03} high\n\nFile `svc{:03}/deploy.yaml` line {}: configuration issue description for finding {i}, with remediation text kept short.\n\n", i % 300, i % 997, i % 400));
        }
        v.push(fx("G1", "supported", md, (50, 1500), vec!["F-00001", "F-02500", "F-05000"], "bounded time/memory; first/last finding present"));
    }
    {
        let mut f = fx("C1", "supported", format!("# Canary\n\nToken: {canary}\n"), (1, 1), vec![], "renderer is not a redaction boundary: canary must appear");
        f.sentinels.push(canary.to_string());
        v.push(f);
    }
    // X1 limits
    let lim = |id: &str, md: Vec<u8>, e: &'static str| Fixture {
        id: id.into(), group: "limit", md, fault: "none", out_parent_missing: false, expect_ok: false,
        expect_error_prefix: Some(e), pages: (0, 0), sentinels: vec![], note: "typed limit; no PDF",
    };
    v.push(lim("X1a-input", vec![b'a'; (MAX_INPUT + 1) as usize], "limit_exceeded:input_bytes"));
    v.push(lim("X1b-nesting", format!("{}deep\n", "> ".repeat(17)).into_bytes(), "limit_exceeded:nesting"));
    v.push(lim("X1c-columns", {
        let mut m = String::from("|");
        for c in 0..33 { m.push_str(&format!("h{c}|")); }
        m.push_str("\n|");
        m.push_str(&"---|".repeat(33));
        m.push_str("\n|");
        m.push_str(&"x|".repeat(33));
        m.push('\n');
        m.into_bytes()
    }, "limit_exceeded:table_columns"));
    v.push(lim("X1d-pages", {
        let mut m = String::new();
        for i in 0..2100 {
            m.push_str(&format!("{}\n\n", format!("page filler {i} ").repeat(1)));
            for _ in 0..55 { m.push_str("filler line\n\n"); }
        }
        m.into_bytes()
    }, "limit_exceeded:pages"));
    v.push(lim("X1e-listdepth", b"- a\n  - b\n    - c\n      - d\n".to_vec(), "limit_exceeded:list_depth"));
    // F1 failure paths
    let mut f1a = lim("F1a-font", b"# font fault\n\ntext\n".to_vec(), "font_unavailable");
    f1a.fault = "font";
    f1a.group = "failure";
    f1a.note = "typed font failure; no PDF";
    v.push(f1a);
    let mut f1b = lim("F1b-write", b"# write fault\n\ntext\n".to_vec(), "write_failed");
    f1b.out_parent_missing = true;
    f1b.group = "failure";
    f1b.note = "typed write failure; no temp file; JSON/MD fixtures untouched";
    v.push(f1b);
    v
}

// ---------------------------------------------------------------- orchestrate

fn preflight(root: &Path) -> (bool, J) {
    let mut checks = Vec::new();
    let mut ok = true;
    let mut ck = |name: &str, pass: bool, detail: String| {
        ok &= pass;
        checks.push(obj(vec![("check", J::s(name)), ("ok", J::B(pass)), ("detail", J::s(detail))]));
    };
    ck("pid1", std::process::id() == 1, format!("pid={}", std::process::id()));
    let netdev = fs::read_to_string("/proc/net/dev").unwrap_or_default();
    let ifaces: Vec<String> = netdev.lines().skip(2).filter_map(|l| l.split(':').next().map(|s| s.trim().to_string())).collect();
    ck("netns_loopback_only", ifaces == ["lo"], format!("{ifaces:?}"));
    let mut keys: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    keys.sort();
    ck("inherited_env_exactly_LANG_PATH", keys == ["LANG", "PATH"], format!("{keys:?}"));
    let mode = fs::metadata(root).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
    ck("spike_root_0700", mode == 0o700, format!("{mode:o}"));
    ck("spike_root_prefix", root.to_string_lossy().starts_with("/home/kali/.local/share/pwt-dev-tools/_spikes/t08."), root.display().to_string());
    let limit = fs::read_to_string("/proc/self/limits").unwrap_or_default();
    let as_line = limit.lines().find(|l| l.starts_with("Max address space")).unwrap_or("").to_string();
    ck("address_space_limit_2GiB", as_line.split_whitespace().nth(3) == Some("2147483648"), as_line.split_whitespace().collect::<Vec<_>>().join(" "));
    let expected = fs::read_to_string(root.join("fonts.sha256")).unwrap_or_default();
    for (name, path) in FONTS {
        let meta = fs::symlink_metadata(path);
        let regular = meta.map(|m| m.file_type().is_file()).unwrap_or(false);
        let h = sha256(Path::new(path)).unwrap_or_default();
        let want = expected.lines().find(|l| l.ends_with(path)).and_then(|l| l.split_whitespace().next()).unwrap_or("");
        ck(&format!("font {name} regular+hash_matches_outer"), regular && !h.is_empty() && h == want, format!("sha256={h}"));
    }
    for t in [MUTOOL, SHA256SUM] {
        ck(&format!("exists {t}"), Path::new(t).is_file(), String::new());
    }
    (ok, J::A(checks))
}

fn verify(root: &Path, case_dir: &Path, tmp: &Path, fxt: &Fixture, canary: &str) -> (bool, J) {
    let mut steps: Vec<J> = Vec::new();
    let mut all = true;
    let mut step = |n: i64, name: &str, pass: bool, detail: J| {
        all &= pass;
        steps.push(obj(vec![("step", J::N(n)), ("name", J::s(name)), ("ok", J::B(pass)), ("detail", detail)]));
    };
    // 1. size
    let size = fs::metadata(tmp).map(|m| m.len()).unwrap_or(0);
    step(1, "size_nonzero_within_limit", size > 0 && size as usize <= MAX_OUTPUT, J::N(size as i64));
    // 2. header/trailer
    let data = fs::read(tmp).unwrap_or_default();
    let head_ok = data.starts_with(b"%PDF-1.");
    let tail = &data[data.len().saturating_sub(1024)..];
    let eof_ok = tail.windows(5).any(|w| w == b"%%EOF");
    step(2, "header_and_eof", head_ok && eof_ok, J::s(format!("header={} eof_in_last_1KiB={}", head_ok, eof_ok)));
    // 3. mutool info
    let (info_out, info_err) = (case_dir.join("mutool-info.stdout"), case_dir.join("mutool-info.stderr"));
    let mut c = tool(root);
    c.arg("info").arg(tmp);
    let (code, killed, ms) = run_capped(c, TOOL_KILL, &info_out, &info_err);
    let info = String::from_utf8_lossy(&read_capped(&info_out, CAPTURE_CAP)).into_owned();
    let info_e = String::from_utf8_lossy(&read_capped(&info_err, CAPTURE_CAP)).into_owned();
    let pages = info.lines().find(|l| l.trim_start().starts_with("Pages:")).and_then(|l| l.split(':').nth(1)).and_then(|s| s.trim().parse::<usize>().ok()).unwrap_or(0);
    let encryption = info.lines().any(|l| l.to_ascii_lowercase().contains("encrypt"));
    let repaired = info_e.to_ascii_lowercase().contains("repair") || info_e.to_ascii_lowercase().contains("error");
    let in_range = pages >= fxt.pages.0 && pages <= fxt.pages.1;
    step(3, "mutool_info_parses_page_range", code == Some(0) && !killed && in_range && !encryption && !repaired,
        obj(vec![("exit", code.map(|c| J::N(c as i64)).unwrap_or(J::Null)), ("ms", J::N(ms as i64)), ("pages", J::N(pages as i64)),
            ("expected_range", J::s(format!("{}-{}", fxt.pages.0, fxt.pages.1))), ("encryption_reported", J::B(encryption)),
            ("stderr_repair_or_error", J::B(repaired)), ("stderr_excerpt", J::s(info_e.chars().take(400).collect::<String>())),
            ("font_lines", J::A(info.lines().filter(|l| l.contains("Font") || l.contains("TrueType") || l.contains("Type0")).take(12).map(|l| J::s(l.trim())).collect()))]));
    // 4. text extraction
    let (txt, txt_err) = (case_dir.join("extracted.txt"), case_dir.join("mutool-draw.stderr"));
    let mut c = tool(root);
    c.args(["draw", "-F", "txt", "-o"]).arg(&txt).arg(tmp);
    let (code, killed, ms) = run_capped(c, TOOL_KILL, &case_dir.join("mutool-draw.stdout"), &txt_err);
    let extracted = String::from_utf8_lossy(&fs::read(&txt).unwrap_or_default()).into_owned();
    let norm = normalise_extracted(&extracted);
    let mut missing = Vec::new();
    for s in &fxt.sentinels {
        if !norm.contains(&normalise_extracted(s)) {
            missing.push(J::s(if s == canary { "<canary>".to_string() } else { s.chars().take(60).collect() }));
        }
    }
    let canary_present = norm.contains(canary);
    let canary_ok = if fxt.id == "C1" { canary_present } else { !canary_present };
    let draw_e = String::from_utf8_lossy(&read_capped(&txt_err, CAPTURE_CAP)).into_owned();
    let draw_repair = draw_e.to_ascii_lowercase().contains("repair") || draw_e.to_ascii_lowercase().contains("error");
    step(4, "text_extraction_sentinels", code == Some(0) && !killed && missing.is_empty() && canary_ok && !draw_repair,
        obj(vec![("exit", code.map(|c| J::N(c as i64)).unwrap_or(J::Null)), ("ms", J::N(ms as i64)), ("sentinels", J::N(fxt.sentinels.len() as i64)),
            ("missing_sentinels", J::A(missing)), ("canary_expectation_met", J::B(canary_ok)), ("extracted_chars", J::N(extracted.chars().count() as i64)),
            ("stderr_repair_or_error", J::B(draw_repair)), ("stderr_excerpt", J::s(draw_e.chars().take(400).collect::<String>()))]));
    // 5. forbidden objects on the parser-normalised (decompressed, no object streams) file
    let norm_pdf = case_dir.join("normalised.pdf");
    let mut c = tool(root);
    c.args(["clean", "-d"]).arg(tmp).arg(&norm_pdf);
    let (code, killed, ms) = run_capped(c, TOOL_KILL, &case_dir.join("mutool-clean.stdout"), &case_dir.join("mutool-clean.stderr"));
    let clean_e = String::from_utf8_lossy(&read_capped(&case_dir.join("mutool-clean.stderr"), CAPTURE_CAP)).into_owned();
    let nd = fs::read(&norm_pdf).unwrap_or_default();
    let names_norm = pdf_names(&nd);
    let names_orig = pdf_names(&data);
    let hits: Vec<J> = FORBIDDEN_NAMES.iter().filter(|n| names_norm.contains_key(**n) || names_orig.contains_key(**n)).map(|n| J::s(*n)).collect();
    let images = names_norm.get("Image").copied().unwrap_or(0);
    let objstm = names_orig.contains_key("ObjStm");
    let font_files = names_norm.get("FontFile2").copied().unwrap_or(0) + names_norm.get("FontFile3").copied().unwrap_or(0);
    let clean_repair = clean_e.to_ascii_lowercase().contains("repair") || clean_e.to_ascii_lowercase().contains("error");
    step(5, "forbidden_object_scan", code == Some(0) && !killed && hits.is_empty() && images == 0 && !clean_repair && !nd.is_empty(),
        obj(vec![("exit", code.map(|c| J::N(c as i64)).unwrap_or(J::Null)), ("ms", J::N(ms as i64)), ("forbidden_name_hits", J::A(hits)),
            ("image_name_count", J::N(images as i64)), ("embedded_font_programs", J::N(font_files as i64)),
            ("original_uses_object_streams", J::B(objstm)), ("stderr_repair_or_error", J::B(clean_repair)),
            ("stderr_excerpt", J::s(clean_e.chars().take(400).collect::<String>()))]));
    let _ = fs::remove_file(&norm_pdf);
    (all, J::A(steps))
}

fn orchestrate(root: &Path, out: &Path) {
    let (pf_ok, pf) = preflight(root);
    let mut top: Vec<(&str, J)> = vec![("preflight_ok", J::B(pf_ok)), ("preflight", pf)];
    if !pf_ok || std::env::args().nth(4).as_deref() == Some("preflight-only") {
        top.push(("cases", J::s(if pf_ok { "not run: preflight-only mode" } else { "not run: preflight failed" })));
        fs::write(out.join("results.json"), obj(top).dump()).unwrap();
        if !pf_ok {
            std::process::exit(1);
        }
        return;
    }
    for d in ["home", "cases", "pdf", "fixtures-json-md"] {
        fs::create_dir_all(root.join(d)).unwrap();
        fs::set_permissions(root.join(d), fs::Permissions::from_mode(0o700)).unwrap();
    }
    // JSON/MD companions that F1 must leave untouched.
    let jpath = root.join("fixtures-json-md/report.json");
    let mpath = root.join("fixtures-json-md/report.md");
    fs::write(&jpath, b"{\"schema_version\":\"1.3\",\"store_type\":\"report\"}\n").unwrap();
    fs::write(&mpath, b"# report\n").unwrap();
    let (jh0, mh0) = (sha256(&jpath), sha256(&mpath));

    let mut rnd = [0u8; 8];
    fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut rnd)).unwrap_or_else(|_| die("urandom"));
    let canary = format!("PWTCANARY-{}", rnd.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let exe = std::env::current_exe().unwrap_or_else(|_| die("current_exe"));
    let mut cases = Vec::new();
    for f in fixtures(&canary) {
        let cd = root.join("cases").join(&f.id);
        fs::create_dir_all(&cd).unwrap();
        let input = cd.join("input.md");
        fs::write(&input, &f.md).unwrap();
        let final_pdf = root.join("pdf").join(format!("{}.pdf", f.id));
        let tmp = if f.out_parent_missing { root.join("does-not-exist").join("out.pdf.tmp") } else { root.join("pdf").join(format!("{}.pdf.tmp", f.id)) };
        let mut c = Command::new(&exe);
        c.env_clear().env("PATH", "/usr/bin:/bin").env("LANG", "C.UTF-8").env("HOME", root.join("home"));
        c.arg("render").arg(&input).arg(&tmp).arg(f.fault);
        let (code, killed, ms) = run_capped(c, CONVERSION_KILL, &cd.join("render.stdout"), &cd.join("render.stderr"));
        let rj = String::from_utf8_lossy(&read_capped(&cd.join("render.stdout"), CAPTURE_CAP)).lines().last().unwrap_or("").to_string();
        let ok_flag = rj.contains("\"ok\":true");
        let err = rj.split("\"error\":\"").nth(1).and_then(|s| s.split('"').next()).unwrap_or("").to_string();
        let mut rec: Vec<(&str, J)> = vec![
            ("case", J::s(f.id.clone())), ("group", J::s(f.group)), ("note", J::s(f.note)),
            ("render_exit", code.map(|c| J::N(c as i64)).unwrap_or(J::Null)), ("render_killed_at_60s", J::B(killed)),
            ("conversion_wall_ms", J::N(ms as i64)), ("render_result_raw_json", J::s(rj.replace(&canary, "<canary>"))),
        ];
        let verdict;
        if f.expect_ok {
            if ok_flag && !killed {
                let (vok, steps) = verify(root, &cd, &tmp, &f, &canary);
                rec.push(("verification", steps));
                if vok {
                    let renamed = fs::rename(&tmp, &final_pdf).is_ok();
                    let h = sha256(&final_pdf);
                    rec.push(("step6_renamed", J::B(renamed)));
                    rec.push(("pdf_sha256", h.map(J::S).unwrap_or(J::Null)));
                    verdict = if renamed { "pdf_written" } else { "verification_failed:rename" };
                } else {
                    let _ = fs::remove_file(&tmp);
                    verdict = "verification_failed";
                }
            } else {
                let _ = fs::remove_file(&tmp);
                verdict = if killed { "render_error:timeout" } else { "render_error" };
            }
        } else {
            let no_pdf = !tmp.exists() && !final_pdf.exists();
            let typed = f.expect_error_prefix.map(|p| err.starts_with(p)).unwrap_or(false);
            rec.push(("expected_error", J::s(f.expect_error_prefix.unwrap_or(""))));
            rec.push(("no_pdf_left", J::B(no_pdf)));
            verdict = if typed && no_pdf && !ok_flag && !killed { "expected_typed_failure" } else { "unexpected_outcome" };
            let _ = fs::remove_file(&tmp);
        }
        rec.push(("pdf_written", J::B(verdict == "pdf_written")));
        rec.push(("verdict", J::s(verdict)));
        rec.push(("visual_correctness", J::s("unverified (page rendering/inspection not in the approved tool list)")));
        if f.id == "C1" {
            // Do not retain canary-bearing artefacts.
            let _ = fs::remove_file(&final_pdf);
            let _ = fs::remove_file(cd.join("extracted.txt"));
            let _ = fs::remove_file(&input);
            rec.push(("canary_artifacts_removed", J::B(!final_pdf.exists() && !cd.join("extracted.txt").exists())));
        }
        if f.md.len() > 1024 * 1024 {
            let _ = fs::remove_file(&input); // large generated inputs are not retained
        }
        cases.push(obj(rec));
    }
    let untouched = sha256(&jpath) == jh0 && sha256(&mpath) == mh0;
    top.push(("cases", J::A(cases)));
    top.push(("json_md_companions_untouched", J::B(untouched)));
    let text = obj(top).dump();
    let text = if text.contains(&canary) { "{\"error\":\"results suppressed: contained canary\"}".to_string() } else { text };
    fs::write(out.join("results.json"), text).unwrap_or_else(|_| die("write results"));
    println!("t08: done");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("orchestrate") if args.len() >= 4 => {
            if std::process::id() != 1 {
                die("orchestrate must run as PID 1 of a dedicated PID namespace");
            }
            orchestrate(Path::new(&args[2]), Path::new(&args[3]));
        }
        Some("render") if args.len() == 5 => {
            let r = render(Path::new(&args[2]), &PathBuf::from(&args[3]), &args[4]);
            println!("{}", r.dump());
        }
        _ => die("usage: t08-pdf orchestrate <spike-root> <out-dir> [preflight-only] | (internal) render <in> <tmp-out> <fault>"),
    }
}
