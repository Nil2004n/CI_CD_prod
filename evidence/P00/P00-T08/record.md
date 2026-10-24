# P00-T08 — stage A (resolution) and A2 (advisory screen)

Date: 2026-10-24. Authority: owner approval of T08 stages A and A2 only (resolution into the new spike member; determining and reporting the minimum printpdf feature set before any download; RustSec screen). Executed by Opus on `phase/P00-decisions-spikes` at base `f303360` (clean). Isolated Cargo environment as in T05/T06/T07 (rustc/cargo 1.97.1 toolchain binaries, `env -i`, isolated `CARGO_HOME`/`CARGO_TARGET_DIR`, wrapper script). No `.cargo/config*` in any ancestor.

**Network used:** the crates.io sparse index (resolution), plus `api.github.com` and `raw.githubusercontent.com` for the rustsec/advisory-db listing and advisory files only. **Not done:** no crate or source download (`cargo fetch` not run; no `.crate` files for the new packages), no build, no conversion, no WeasyPrint execution, no install.

## A — resolution
- New member `spikes/rust/t08-pdf` (empty placeholder `main`). Dependencies are `pulldown-cmark = { version = "=0.13.5", default-features = false }` and `printpdf = { version = "=0.12.8", default-features = false }`, with **no features enabled**. Workspace members are now `t05-process`, `t06-yaml`, `t07-keyring`, `t08-pdf`.
- `cargo update --workspace --dry-run` → exit 0, "Adding" only (`A-resolve-dryrun.log`); then `cargo update --workspace` → exit 0 (`A-resolve.log`).
- Lock 179 → 235 packages (+56: 55 registry plus the local member); sources crates.io and local only (`A-lock-diff.txt`). `Cargo.lock` SHA-256 `a81eec56…e21b9`; before/after copies kept.
- **No existing package version changed or was removed.** Four existing entries changed only in their dependency lists:

  | Existing entry | Change | Cause (index metadata) | Graphs that could be affected |
  |---|---|---|---|
  | `getrandom 0.4.3` (T07) | + `rand_core`, + `js-sys`, `wasm-bindgen` | `rand 0.10.3` (via `lopdf`) enables getrandom's `sys_rng` → `rand_core`; the js/wasm edges are wasm-only | T07 only if built in the same Cargo invocation as T08 (feature unification); host confirmation needs `cargo tree` (stage B) |
  | `smallvec 1.16.4` (T06 graph) | + `serde` | `printpdf` depends on `smallvec` with `features = ["serde"]` (non-optional) | T06 only if built together with T08 |
  | `strum_macros 0.28.0` | `heck` → `heck 0.5.0` | version qualifier only (second `heck 0.4.1` added for `ouroboros_macro`) | none |
  | `tempfile 3.27.0` | `getrandom 0.3.4` → `getrandom 0.4.3` | tempfile accepts `>=0.3, <0.5`; Cargo re-resolved the edge to the version now present | none so far: `tempfile` is not in the T05/T06/T07 host graphs (T07 B: not fetched, absent from host tree) |
- Per-package builds (`-p`), the established rule, are expected to keep the T05–T07 host features unchanged. This is **not proven at stage A**: `cargo tree` needs crate sources, so the T06/T07 comparison is deferred to stage B, as for T07.

### New packages (55 registry)
`adler2 2.0.1, aliasable 0.1.3, alloc-no-stdlib 2.0.4, alloc-stdlib 0.2.4, allsorts-azul 0.17.2, base64 0.22.1, brotli-decompressor 5.0.3, chacha20 0.10.2, core_detect 1.0.0, crc32fast 1.5.3, deranged 0.5.9, ecb 0.2.1, encoding_rs 0.8.42, flate2 1.1.10, glyph-names 0.2.0, heck 0.4.1, lopdf 0.44.0, md-5 0.11.0, miniz_oxide 0.9.1, multiversion_no_op 1.0.0, nom 8.0.0, num-conv 0.2.2, ouroboros 0.18.5, ouroboros_macro 0.18.5, pathfinder_geometry 0.5.1, pathfinder_simd 0.5.6, powerfmt 0.2.1, printpdf 0.12.8, proc-macro2-diagnostics 0.10.1, pulldown-cmark 0.13.5, rand 0.10.3, rand_core 0.10.1, rangemap 1.8.0, rustc-hash 2.1.3, rustc_version 0.4.1, semver 1.0.28, simd-adler32 0.3.10, simdutf8 0.1.5, static_assertions 1.1.0, stringprep 0.1.5, time 0.3.55, time-core 0.1.9, tinyvec 1.13.3, tokio 1.54.1, ucd-trie 0.1.7, unicase 2.10.0, unicode-bidi 0.3.18, unicode-canonical-combining-class 1.0.0, unicode-joining-type 1.0.0, unicode-normalization 0.1.26, unicode-properties 0.1.4, wasm-bindgen-futures 0.4.79, weezl 0.2.1, yansi 1.0.1, zlib-rs 0.6.8`

