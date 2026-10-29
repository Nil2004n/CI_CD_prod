# cargo-audit stage C-A — record

Date: 2026-10-27 (fetch time UTC in `fetch-time.txt`). Executed by Opus. Authority: owner approval of P02 A8 = 8a (2026-10-26, "approve stage C-A before T02") and of P02-T01 (2026-10-27). Procedure: `planning/P01-T03-cargo-audit-request.md` §3 stage C-A. Branch `phase/P02-store` at `f454f92`.

## Result: PASS (stop rules not triggered)

| Step | Command / action | Result |
|---|---|---|
| Staging | new empty `~/.local/share/pwt-dev-tools/cargo-audit/staging-C-A/{dl,extract}` (refused if it already existed) | created |
| Fetch (only network action) | `curl --proto '=https' --tlsv1.2 -sS -f -L --max-filesize 20000000 https://static.crates.io/crates/cargo-audit/cargo-audit-0.22.2.crate` | exit 0, HTTP/2 200, 63,768 bytes, `last-modified: Sun, 28 Jun 2026 15:49:23 GMT` (`headers.txt`, `curl-exit.txt`) |
| Checksum | `sha256sum` | `700c2b240f7fd330c24b675fe429f73a5b676531fcc6300400b2b67f155ba12a` = P00 lookup L12 value — **match** (`sha256.txt`) |
| Archive check | `tar -tzv` before extraction | 24 entries, all regular files, all under `cargo-audit-0.22.2/`, no `..` (`tar-listing.txt`) |
| Extract | `tar -xzf … --no-same-owner --no-same-permissions` into the empty `extract/` dir | exit 0 |
| Packaged lockfile | present | sha256 `1762e201cbd2cd6992bb9250f45dda4ebbdde2f79d6393f37ee89bda4e0d9fd6` (`packaged-lock.sha256`); read as data with `python3 -I -B` + `tomllib` |

Nothing was built, installed or executed from the package; no index access (direct static file URL); no cargo invocation.

## Packaged graph (data only; assessment is C-A2, not approved)
- Lock format v4; **399 packages** (398 crates.io + the local package); 20 crate names in more than one version. Full list `packaged-lock-packages.txt` (name, version, source, checksum).
- Present in the graph (all-platform lock; not filtered to Linux, not assessed): `aws-lc-rs`, `aws-lc-sys`, `ring`, `rustls`, `reqwest`, `hyper`, `tokio`, `gix`, `tame-index`, `zlib-rs`, `cc`, `linux-raw-sys`, and platform `-sys` crates (`windows-sys`, `core-foundation-sys`, `security-framework-sys`, `jni-sys`, `js-sys`, `web-sys`).

## Observations for the owner (no decision taken)
- **O-1:** `aws-lc-sys` and `ring` compile bundled C/assembly through build scripts. If they are in the Linux build graph, C-C (`cargo install`) would run native compilers on fetched source. That is a much larger build-code exposure than anything accepted in P00–P02, and it meets the C-A2 stop rule "native TLS library required" unless features exclude them. C-A2 should establish the Linux feature-resolved graph.
- **O-2:** The graph is ~5× the P02 application graph (88 packages, `plan.md` §5B.2), as the P01-T03 request anticipated.
- Staging stays in place for C-A2 (outside the repository). Removal: delete `~/.local/share/pwt-dev-tools/cargo-audit/staging-C-A/`.

## Not done (needs separate approval)
C-A2 (graph/exposure listing + RustSec lookups), C-B, C-C, C-D, C-E. P02 criterion 11 therefore currently reads "cargo-audit not run (stages C-A2–C-E not approved)".
