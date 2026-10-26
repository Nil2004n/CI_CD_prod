//! P01-T06 text-safety tests (`planning/P01-T01-request.md` §5.3, §6.3).

use domain::text::{RedactedText, Redactor, SanitisedText, redact, sanitise};
use std::borrow::Cow;
use std::cell::RefCell;

/// The frozen escape set as inclusive ranges.
const ESCAPE_RANGES: &[(u32, u32)] = &[
    (0x0000, 0x0008),
    (0x000B, 0x001F),
    (0x007F, 0x007F),
    (0x0080, 0x009F),
    (0x061C, 0x061C),
    (0x200E, 0x200F),
    (0x202A, 0x202E),
    (0x2066, 0x2069),
    (0x200B, 0x200D),
    (0x2060, 0x2060),
    (0xFEFF, 0xFEFF),
    (0x2028, 0x2029),
];

const NEIGHBOURS: &[u32] = &[
    0x0009, 0x000A, 0x0020, 0x00A0, 0x061B, 0x202F, 0x2065, 0x206A, 0xFEFE, 0xFF00,
];

fn in_set(cp: u32) -> bool {
    ESCAPE_RANGES
        .iter()
        .any(|&(lo, hi)| (lo..=hi).contains(&cp))
}

fn ch(cp: u32) -> char {
    char::from_u32(cp).unwrap_or(char::REPLACEMENT_CHARACTER)
}

#[test]
fn every_escape_set_code_point_is_escaped() {
    let mut count = 0;
    for &(lo, hi) in ESCAPE_RANGES {
        for cp in lo..=hi {
            let input = format!("a{}b", ch(cp));
            let out = sanitise(&input);
            assert_eq!(out.as_str(), format!("a<U+{cp:04X}>b"), "U+{cp:04X}");
            count += 1;
        }
    }
    // 9 + 21 + 1 + 32 + 1 + 2 + 5 + 4 + 3 + 1 + 1 + 2
    assert_eq!(count, 82);
}

#[test]
fn escape_form_is_uppercase_hex_four_digits() {
    assert_eq!(sanitise("\u{0}").as_str(), "<U+0000>");
    assert_eq!(sanitise("\u{1b}").as_str(), "<U+001B>");
    assert_eq!(sanitise("\u{7f}").as_str(), "<U+007F>");
    assert_eq!(sanitise("\u{9f}").as_str(), "<U+009F>");
    assert_eq!(sanitise("\u{202e}").as_str(), "<U+202E>");
    assert_eq!(sanitise("\u{feff}").as_str(), "<U+FEFF>");
    assert_eq!(sanitise("\u{2028}").as_str(), "<U+2028>");
}

#[test]
fn neighbours_are_not_escaped() {
    for &cp in NEIGHBOURS {
        assert!(!in_set(cp), "test table error for U+{cp:04X}");
        let input = ch(cp).to_string();
        assert_eq!(sanitise(&input).as_str(), input, "U+{cp:04X}");
    }
}

#[test]
fn tab_and_newline_survive() {
    assert_eq!(sanitise("a\tb\nc").as_str(), "a\tb\nc");
}

#[test]
fn terminal_sequences_contain_no_raw_control() {
    let cases = [
        "\x1b[31mred\x1b[0m",
        "\x1b]0;title\x07",
        "\x1b]8;;http://example.invalid\x1b\\link\x1b]8;;\x1b\\",
        "\u{9b}2J",
        "\u{9d}0;title\u{9c}",
        "\x1bP1$r\x1b\\",
        "\r\x08\x08\x0c\x0b",
    ];
    for case in cases {
        let out = sanitise(case);
        for c in out.as_str().chars() {
            assert!(
                !in_set(u32::from(c)),
                "raw escape-set char U+{:04X} survived in {case:?}",
                u32::from(c)
            );
        }
        assert!(out.as_str().contains("<U+"));
    }
    assert_eq!(
        sanitise("\x1b[31m").as_str(),
        "<U+001B>[31m",
        "CSI is neutralised, printable remainder kept"
    );
    assert_eq!(sanitise("\u{9b}2J").as_str(), "<U+009B>2J");
}

#[test]
fn hangul_cjk_emoji_unchanged() {
    for s in [
        "한국어 텍스트",
        "漢字かな交じり文",
        "🙂🚀👩💻",
        "nbsp\u{a0}here",
        "é ü ß",
        "",
    ] {
        assert_eq!(sanitise(s).as_str(), s);
    }
}