### Graph and feature review (cached sparse-index metadata only, `A-index-features.txt`; no sources)
- **printpdf 0.12.8 with no features:** non-optional dependencies are `allsorts` (= `allsorts-azul 0.17.2`, `default-features = false`, `features = ["flate2_rust"]`), `lopdf 0.44.0` (`default-features = false`), `base64`, `flate2`, `serde`/`serde_derive`/`serde_json`, `smallvec` (`serde`), `time` (`std`) and `weezl`; plus wasm-only `wasm-bindgen`/`getrandom`/`js-sys`/`wasm-bindgen-futures`.
- **Optional printpdf features not enabled:**
  - `html` (default): `xmlparser`, `rust-fontconfig`, `azul-layout` XML/PDF;
  - `text_layout`: `azul-css`, `azul-core`, `azul-layout`, and **`rust-fontconfig` with `font_loading`**, i.e. system font discovery;
  - `text_layout_hyphenation`;
  - `images` and all codec features: `image`;
  - `svg`: `svg2pdf`;
  - `js-sys`, `html_multithreaded`, `rayon`.
- **lopdf 0.44.0 with defaults off:** non-optional `aes`, `cbc`, `ecb`, `md-5`, `sha2`, `rand`, `stringprep` (PDF encryption/security handler support), plus `flate2`, `nom`, `encoding_rs`, `indexmap`, `itoa`, `log`, `rangemap`, `thiserror`, `weezl`, `bitflags`, `getrandom ^0.4`. Optional and not enabled: `chrono`, `jiff`, `rayon`, `time`, `image`, `ttf-parser` (`font_embedding`), `tokio` (`async`), `serde`.
- **allsorts-azul 0.17.2** (`flate2_rust` only; its default `flate2_zlib` is off): an OpenType parsing, subsetting and shaping engine (`unicode-joining-type`, `unicode-canonical-combining-class`, `unicode-general-category`, `glyph-names`, `brotli-decompressor` for WOFF2, `ouroboros` self-referencing structs).
- **`tokio 1.54.1`** is in the lock **only** through `wasm-bindgen-futures 0.4.79` (wasm-only), with dependencies `[pin-project-lite]` only. It is not expected in the Linux host graph; *verify in B*.
- **flate2 1.1.10** lists both `miniz_oxide` and `zlib-rs` (both pure Rust). No `libz-sys` or C zlib is in the lock.
- **Name screen of the full lock** (a superset of the host graph):
  - none of the new packages is an HTTP/TLS client (`reqwest`, `ureq`, `hyper`, `rustls`, `native-tls`, `openssl`), `libz-sys`, `cc`, `pkg-config`, `cmake`, `bindgen`, `fontconfig`/`freetype` bindings or an `image` codec;
  - no new `-sys` crate, and no new package declares `links` (checked in the index metadata for printpdf, lopdf, allsorts-azul, tokio);
  - `tokio` and `wasm-bindgen-futures` are present only on the wasm path.
- The stop conditions (native/C, vendored, TLS, existing-version change, applicable advisory) are **not triggered** on the index-metadata evidence.

### Proposed minimum printpdf feature set — **none** (`default-features = false`, `features = []`)
- **Rationale:** with no features, printpdf still depends non-optionally on `allsorts-azul`, a font parser, subsetter and shaper. Its `text_layout` feature would add the azul layout stack and **`rust-fontconfig` font loading (system font discovery)**, contrary to the allowlisted-font-path policy. `html`, `images` and `svg` add parsers, codecs and layout outside the subset.
- **Still to verify in B (source review, no build):** that the featureless printpdf API exposes font parsing from bytes, subsetting/embedding and glyph metrics sufficient for spike-owned wrapping, and whether allsorts shaping (Arabic joining, GSUB/GPOS) is reachable without `text_layout`.
- If not, stage B stops and proposes the exact feature amendment, for example `text_layout` with an assessment of `rust-fontconfig`. **No feature is added under this approval.**
- pulldown-cmark: no features (`getopts`, `html`, `simd` off). Tables are enabled at run time via parser `Options`, not a Cargo feature.

## A2 — RustSec screen (`A2-advisories.txt`, raw advisory files in `A2-advisories/`)
- **Listing:** advisory-db `crates/` via the GitHub contents API, HTTP 200, 967 entries (below the 1000 cap). The raw-response SHA-256 `9ea53553…a770` is identical to the T07 listing; the raw file is not committed (names are in `A2-crates-listing-names.txt`).
- **Screened:** the 55 new registry crate/versions; 9 have advisory directories. Every advisory file was fetched (all HTTP 200), its TOML `[versions]` evaluated against the locked version, and the result checked by hand.

| Crate (locked) | Advisories | Result |
|---|---|---|
| base64 0.22.1 | RUSTSEC-2017-0004 | patched (≥0.5.2) |
| chacha20 0.10.2 | RUSTSEC-2019-0029 | patched (≥0.2.3) |
| lopdf 0.44.0 | RUSTSEC-2026-0187 (2026-06-21) | patched (≥0.42.0) |
| ouroboros 0.18.5 | RUSTSEC-2023-0042 (unsound) | patched (≥0.16.0) |
| rand 0.10.3 | RUSTSEC-2026-0097 (unsound) | patched (≥0.10.1) |
| rand_core 0.10.1 | RUSTSEC-2019-0035 (unsound), RUSTSEC-2021-0023 | patched |
| time 0.3.55 | RUSTSEC-2020-0071, RUSTSEC-2026-0009 | patched (≥0.3.47) |
| tokio 1.54.1 (wasm path only) | RUSTSEC-2021-0072, -2021-0124, -2023-0001, -2023-0005 (unsound), -2025-0023 (unsound), **-2026-0384 (2026-10-21; patched `>= 1.54.1`, exactly at the bound — hand-checked)**, -2026-0386 (unsound) | all patched |
| zlib-rs 0.6.8 | RUSTSEC-2024-0401 | patched (≥0.4.0) |

