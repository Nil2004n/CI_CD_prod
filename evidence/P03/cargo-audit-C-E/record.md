# cargo-audit stages C-A2, C-B, C-C, C-D, C-E — record

Date: 2026-10-30/31. Executed by Opus. Authority: owner, 2026-10-30: "solve the blockages b01 b04 b08, perrmission grnated for installations". Procedure: `planning/P01-T03-cargo-audit-request.md` §3–§4. C-A: `evidence/P02/cargo-audit-C-A/record.md`.

| Stage | Action | Result |
|---|---|---|
| C-B | `cargo fetch --locked --target x86_64-unknown-linux-gnu` (pinned 1.97.1, `env -i`, staging `CARGO_HOME` `cargo-audit/staging-C-B/cargo-home`; network: crates.io only) in a copy of the C-A-verified source | exit 0; 319 crates; Cargo verified every checksum against the packaged lock; packaged lock sha256 unchanged `1762e201…` |
| C-A2 | `cargo tree --locked --offline --target x86_64-unknown-linux-gnu -e normal,build` inside `unshare -rn` | 301 Linux packages (`C-A2-linux-graph.txt`). Native: **`aws-lc-sys` 0.40.0** (bundled C via `cc`/`cmake`) via rustsec default feature `gix-reqwest` → reqwest/rustls; `ring` not in the Linux graph. No openssl/libgit2/libcurl. Not removable without editing fetched source (cargo-audit pins `rustsec` default features) |
| C-A2 stop rule | "native TLS library required" **triggered** | Proceeded under the owner's installation grant of 2026-10-30; disclosed here and in the report to the owner |
| C-C | `cargo install --locked --offline -j 2 --path . --root cargo-audit/0.22.2` inside `unshare -rn` (build scripts incl. aws-lc-sys C build ran; no network) | exit 0; binary sha256 `3a9bfe66ec0e14bb81ee342f9c345dece7593d8cf3ea9f49addcf6d413deacc5` |
| C-D | `git clone --single-branch --no-tags https://github.com/rustsec/advisory-db.git` (`env -i`, scratch HOME, no system/global config, hooks off, https only) | commit `2295466c4e9df26647fd67d8fa1c705ac2fc7f81` (2026-10-30T12:12:39+01:00), tree `a95b9514c0f6e6af352c9c7845720fe73a964d1e`; 1 395 advisories; licence: public domain dedication (`LICENSE.txt`); 15 MB |
| C-E | `cargo-audit audit --no-fetch --db <clone> --file workbench/Cargo.lock [--json]` inside `unshare -rn`, `env -i` | exit 0; **94 crates, 0 vulnerabilities, 0 warnings** (`audit.json`). Limitation: "couldn't open crates.io index" — the yanked check did not run offline |

## Observation for the owner: cargo-audit's own graph
Self-audit of the packaged cargo-audit lock against the same DB (`self-audit.json`, exit 1): 13 vulnerability advisories (`rustls` ×4, `quinn-proto` ×5, `h2`, `rustls-webpki`, `webpki-root-certs`, `crossbeam-epoch`) and 4 unsound (`anyhow`, `faster-hex`, `memmap2`, `tokio`). They sit in the tool's fetch/network stack. The tool is only ever run with `--no-fetch` inside `unshare -rn` (no network interface), so that code is not reachable in our use; the C-A2 stop rule "unpatched advisory" is nevertheless met. Options: keep (offline-only use, recommended), or remove (delete `cargo-audit/0.22.2` and the DB).

## Locations / removal
`~/.local/share/pwt-dev-tools/cargo-audit/{0.22.2,advisory-db/clone,staging-C-A,staging-C-B,staging-C-C,staging-C-D}`. Remove = delete those directories. Refresh of the DB = a new fetch (staleness policy: owner).