#[test]
fn zwj_emoji_sequence_is_escaped_per_frozen_set() {
    // U+200D is in the frozen escape set, so ZWJ emoji sequences are deliberately not preserved.
    assert_eq!(sanitise("👩\u{200d}💻").as_str(), "👩<U+200D>💻");
}

#[test]
fn accessors_and_display() {
    let t = sanitise("a\u{1b}b");
    assert_eq!(t.as_str(), "a<U+001B>b");
    assert_eq!(t.len(), 10);
    assert!(!t.is_empty());
    assert_eq!(t.to_string(), "a<U+001B>b");
    assert!(sanitise("").is_empty());
    assert_eq!(sanitise("").len(), 0);
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Reduces `r` modulo `n` and narrows to `u32` (n is small and non-zero here).
fn below(r: u64, n: u64) -> u32 {
    u32::try_from(r % n).unwrap_or(0)
}

fn pick_char(state: &mut u64) -> char {
    let r = xorshift(state);
    let sel = r >> 8;
    match r % 6 {
        0 => {
            let (lo, hi) = ESCAPE_RANGES[below(sel, ESCAPE_RANGES.len() as u64) as usize];
            ch(lo + below(sel >> 8, u64::from(hi - lo + 1)))
        }
        1 => ch(NEIGHBOURS[below(sel, NEIGHBOURS.len() as u64) as usize]),
        2 => ch(0x20 + below(sel, 0x5F)),
        3 => ch(0xAC00 + below(sel, 0x2BA4)), // Hangul syllables
        4 => ch(0x1F300 + below(sel, 0x200)), // emoji block
        _ => ['<', '>', '+', 'U'][below(sel, 4) as usize],
    }
}

#[test]
fn idempotent_over_hand_corpus() {
    let corpus = [
        "",
        "plain",
        "\x1b[31m",
        "<U+001B>",
        "<U+<U+001B>>",
        "\u{202e}rtl\u{202c}",
        "a\u{200b}b\u{feff}",
        "한국어\u{2028}漢字\u{1b}🙂",
    ];
    for s in corpus {
        let once = sanitise(s);
        assert_eq!(sanitise(once.as_str()), once, "{s:?}");
    }
    // Disclosed ambiguity: literal "<U+001B>" equals an escaped ESC.
    assert_eq!(sanitise("<U+001B>"), sanitise("\u{1b}"));
}

#[test]
fn idempotent_over_seeded_generator() {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    for case in 0..10_000 {
        let len = below(xorshift(&mut state), 24) as usize;
        let input: String = (0..len).map(|_| pick_char(&mut state)).collect();
        let once = sanitise(&input);
        assert_eq!(sanitise(once.as_str()), once, "case {case}: {input:?}");
        for c in once.as_str().chars() {
            assert!(
                !in_set(u32::from(c)),
                "case {case}: raw U+{:04X}",
                u32::from(c)
            );
        }
    }
}

// ---- redact ----

struct Replace {
    from: &'static str,
    to: &'static str,
}

impl Redactor for Replace {
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        if raw.contains(self.from) {
            Cow::Owned(raw.replace(self.from, self.to))
        } else {
            Cow::Borrowed(raw)
        }
    }
}

struct Spy {
    seen: RefCell<Vec<String>>,
}

impl Redactor for Spy {
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        self.seen.borrow_mut().push(raw.to_owned());
        Cow::Borrowed(raw)
    }
}

struct Injector;

impl Redactor for Injector {
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("\x1b[31m{raw}\u{202e}\u{9b}"))
    }
}

#[test]
fn redactor_sees_raw_unescaped_text_and_output_is_escaped() {
    let spy = Spy {
        seen: RefCell::new(Vec::new()),
    };
    let input = "tok\x1b[0m\u{202e}en";
    let out: RedactedText = redact(input, &spy);
    assert_eq!(spy.seen.borrow().as_slice(), [input.to_owned()]);
    assert_eq!(out.as_str(), "tok<U+001B>[0m<U+202E>en");
}

#[test]
fn redactor_injected_controls_are_escaped() {
    let out = redact("x", &Injector);
    assert_eq!(out.as_str(), "<U+001B>[31mx<U+202E><U+009B>");
    assert!(out.as_str().chars().all(|c| !in_set(u32::from(c))));
}

#[test]
fn seeded_fake_marker_never_appears() {
    let marker = "FAKE-SECRET-0000";
    let r = Replace {
        from: marker,
        to: "[REDACTED]",
    };
    let input = format!("key={marker}\x1b[0m end");
    let out = redact(&input, &r);
    assert!(!out.as_str().contains(marker));
    assert!(!out.to_string().contains(marker));
    let json = serde_json::to_string(&out).unwrap();
    assert!(!json.contains(marker));
    assert!(!format!("{out:?}").contains(marker));
    assert_eq!(out.as_str(), "key=[REDACTED]<U+001B>[0m end");
}