**Result:** no applicable advisory for any new crate/version; no new exception is needed. The T07 exception does not extend to T08.

**Scope and limitations:**
- Published advisories only; absence of an advisory is not evidence of safety. Yanked status was checked only for printpdf, pulldown-cmark, lopdf, allsorts-azul, tokio, getrandom, smallvec and tempfile (all not yanked).
- Existing T05–T07 crates were not re-screened in this stage.
- A new advisory inside an existing crate directory does not change the listing hash; that is why every directory matching a new crate was fetched individually.

## Not done (needs separate approval)
- **Stage B:** `cargo fetch --locked`, checksums, lock stability, host `cargo tree` and T06/T07 unification delta, build-exposure inventory, printpdf/allsorts/lopdf source review (font API, file/network APIs, annotation/action/JavaScript emitters, encryption paths).
- **Stages C and D.**
- **WeasyPrint:** suspended by the owner-reported caveat (request §1).

## Owner decision and stage B (2026-10-24) — appended; A/A2 above unchanged

Owner accepted the featureless configuration **for stage-B inspection** (pulldown-cmark =0.13.5 and printpdf =0.12.8, both `default-features = false`, no features). The four lock-entry changes are provisionally accepted as recorded resolution artifacts. This does not accept unverified changes to earlier spikes' active features, and it does not establish that the featureless API meets the PDF requirements. Stage B approved: locked fetch from static.crates.io, checksums, host graph, earlier-spike comparison, build exposure, source/API review. No compilation, build-script or proc-macro execution, harness, conversion or renderer execution.

### Fetch and checksums (`B-fetch.log`, `B-checksums.txt`)
- Before the fetch: `sha256sum -c lock-after-A.sha256` OK; hashes of `Cargo.lock`, the workspace `Cargo.toml` and all member manifests recorded (`hashes-before-B.txt`).
- `cargo fetch --locked --target x86_64-unknown-linux-gnu` → exit 0, 54 downloads. These are 53 new T08 crates plus `rustversion 1.0.23`, a pre-existing lock entry not previously fetched and now needed by `pathfinder_simd`/`pulldown-cmark`.
- **Lock and manifests unchanged** (`hashes-after-B.txt` = before).
- Independent SHA-256 of each cached `.crate` against `Cargo.lock`:
  - **New T08 entries:** 53 verified, 0 mismatches. **2 not downloaded and therefore NOT checksum-verified:** `tokio 1.54.1` and `wasm-bindgen-futures 0.4.79`. They are wasm-only (`cfg(target_arch="wasm32", target_os="unknown")` path) and absent from the x86_64 host graph.
  - **Pre-existing entries:** 150 verified (149 from T07 plus `rustversion`), 0 mismatches. 26 not downloaded and not verified (Apple/Windows/wasm/UEFI/hermit/redox crates, plus `tempfile`, `zeroize`, `memoffset`, `byteorder`, `bumpalo`, `zerocopy-derive`, which are not in any host graph).

### Host graph (`B-t08-host-features.txt`, `B-t08-host-tree.txt`)
- `t08-pdf` host graph: 95 packages (excluding the root), 53 not in the T05–T07 host graphs.
- Active features of interest:
  - `printpdf []`, `pulldown-cmark []`, `lopdf []`;
  - `allsorts-azul [flate2, flate2_rust]`;
  - `flate2 [any_impl, default, miniz_oxide, runtime_detection, rust_backend]` (miniz_oxide; **`zlib-rs` not active**);
  - `time [alloc, std]`; `serde`/`serde_json [default, std]`;
  - `rand 0.10.3 [alloc, default, std, std_rng, sys_rng, thread_rng]` → `getrandom 0.4.3 [std, sys_rng]`;
  - PDF security-handler crates are present because lopdf requires them non-optionally: `aes`, `cbc`, `ecb`, `md-5`, `sha2`, `chacha20`, `stringprep`/`unicode-bidi`.
- **Absent from the host graph:** `tokio`, `wasm-bindgen-futures`, `zlib-rs`, `azul-*`, `rust-fontconfig`, `xmlparser`, `image`, `svg2pdf`, any HTTP/TLS client, `libz-sys`/`cc`/`pkg-config`. No package declares `links`.
- A first `cargo tree` attempt exited 1 because zsh did not word-split an options variable (same trap as T06/T07). Its empty outputs, and the "DIFFERS" comparison made on them, were discarded. The trees were regenerated with a bash array script.

### Earlier-spike comparison (`B-earlier-spike-comparison.txt`; `B-t0{5,6,7}-host-features-now.txt`; `B-t0{5,6,7}+t08-unified-features.txt`; `B-workspace-host-features.txt`)
- **Package-scoped graphs after T08:**
  - **T06** (`-p t06-yaml`) is **identical** to the accepted `evidence/P00/P00-T06/B-tree-features.txt`: 70 packages, same versions and features (after removing `(*)` markers).
  - **T07** (`-p t07-keyring`) is **identical** to the T07 stage-B graph `evidence/P00/P00-T07/B-t07-host-features.txt`: 102 packages.
  - **T05** (`-p t05-process`) is `libc 0.2.191 [default, std]` only, matching D3-1.
  - **The earlier acceptances' package-scoped assumptions remain valid on this evidence.**
