# P00-T08-D01 — debug cycle record

Date: 2026-10-24. Authority: owner approval: "accept empty /Annots and approve D01".
- D-A decided: option (a). `/Annots` is permitted only as an inline empty array; any element, an indirect reference, or a `/Annot` or `/Link` object fails.
- D-B (Hangul coverage) **not decided**, so the Korean expectation in U1-latin-cjk is unchanged.
- The optional page-image rendering item (`mutool draw -F png`) was **not** separately approved and was not used, so visual correctness stays unverified.

Base `91d419f`, branch `phase/P00-decisions-spikes`. The P00-T08 run1 evidence (`evidence/P00/P00-T08/D/run1/`) is preserved unchanged.

## Implementation (delegated)
- Implemented by the `sonnet-implementer` sub-agent (`.claude/agents/sonnet-implementer.md`, `model: claude-sonnet-5-5`; the model actually used is **unverified**, and the worker self-reported "configured: claude-sonnet-5-5 (unverified self-report)").
- The assignment packet gave it ownership of only `spikes/rust/t08-pdf/src/main.rs` and `spikes/rust/t08-pdf/run/outer.sh`, and permitted only the isolated build.
- Worker deviations, accepted by Opus:
  - it ran `bash -n outer.sh`, a syntax check that was not in the permitted list;
  - it also quarantines on a rename failure after verification (a `verification_failed:*` verdict).
- Opus inspected the full diff before verification. The worker's report is not evidence.

Changes:
1. **H-1:** `is_rtl` range now ends at `U+FEFC`, so the BOM is no longer RTL.
2. **D-A:** new `annots_values()`, using the same lexer rules as `pdf_names`, classifies each `/Annots` value as inline-empty or other. `Annots` was removed from the forbidden names, and `Annot` and `Link` were added. Step 5 requires `annots_other == 0` in both the normalised and original files.
3. **H-2:** new `quarantine()` moves not-written temp and normalised PDFs to `<root>/quarantine/<id>/…NOT-WRITTEN.pdf`, and `outer.sh` moves them to `<artifact-dir>/quarantine/` outside source control, hashes only. C1 artefacts are always deleted.
4. **O-1:** instrumentation only. `model_max_visible_right_overflow_pt` and up to 5 `overflow_samples` are added; the meaning of `model_max_right_overflow_pt` is unchanged.

Unchanged: fixtures, sentinels, page ranges, limits, the other verification steps, manifests, `Cargo.lock`, `setup-and-run.sh`.

Known limitation (pre-existing, noted by the worker): PDF names with `#xx` escapes are not decoded by the tokenizer. `mutool clean` writes canonical names, which reduces but does not remove this risk.

## C — build (`C/`)
- Opus re-ran `unshare -rn <isolated cargo> build --frozen --offline -p t08-pdf` → exit 0, 0 warnings (`C/C-build.log`). Cargo reported the target up to date with the worker's build of the same source.
- Frozen files (`Cargo.lock`, all `Cargo.toml`, `setup-and-run.sh`) are identical to `HEAD` and unchanged by the build (`C/hashes-before.txt`).
- Artifact `8b2bece6…a8a06`.

## D — formal re-run (`D/preflight/`, `D/run/`)
- Same isolation as run1. **Preflight 11/11 passed.** Fonts unchanged; spike root removed; no t08 roots left.
- Run: 11 s wall; `results.json` contains no canary value (checked).

| Case | Steps 1–5 | Verdict | Notes |
|---|---|---|---|
| U1-latin-cjk | 1–3 ok, **4 FAIL**, 5 ok | verification_failed, **quarantined** | `한국어` still missing: no Hangul glyphs in the allowlisted fonts (D-B, undecided); /Annots inline-empty 1/1 |
| U1-unsupported | all ok | pdf_written | 2 RTL markers, emoji/astral as code points; RTL unshaped (documented limitation, not "correct") |
| U2 | all ok | pdf_written | **0 RTL markers (H-1 fixed)**; 6 controls as code points |
| T1 | all ok | pdf_written | 13 pp; header repeated; /Annots empty 13/13 |
| B1 | all ok | pdf_written | 6 pp |
| R1 / L1 / A1 | all ok | pdf_written | inert text only; no image, URI, JS, action, form or annotation objects |
| G1 | all ok | pdf_written | 401 pp, 833,390 B, 1,911 ms; /Annots empty 401/401 |
| C1 | all ok | pdf_written, then **deleted** | canary present in extraction as required; canary artefacts removed |
| X1a–X1e, F1a, F1b | — | expected_typed_failure (all 7) | as run1; JSON/MD companions untouched |

- **D-A, now shown at runtime:** each page has exactly one `/Annots` with an inline empty array in both the original and the normalised file, and `annots_other = 0` everywhere. No `Annot`/`Link`/JavaScript/JS/OpenAction/AA/Launch/URI/SubmitForm/AcroForm/XFA/EmbeddedFile/RichMedia/Encrypt/Metadata/GoTo names; no image objects; no object streams; no parser repair.
- **O-1, cause confirmed:** G1's `model_max_right_overflow_pt = 1.185` is entirely trailing whitespace. Each sample has `trailing_ws_width = 3.179` (one space at 10 pt), `visible_end = 543.286` against a limit of 545.28. `model_max_visible_right_overflow_pt = 0.0` for every case, so no glyph passes the right margin. This is a measurement artefact; no layout change is needed.
- **H-2 shown:** the U1-latin-cjk temp and normalised PDFs are in `<artifact-dir>/quarantine/U1-latin-cjk/` (hashes in `D/run/quarantine-pdfs.sha256`), with `pdf_written=false`.
- **Artefacts:** 8 written PDFs plus the quarantine are under `~/.local/share/pwt-dev-tools/_artifacts/t08-d01-run/`, outside source control. Hashes are in `D/run/artifact-pdfs.sha256`. No PDF is in the repository.
- **Visual correctness: unverified for every case.** No page was rendered to an image or inspected.

## Result
- **D01 fixed H-1 and H-2, applied D-A, and diagnosed O-1.**
- The formal re-run still has **one failing case**: U1-latin-cjk, Korean only. That is a font-coverage gap pending owner decision D-B, not a harness defect.
- It is not an overall pass.
- Remaining owner decisions: D-B (Hangul) and whether to approve visual inspection; then T08 acceptance or qualification.

## Owner decision D-15 (2026-10-24) — appended; runs above unchanged
Korean (Hangul script) support is out of scope for the entire project (`plan.md` D-15, PRD §16). For T08 this resolves D-B:
- U1-latin-cjk's only failure (sentinel `한국어` absent; Hangul rendered as visible `<U+XXXX>` labels) now concerns out-of-scope content.
- The visible code-point labels match the required degradation behaviour (not dropped, no crash).
- The recorded verdicts of run1 and D01 are **not** rewritten; they stay `verification_failed` against the criteria as declared at the time.
- A formal re-run with the Korean line reclassified into the unsupported fixture would be needed for a clean formal result. That re-run is not approved and not done.