#[test]
fn redacted_to_sanitised_and_accessors() {
    let r = Replace { from: "s", to: "" };
    let out = redact("a\u{1b}s", &r);
    let san: SanitisedText = out.to_sanitised();
    assert_eq!(san.as_str(), out.as_str());
    assert_eq!(san.as_str(), "a<U+001B>");
    assert_eq!(out.len(), san.len());
    assert!(!out.is_empty());
    assert_eq!(out.to_string(), "a<U+001B>");
    let spy = Spy {
        seen: RefCell::new(Vec::new()),
    };
    assert!(redact("", &spy).is_empty());
}

// ---- serde ----

#[test]
fn sanitised_deserialize_resanitises() {
    let t: SanitisedText = serde_json::from_str("\"\\u001b\"").unwrap();
    assert_eq!(t.as_str(), "<U+001B>");
    let t: SanitisedText = serde_json::from_str("\"a\\u202eb\\u2028\"").unwrap();
    assert_eq!(t.as_str(), "a<U+202E>b<U+2028>");
}

#[test]
fn sanitised_serde_round_trip_is_exact() {
    for s in [
        "",
        "plain",
        "\x1b[31m",
        "한국어\u{2028}🙂",
        "tab\tnl\n",
        "<U+001B>",
    ] {
        let t = sanitise(s);
        let json = serde_json::to_string(&t).unwrap();
        let back: SanitisedText = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t, "{s:?}");
    }
}

#[test]
fn serialises_as_plain_json_string() {
    assert_eq!(
        serde_json::to_string(&sanitise("a\u{1b}")).unwrap(),
        "\"a<U+001B>\""
    );
    let r = redact(
        "a\u{1b}",
        &Replace {
            from: "zzz",
            to: "",
        },
    );
    assert_eq!(serde_json::to_string(&r).unwrap(), "\"a<U+001B>\"");
}

#[test]
fn deserialize_rejects_non_string() {
    assert!(serde_json::from_str::<SanitisedText>("1").is_err());
    assert!(serde_json::from_str::<SanitisedText>("null").is_err());
}

/// Exhaustive check over every Unicode scalar value (P01-D04a, reviewer L1): `sanitise` must leave a
/// code point unchanged exactly when it is outside the frozen escape set and produce `<U+XXXX>` exactly
/// when it is inside. The oracle below is this test's own literal copy of the frozen set
/// (`planning/P01-T01-request.md` §5.3), written independently of `domain::text` internals and of
/// `ESCAPE_RANGES` above, so that over-escaping outside the set is detected, not just sampled.
#[test]
fn every_unicode_scalar_value_matches_frozen_escape_set() {
    fn oracle_escapes(c: char) -> bool {
        let u = c as u32;
        // C0 controls except TAB (0x09) and LF (0x0A), DEL, C1 controls.
        (u <= 0x1F && u != 0x09 && u != 0x0A)
            || u == 0x7F
            || (0x80..=0x9F).contains(&u)
            // Bidi controls.
            || u == 0x061C
            || u == 0x200E
            || u == 0x200F
            || (0x202A..=0x202E).contains(&u)
            || (0x2066..=0x2069).contains(&u)
            // Zero-width characters and BOM.
            || (0x200B..=0x200D).contains(&u)
            || u == 0x2060
            || u == 0xFEFF
            // Line and paragraph separators.
            || u == 0x2028
            || u == 0x2029
    }

    let mut scalars = 0_u32;
    let mut escaped = 0_u32;
    for cp in 0..=0x10FFFF_u32 {
        let Some(c) = char::from_u32(cp) else {
            continue; // surrogate range D800..=DFFF is not a scalar value
        };
        scalars += 1;
        let input = c.to_string();
        let out = sanitise(&input);
        if oracle_escapes(c) {
            escaped += 1;
            assert_eq!(
                out.as_str(),
                format!("<U+{:04X}>", cp),
                "U+{cp:04X} must be escaped"
            );
        } else {
            assert_eq!(out.as_str(), input, "U+{cp:04X} must pass unchanged");
        }
    }
    // 0x110000 code points minus 2048 surrogates; the independent oracle covers 82 escapes.
    assert_eq!(scalars, 0x11_0000 - 0x800);
    assert_eq!(escaped, 82);
}