- **Features activated only by a combined build** (one Cargo invocation including `t08-pdf`):
  - T05+T08: none.
  - T06+T08: `smallvec 1.16.4 +serde`, `syn 2.0.119 +full`, `thiserror 2.0.21 +default,std`.
  - T07+T08: `getrandom 0.4.3 +std,sys_rng`, `syn 2.0.119 +default`.
  - `--workspace` (all four): T06 crates gain `num +default`, `once_cell +default,std`, `smallvec +serde`, `syn` 2/3 extra features, `thiserror +default,std`. T07 crates gain `getrandom +std,sys_rng`, `hashbrown 0.17.1` (+allocator-api2, default, default-hasher, equivalent, inline-more, raw-entry), `num-bigint +default`, `num-traits +default`, `serde`/`serde_core +alloc`, `syn +default`.
  - No combined build enables HTTP/file retrieval, TLS or a native dependency.
  - **Remedy:** keep building per package (`-p`). A combined build would require re-running the affected spike's suites with the unified features: T06 D01 + J16; T07 K-cases.
- **The four lockfile-wide changes explained:**
  1. **`getrandom 0.4.3` + `rand_core`:** `lopdf 0.44.0` requires `rand ^0.10` with default features. `rand 0.10.3`'s defaults include `sys_rng`, which enables `dep:getrandom` and `getrandom/sys_rng`; that in turn enables getrandom's optional `rand_core`. The lockfile is resolution-wide, so the edge appears there. It is **active only in the T08 graph** and in combined T07+T08 builds. In T07 alone, `getrandom 0.4.3` features are `[]`, as accepted.
  2. **`smallvec 1.16.4` + `serde`:** `printpdf` declares `smallvec ^1.15.1` with `features = ["serde"]` (non-optional). Active in T08 and in combined T06+T08 builds; T06 alone has `smallvec []`, as accepted.
  3. **`strum_macros 0.28.0` qualifier:** `ouroboros_macro 0.18.5` requires `heck = "0.4.1"`, adding `heck 0.4.1` alongside the existing `heck 0.5.0`. The lockfile then disambiguates `strum_macros`' dependency as `heck 0.5.0`. The version is unchanged, so behaviour is unchanged.
  4. **`tempfile 3.27.0`** declares `getrandom = ">=0.3.0, <0.5"` (optional, default feature `getrandom`, target `cfg(any(unix, windows, target_os = "wasi"))`), so both 0.3.4 and 0.4.3 satisfy it. In this resolution Cargo selected 0.4.3. The exact resolver reason for the change during `cargo update --workspace` was not established from the evidence; the requirement permits either. `tempfile`'s only dependent in the lock is `uds_windows 1.2.1` (Windows-only), so **`tempfile` is in no Linux host graph** of any spike.

### Build-time exposure (`B-build-exposure.txt`, `B-pattern-review.txt`; grep-based, not a security audit)
- **T08 host graph: 18 build scripts.** 10 were already in the T05–T07 graphs (`getrandom 0.4.3`, `libc`, `proc-macro2`, `quote`, `serde`, `serde_core`, `serde_json`, `thiserror`, `unicode-general-category`, `zmij`). **8 are new:**
  - `crc32fast 1.5.3` runs `$RUSTC --version` and emits cfgs.
  - `encoding_rs 0.8.42` emits a cfg via `rustversion::cfg!`.
  - `pathfinder_simd 0.5.6` uses `rustc_version` (runs `rustc -vV`) and emits a cfg on nightly.
  - `proc-macro2-diagnostics 0.10.1` uses `version_check` (runs rustc) and emits a cfg.
  - `pulldown-cmark 0.13.5`: its spec-test generator (reads `./specs`, creates files) is compiled only with feature `gen-tests`, which is off; otherwise `main` is empty.
  - `rustversion 1.0.23` (`build/build.rs`) runs `$RUSTC`/`RUSTC_WRAPPER --version` and writes `OUT_DIR/version.expr`.
  - `unicode-canonical-combining-class 1.0.0` and `unicode-joining-type 1.0.0` generate tables into `OUT_DIR`.
- **6 proc-macro crates.** 3 were already present (`enumflags2_derive`, `serde_derive`, `thiserror-impl`). **3 are new:** `multiversion_no_op 1.0.0` (43 lines), `ouroboros_macro 0.18.5` (2,339 lines) and `rustversion 1.0.23` (1,029 lines; reads `CARGO_PKG_NAME`; includes `OUT_DIR/version.expr`). The pattern grep found no process spawning, file I/O or networking in their `src/` beyond that.
- No native compilation or linking (`cc`, `pkg-config`, `rustc-link-*`), no system discovery, and no network patterns in the new build code. There is no filesystem sandbox when these run, and grep absence is not proof.

### Featureless API review (printpdf 0.12.8 sources; observations only — nothing compiled or run)
| Question | Finding | Source |
|---|---|---|
| Load only explicitly supplied font bytes | **Yes.** Featureless `ParsedFont::from_bytes(bytes, index, &mut warnings)` parses cmap/head/hhea/hmtx/maxp/OS2 with allsorts from caller-supplied bytes. No font discovery: `rust-fontconfig` is only behind `text_layout`/`html` | `src/font.rs` 220–470 (`#[cfg(not(feature = "text_layout"))]`) |
| Embedding | **Yes, whole font.** Serialisation embeds `parsed_font.original_bytes` (a TTC face is extracted to a standalone sfnt). If the bytes are empty it warns and does not embed | `src/serialize.rs` 1716–1760, `font.rs` `extract_collection_face` |
| **Subsetting** | **Not available featureless.** `subset_font` returns the original bytes unchanged, and the runtime always uses `create_full_font_runtime_info`, so `PdfSaveOptions.subset_fonts` has no effect. Whole fonts are embedded: DejaVuSans ≈ 0.76 MB, DroidSansFallbackFull ≈ 4.0 MB, NotoSansMono ≈ 0.51 MB, NotoColorEmoji ≈ 11.1 MB, before stream compression | `font.rs` 967–975, `serialize.rs` 1592–1607 |
| Glyph advances for wrapping | **Yes.** `lookup_glyph_index(codepoint) -> Option<u16>` and `get_glyph_width(gid) -> Option<u16>` (font units), with `units_per_em` in `pdf_font_metrics`. Missing glyphs are detectable (`None`) | `font.rs` 447–470, `serialize.rs` 1833–1845 |
| Text and page operations | **Yes.** `Op::StartTextSection`/`EndTextSection`, `SetFont { font, size }`, `SetTextCursor`, `SetLineHeight`, `ShowText { items: Vec<TextItem> }` (`TextItem::Text`, `GlyphIds`, `Offset`), `SetFillColor`, `DrawLine`, `DrawPolygon` | `src/ops.rs` 425–470 |
| Write without resource discovery or fetching | **Yes, on source evidence:** no `std::fs`, `std::net`, process or env access in the featureless library paths (the only `std::fs::write` calls are in `font.rs` `mod test`, from line 1704). Builtin fonts are `include_bytes!` data. Behaviour still needs a run in a network namespace (D) | grep of `src/` (excluding `html/`, `wasm/` and tests) |
| Font contains required glyphs | A property of the chosen font, not of the API. Checkable via `lookup_glyph_index`; to be measured in D | — |
| **Shaping reachable** | **Not featureless.** `text_shaping` (azul `text3`) is behind `text_layout`, and printpdf does not re-export `allsorts`, so GSUB/GPOS (Arabic joining, mark positioning, ligatures) needs either `text_layout` (which adds azul-layout and **rust-fontconfig font loading**) or a new direct `allsorts-azul` dependency. Neither is approved | `src/lib.rs` 72–81 |
| Bidi / layout | **Not provided featureless.** `unicode-bidi` is in the graph only through `stringprep` (lopdf) and is not exposed. Line breaking, bidi reordering and pagination are spike-owned | — |
| Extraction / Unicode mapping | **Yes, for text written via codepoints.** The full-font path generates a ToUnicode CMap from the (gid, char) usage (`generate_cmap_string`). For `TextItem::GlyphIds` the caller supplies the char mapping. Actual extraction correctness needs a D test | `serialize.rs` 1796–1810, `font.rs` 977–987 |
| Visual correctness | Not answerable at B; needs approved C/D runs | — |

**Forbidden-object APIs to avoid (harness) and to scan for (D):**
- `Op::LinkAnnotation` / `PdfDocument::add_link` with `annotation::Actions::{Goto, Uri}` emits `/Annots` and `/URI` (`ops.rs` 476–478, `annotation.rs` 133–160, `serialize.rs` 332, 2220). printpdf exposes no JavaScript, Launch, SubmitForm or EmbeddedFile action constructors (the `Actions` enum is `Goto | Uri` only); the doc comment listing them is spec text.
- **XMP metadata** (`/Metadata`) is emitted only when `conformance.must_have_xmp_metadata()`. The default `PdfConformance::Custom(CustomPdfConformance::default())` has `requires_xmp_metadata: false` and `allows_embedded_javascript: false` (`serialize.rs` 150–158, `conformance.rs` 65–69, 138–146). The harness keeps the default and does not set `metadata.xmp`.
- The document info dictionary and trailer `/ID` are always written. IDs come from a deterministic counter-based generator (`utils.rs` 14–40), not a secure RNG; this is not a privacy concern for local reports, but it is recorded.
- `PdfSaveOptions.secure` means "skip unknown operations", not encryption. printpdf calls no lopdf encryption API (grep `encrypt` in `src/`: none). The security-handler crates are compiled in but unused; their presence does not permit enabling encryption.
- Optional content (layers, `OCProperties`), ICC output intents and the image/SVG/HTML paths are not used. Images and SVG are compiled out.

### Stop-condition assessment
- No new advisory: the graph is unchanged since A2.
- No unexpected native or system dependency or execution exposure.
- The earlier spikes' package-scoped assumptions remain valid.
- **Capability gaps found, for owner decision before C:**
  1. **No font subsetting featureless.** Whole fonts are embedded, so each report PDF is several MB. This is within the request's 64 MiB output limit, so not a blocker under the request's pass criteria.
  2. **No shaping or bidi featureless.** Under the request (§4 U1), RTL/Arabic shaping and correct combining-mark positioning may be a documented visible limitation, and only Latin and CJK legibility are required. So this is not a blocker for the stated criteria either.
- If the owner considers subsetting or shaping **required**, that is a stop condition. The smallest amendment candidates (none applied) would be:
  - (a) a direct `allsorts-azul =0.17.2` dependency (already in the graph, same `flate2_rust` feature) for harness-side subsetting/shaping, which would need a further API review;
  - (b) printpdf `text_layout`, which adds azul-layout plus rust-fontconfig font loading and conflicts with the allowlisted-font policy.
- No checkpoint includes any remedy.

### Remaining questions for C/D
- Whether ToUnicode extraction round-trips for CJK and combining marks.
- Actual PDF size and time with whole-font embedding (G1).
- Whether `.notdef` occurs for any allowlisted-font codepoint in the U1 fixtures.
- Whether lopdf serialisation adds any object the forbidden-object scan flags.
- Whether `optimize=true` compression keeps output within limits.
- Visual correctness of spike-owned wrapping and pagination.
- WeasyPrint remains suspended; no research, upgrade or execution.

## Owner decision, stage C and formal stage D (2026-10-24) — appended; A/A2/B above unchanged

**Owner decision:** keep the featureless configuration for the experiment.
- Provisionally accepted: whole-font embedding within existing limits; no complex-script shaping or RTL layout, as an explicit experimental limitation. Neither is accepted for the final product.
- Not to be done: add `allsorts-azul`, enable `text_layout`, or add another engine.
- Latin/CJK requirements, extraction checks and other mandatory criteria must not be weakened.
- Approved: stage C, then stage D (the eleven P1 cases). WeasyPrint stays suspended.

### Implementation agent — disclosure
`CLAUDE.md` now directs implementation to the `sonnet-implementer` sub-agent. A spawn attempt returned "Agent type 'sonnet-implementer' not found" because the definition is not loaded in this session (it needs a new session). No generic agent was substituted with an unpinned alias. **The harness was written by Opus as a disclosed fallback.** No worker was used, and the worker model remains unverified.

### C — implementation and build (`C/`)
- Harness `spikes/rust/t08-pdf/src/main.rs` (std + pulldown-cmark + printpdf only); scripts `spikes/rust/t08-pdf/run/{outer.sh,setup-and-run.sh}`.
- Design fixes made before any run and before any output was observed: T1 path column moved last (to avoid extraction interleaving of multi-line cells); row measurement no longer double-counts statistics; unused fields removed.
- `unshare -rn <isolated cargo> build --frozen --offline -p t08-pdf`:
  - attempt 1 exit 101 (`C-build-1.log`): 11 compile errors in harness code (missing lifetime on `layout_markdown`, with cascading type errors) and 2 warnings. All dependency build scripts and proc-macros ran during this attempt, inside the network namespace;
  - fixed;
  - attempt 2 exit 0, **0 warnings** (`C-build-2.log`).
- 71 packages compiled (the rest were cached from earlier spikes), all in the reviewed T08 host graph (`C/compiled.txt`). No download or index update. Manifests and `Cargo.lock` unchanged (`C/hashes-before.txt` = `C/hashes-after.txt`).
- Artifact `2905abc9…ed564f3`.
- Build-time code ran **without a filesystem sandbox**; pattern review and a stripped environment do not prove otherwise.

### D — isolation and formal run (`D/preflight/`, `D/run1/`)
- **Command:** `outer.sh` (outside) → `timeout -s KILL 900 unshare -r -n --pid --fork --mount-proc --kill-child env -i PATH LANG bash setup-and-run.sh` (PID 1 check, root prefix, artifact SHA-256, `ulimit -v 2097152`) → `exec env -i` harness as PID 1.
- **Preflight 11/11 passed** before any conversion:
  - PID 1; network namespace loopback only; inherited environment exactly `{LANG, PATH}`;
  - spike root 0700 with the expected prefix; address-space limit 2 GiB;
  - each allowlisted font is a regular file whose SHA-256 inside equals the hash recorded outside: DejaVuSans `bb01e8ee…3481`, DroidSansFallbackFull `acb6440a…77b8`, NotoSansMono-Regular `6b692c4b…e29f`;
  - `/usr/bin/mutool` and `/usr/bin/sha256sum` present.
  - Font hashes were unchanged after the run.
- **Resource-loading routes (code review):** the harness opens only the three allowlisted font paths, its own fixture/output files under the spike root, `/proc/self/{status,limits,net/dev}` and `/dev/urandom`. `mutool` is given only the temp PDF and spike-owned output paths. Private `HOME` under the spike root. No other tool was used.
- Formal run `D/run1/`: 11 s wall in total; spike root removed; fonts unchanged.

### Formal results (run1) — the run FAILED its stated criteria
**Overall: no supported-content PDF passed the six-step verification.** Every PDF-producing case (U1-latin-cjk, U1-unsupported, U2, T1, B1, R1, L1, A1, G1, C1) failed **step 5**, the forbidden-object scan, on the name **`/Annots`**. U1-latin-cjk also failed **step 4**. Per the request's failure behaviour (§7), temp PDFs of failed cases were removed, so **no PDF was written or retained**. The artifact directory is empty and there is no `artifact-pdfs.sha256`.

| Case | Render | Steps 1–3 | Step 4 (extraction) | Step 5 | Verdict |
|---|---|---|---|---|---|
| U1-latin-cjk (supported) | ok, 1 p, 2,525,895 B | pass | **FAIL**: sentinel `한국어` missing (Hangul shown as `<U+D55C><U+AD6D><U+C5B4> <U+C2DC><U+D5D8>`); 10/11 sentinels incl. Latin and Han/Kana present | **FAIL** `/Annots` | verification_failed |
| U1-unsupported | ok, 1 p, 387,359 B | pass | pass (RTL marker, emoji code points) | **FAIL** `/Annots` | verification_failed |
| U2 | ok, 1 p, 387,086 B | pass | pass (all 6 controls shown as code points) | **FAIL** `/Annots` | verification_failed |
| T1 | ok, 13 p, 429,270 B | pass (13 in 8–60) | pass (7/7, incl. 200-char path) | **FAIL** `/Annots` | verification_failed |
| B1 | ok, 6 p, 651,180 B | pass | pass (4/4) | **FAIL** `/Annots` | verification_failed |
| R1 | ok, 1 p, 387,034 B | pass | pass | **FAIL** `/Annots`; 0 image objects | verification_failed |
| L1 | ok, 1 p, 386,956 B | pass | pass | **FAIL** `/Annots`; 0 image objects | verification_failed |
| A1 | ok, 1 p, 387,070 B | pass | pass (script/iframe/form/`javascript:` as literal text) | **FAIL** `/Annots`; no JavaScript/JS/Launch/URI/SubmitForm/AcroForm/OpenAction/AA names | verification_failed |
| G1 | ok, 401 p, 833,390 B | pass (401 in 50–1500) | pass (3/3) | **FAIL** `/Annots` | verification_failed |
| C1 | ok, 1 p, 386,822 B | pass | pass (canary present, as required) | **FAIL** `/Annots` | verification_failed; canary artefacts removed |
| X1a input > 8 MiB | `limit_exceeded:input_bytes` | — | — | — | expected typed failure, no PDF |
| X1b nesting 17 | `limit_exceeded:nesting` | — | — | — | expected typed failure, no PDF |
| X1c 33 columns | `limit_exceeded:table_columns` | — | — | — | expected typed failure, no PDF |
| X1d > 2,000 pages | `limit_exceeded:pages` | — | — | — | expected typed failure, no PDF |
| X1e list depth 4 | `limit_exceeded:list_depth` | — | — | — | expected typed failure, no PDF |
| F1a font fault | `font_unavailable:DejaVuSans:parse_failed` | — | — | — | expected typed failure, no PDF |
| F1b write fault | `write_failed:entity not found`, `temp_removed` | — | — | — | expected typed failure, no PDF; JSON/MD companions unchanged (hash check) |

In steps 3–5, no case showed parser repair or error output, encryption reported by `mutool info`, image XObjects or object streams. Every `mutool draw` printed `warning: ICC support is not available` (a tool build note, not a repair); it is recorded in the per-case stderr. No other name in the forbidden list was found. `/Metadata` was also absent from both the original and the normalised file; note that `clean -d` without `-m` may drop metadata, so the original-file scan is the relevant one, and objects are not in object streams.

### Diagnosis (confirmed vs hypothesis)
- **D-A (confirmed from source; runtime content not inspected).** printpdf 0.12.8 writes `("Annots", Array(links))` unconditionally in every page dictionary (`src/serialize.rs` ~332). `links` is populated only from `Op::LinkAnnotation`, which the harness never emits, so the array is expected to be **empty**. This could not be shown at runtime: the normalised and temp PDFs were deleted on failure, as designed. The request's P1 criterion "also no `/Annots`" is therefore unattainable with this library without an output-level change. It is a criterion/library mismatch, not a sign of annotations being emitted.
- **D-B (confirmed).** The allowlisted fonts have **no Hangul syllable glyphs**: DroidSansFallbackFull covers the Han/Kana fixtures but not U+AC00–D7A3. Korean, which the fixture includes in "supported CJK", fails extraction and appears as visible code points. No other allowlisted or installed font directory lists a CJK family (`/usr/share/fonts/truetype`: adwaita, dejavu, droid, firacode, font-awesome, freefont, hack, lato, liberation, lyx, noto (mono/emoji only), quicksand). Hangul coverage in any of them is not verified.
- **H-1 (confirmed harness defect, mine).** `is_rtl` includes `U+FE70..=U+FEFF`, so the BOM U+FEFF counts as RTL. U2 (controls) therefore received a spurious `[complex script/RTL not shaped]` marker; the extraction shows it. It did not change any step result, but it is incorrect output. Bounded fix: the range should end at U+FEFC.
- **H-2 (process defect, mine).** On verification failure the temp PDF and normalised file are removed (request §7). That leaves no artefact with which to show D-A at runtime. The textual evidence (results, tool logs, extraction excerpts) is preserved.
- **O-1 (observation; cause not established).** G1's layout model reports `model_max_right_overflow_pt = 1.185`: one line extends 1.2 pt past the right margin, well inside the 50 pt page margin, so it is not clipped at the page edge. Hypothesis (unverified): the line-width accounting differs between the fit test (word width without its trailing space) and the emitted width, or font-advance rounding. All other cases report ≤ 0.

### Typography verdicts (run1) — extraction ≠ visual; visual **unverified for every case**
Rendering pages to images is not in the approved tool list (`mutool info`, `draw -F txt`, `clean -d`). No page was inspected visually, so visual legibility, overlap and mark positioning are **unverified**, not passed.

| Fixture | Glyph coverage | Extraction vs expectation | Visual | Wrapping / clipping / page and table boundaries (layout model) |
|---|---|---|---|---|
| U1 Latin precomposed (supported) | all in DejaVuSans; 0 missing | all Latin sentinels present (Ærøskøbing, café, façade, Łódź, Šibenik, …) | unverified | 1 page; no overflow |
| U1 CJK Han/Kana (supported) | 20 chars from DroidSansFallbackFull | 漢字テスト, ひらがな, カタカナ, 中文测试, 報告書-0042 present | unverified | — |
| U1 Korean (declared supported) | **5 missing → shown as `<U+XXXX>`** | **fail** (`한국어` absent) | unverified | — |
| U1 Arabic/Hebrew (unsupported) | glyphs in DejaVuSans, **unshaped**, logical order; RTL marker shown | extracted in reversed visual order (`.ملاعلاب ابحرم`, `.םלוע םולש`), as expected for unshaped LTR placement; explicit limitation marker present | unverified (expected incorrect: no joining/bidi) | — |
| U1 emoji | not rendered; `<U+1F600> <U+1F680>` | matches the documented limitation | unverified | — |
| U1 combining (e+U+0301, a+U+0308, n+U+0303) | marks present in DejaVuSans, **positioned without GPOS** | extracts as `é ä ñ`; mapping preserved | **unverified**; positioning not claimed | — |
| U1 astral (𝐀𝐁, 𓀀) | missing → `<U+1D400><U+1D401> <U+13000>` | as documented | unverified | — |
| U2 controls | all 6 shown as code points | pass; plus a spurious RTL marker (H-1) | unverified | — |
| T1 tables | DejaVuSans only | all 7 sentinels incl. the unbroken 200-char path | unverified | 13 pages; header repeated 12 times; 5 long words broken at character boundaries; min baseline 3.3 pt above bottom limit; no right overflow |
| B1 page breaks | DejaVuSans + NotoSansMono | first/last code lines, last paragraph, end marker present | unverified | 6 pages; 15 code continuations with `↪`; no overflow |
| G1 large | DejaVuSans only | first/middle/last finding present | unverified | 401 pages; **max right overflow 1.185 pt (O-1)** |

Supported (Latin, Han/Kana) and intentionally unsupported (RTL, emoji, astral, combining positioning) fixtures are kept separate. The unsupported ones produced their documented visible limitation outcome and are **not** reported as correctly rendered.

### Size, time, resources (debug build; `optimize=true`, i.e. flate compression and pruning; `subset_fonts=true` has no effect without `text_layout`; `secure=true` means "skip unknown ops", **not encryption or a security boundary**)
| Case | Input B | Pages | Fonts embedded (file bytes) | Output B | Conversion wall ms | Peak RSS (VmHWM, render child) |
|---|---|---|---|---|---|---|
| U1-latin-cjk | 222 | 1 | DejaVuSans + DroidSansFallbackFull (4,793,140) | 2,525,895 | 2,258 | 39.9 MiB |
| U1-unsupported | 154 | 1 | DejaVuSans (759,720) | 387,359 | 296 | 17.2 MiB |
| T1 | 25,102 | 13 | DejaVuSans | 429,270 | 407 | 21.5 MiB |
| B1 | 21,763 | 6 | DejaVuSans + NotoSansMono (1,270,332) | 651,180 | 480 | 20.6 MiB |
| G1 | 737,481 | 401 | DejaVuSans | 833,390 | 1,931 | 41.6 MiB |
| others (U2, R1, L1, A1, C1) | 44–207 | 1 | DejaVuSans | 386,822–387,086 | 289–301 | 17.1–17.4 MiB |

- The font-file estimate differs from the measured PDF size: compression roughly halves the embedded fonts (DejaVuSans 0.76 MB → about 0.38 MB per PDF; with DroidSansFallbackFull, 4.79 MB → 2.53 MB).
- All outputs are far below the 64 MiB ceiling. That is evidence about this experiment, not proof that multi-MB report PDFs are acceptable for routine product use.
- Wall time is measured by the orchestrator around the render child; peak memory is self-reported VmHWM of the render child. `mutool` timings are in the per-step details.

### Suitability (current evidence)
**P1 is not established as suitable: the formal run failed.**
- All outputs fail the request's "no `/Annots`" criterion (D-A, a library behaviour).
- Korean is not covered by the allowlisted fonts (D-B).
- Visual correctness is unverified throughout.
- On the passing parts: Latin and Han/Kana extraction, tables with header repetition and unbroken long paths, page breaks, code wrapping, remote/local/active content rendered as inert text with no image, JavaScript, action or form objects, typed limits and failure paths, and no encryption are encouraging.
- Whether P1 becomes "suitable only with limitations" depends on owner decisions on D-A and D-B and on a visual-inspection method.

### Proposed bounded debug cycle P00-T08-D01 (not started; needs owner approval)
1. **D-A, owner decision on the criterion. I do not change it.** Either:
   - **(a)** amend the P1 rule to "`/Annots` absent **or** an empty array; any annotation object, `/Subtype /Link`, or action dictionary is a failure", verified by parsing each page dictionary's `/Annots` value in the `clean -d` output (the harness's existing tokenizer, extended to read the array value); or
   - **(b)** keep the criterion, in which case P1 is unsuitable without an output-level change, and no such change is proposed.
2. **D-B, owner decision.** Either:
   - **(a)** an installation request for a Hangul-capable font (for example a Debian CJK font package) plus an allowlist amendment; or
   - **(b)** declare Korean outside the supported set for this experiment (a scope decision, not to be taken by me); or
   - **(c)** record the allowlisted fonts as not meeting full CJK coverage.
3. **H-1 fix:** RTL range ending at U+FEFC; regression: U2 must have no RTL marker, and U1-unsupported must still have its 2.
4. **H-2 fix:** on verification failure, move the temp PDF and normalised file to a quarantine artefact directory outside source control, marked "not written", instead of deleting them. C1 is still deleted.
5. **O-1:** diagnose the 1.185 pt overflow by recording the offending line's measured widths. Fix only if a confirmed cause is found.
6. **Visual check (optional, owner decision):** approve `mutool draw -F png` (or `-F pnm`) on spike-owned PDFs, with Opus inspecting the rendered pages, so that visual verdicts can move from "unverified".
7. **Retest:** a full formal re-run of all eleven cases as `D/D01/`. run1 is preserved unchanged.
