# P00-T03 read-only research log (C-1, C-2)

Date: 2026-10-24. Authority: owner approval C-1 (read-only crates.io / RustSec / docs.rs / upstream metadata for the listed candidates) and C-2 (RustSec screening), 2026-10-24. Method: Claude Code WebFetch (helper-model summaries of the pages; values quoted as returned) and one `curl` of the GitHub contents API for the RustSec advisory-db `crates/` directory. No crate archive, source checkout, Cargo resolution or build. Values are untrusted until re-checked against the resolved lockfile.

## crates.io metadata (latest stable at lookup time)

| Crate | Version | Published | License (crates.io declared) | rust_version | Checksum | Repository | Features (names) |
|---|---|---|---|---|---|---|---|
| libc | 0.2.191 | 2026-10-16 | MIT OR Apache-2.0 | 1.65 | 65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a | github.com/rust-lang/libc | align, const-extern-fn, default, extra_traits, rustc-dep-of-std, std, use_std |
| rustix | 1.1.5 | 2026-09-16 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1.65 | 891efababe418670775f199f0d233d84843c227a0949a883ce15b37c78d6629d | github.com/bytecodealliance/rustix | 31 incl. process, std, use-libc |
| nix | 0.31.3 | 2026-05-11 | MIT | 1.69 | cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d | github.com/nix-rust/nix | 35 incl. process, signal |
| tokio | 1.54.1 | 2026-10-21 | MIT | 1.85 | b52fd3255258d26fac7264ab5a76b53c13367136dc011d3d6ad4f8ed070396a3 | github.com/tokio-rs/tokio | 17 incl. rt, process, time, io-util, sync, macros, signal, full |
| saphyr | 0.2.2 | 2026-10-15 | MIT OR Apache-2.0 | 1.85.0 | 0662b098655e5e779a80041b640702e341cf3ede4560ecc103cf0ef098c8e33e | github.com/saphyr-rs/saphyr | default, encoding |
| saphyr-parser | 0.2.2 | 2026-10-15 | MIT OR Apache-2.0 | 1.85.0 | 6c6220586c3dacac54bfddec87af86ddb31862d29666e7e35cf4baf70a25d387 | github.com/saphyr-rs/saphyr | debug_prints |
| yaml-rust2 | 0.13.0 | 2026-09-11 | MIT OR Apache-2.0 | 1.85.0 | 57e5b818a27a4cd30884ea380857a5e56f7ec3ba24a3990a3cc0b95af3238e18 | github.com/Ethiraric/yaml-rust2 | debug_prints, default, encoding |
| serde_yaml | 0.9.34+deprecated | 2024-03-25 | MIT OR Apache-2.0 | — | — | github.com/dtolnay/serde-yaml | version string marks it deprecated |
| serde_yml | 0.0.13 | 2026-05-27 | MIT OR Apache-2.0 | 1.85.0 | — | github.com/sebastienrousseau/serde_yml | description: "DEPRECATED — `serde_yml` is unmaintained… thin compatibility shim" |
| jsonschema | 0.60.1 | 2026-10-23 | MIT | 1.85.0 | 65de752665c4a06528af6759955eecf60ba73ae7442d35349963a09c8031cd71 | github.com/Stranger6667/jsonschema | default = resolve-http, resolve-file, tls-aws-lc-rs, idna; also tls-ring, resolve-async, arbitrary-precision, … |
| boon | 0.6.1 | 2025-01-07 | MIT OR Apache-2.0 | **not declared** | baa187da765010b70370368c49f08244b1ae5cae1d5d33072f76c8cb7112fe3e | github.com/santhosh-tekuri/boon | none |
| serde | 1.0.229 | 2026-07-18 | MIT OR Apache-2.0 | 1.56 | 4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba | github.com/serde-rs/serde | alloc, default, derive, rc, std, unstable |
| serde_json | 1.0.154 | 2026-10-11 | MIT OR Apache-2.0 | 1.71 | e7e9cc8b1b85264074fbcc02a88680c4096b1e47df8f739dceb03bf482f04bd6 | github.com/serde-rs/json | incl. preserve_order, unbounded_depth |
| keyring | 4.2.0 | 2026-08-29 | MIT OR Apache-2.0 | 1.88.0 | 2270074a3d26bcac93c1dc5d2845eb4c089e8d761ccf6e0ea266a16004640627 | github.com/open-source-cooperative/keyring-rs | default = v1 → zbus-secret-service-keyring-store (Linux), apple keychain, windows store; cli |
| pulldown-cmark | 0.13.5 | 2026-10-21 | MIT | 1.71.1 | 278e9268c93b9e498a2da46c34d90ba93a3255eb99009646398f34bc73ab36eb | github.com/raphlinus/pulldown-cmark | default = getopts, html; simd |
| comrak | 0.57.0 | 2026-10-21 | BSD-2-Clause | 1.89 | b5ef59c72b988dfc7c73ec58ec0dc50f95dd87d7f1f39f428902bc379cea8507 | github.com/kivikakk/comrak | default = cli, syntect-onig, bon |
| printpdf | 0.12.8 | 2026-09-05 | MIT | 1.88 | 95d50369c5e18e18d4716bd855172db3a70f9e82c966687ae051ea58138f285c | github.com/fschutt/printpdf | default = html; text_layout, text_layout_hyphenation, images, … |
| genpdf | 0.2.0 | 2021-06-17 | Apache-2.0 OR MIT | not declared | — | git.sr.ht/~ireas/genpdf-rs | default, images |
| typst | 0.15.1 | 2026-07-17 | Apache-2.0 | 1.92 | d1cb79435d128de5d5443a058a983f6dcc16738760fa55ed703762bc278b1fd3 | github.com/typst/typst | none |
| sha2 | 0.11.1 | 2026-10-12 | MIT OR Apache-2.0 | 1.88 | 47d7069beb7d6ac7b9acd1039986e73443f24234f41074da099d6f994ac9ad19 | github.com/RustCrypto/hashes | alloc, default, oid, zeroize |
| tempfile | 3.27.0 | 2026-03-11 | MIT OR Apache-2.0 | 1.63 | 32497e9a4c7b38532efcdebeef879707aa9f794296a4f0244f6f69e9bc8574bd | github.com/Stebalien/tempfile | default, nightly |

Licence verification level: crates.io declared SPDX expression only. Upstream licence files were not inspected except where noted (libc source listing shows `LICENSE-APACHE`, `LICENSE-MIT`).

## Dependency / build facts established

| Crate | Fact | Source |
|---|---|---|
| libc 0.2.191 | One dependency, `rustc-std-workspace-core` ^1.0.1, **optional** (rustc-dep-of-std); `lib_links` null; **`build.rs` present** at crate root | crates.io dependencies API; docs.rs source listing |
| tokio 1.54.1 | Only non-optional normal dependency: `pin-project-lite`. Feature-gated: `mio`, `libc` (unix), `signal-hook-registry` (unix), `socket2`, `bytes`, `parking_lot`, `tokio-macros` (**proc-macro**, via `macros`) | crates.io dependencies API |
| keyring 4.2.0 | Normal: `keyring-core` ^1.0.0; Linux store via feature `v1` = `zbus-secret-service-keyring-store` (`crypto-rust`); `dbus-secret-service-keyring-store` (`crypto-rust`, `vendored`) only via `cli`; no build dependencies declared at this level | crates.io dependencies + version API |
| serde 1.0.229 | `derive` feature → `serde_derive` proc-macro (from feature name; transitive details not reviewed) | crates.io features |
| jsonschema 0.60.1 | default features include HTTP resolution and `tls-aws-lc-rs` (aws-lc is a native C library; build details not reviewed) | crates.io features |
| comrak 0.57.0 | default includes `syntect-onig` (Oniguruma native library; not reviewed) | crates.io features |

Transitive dependency graphs were **not** reviewed for any crate.

## RustSec screening (direct candidates only)

Advisory-db `crates/` directory listing (GitHub contents API, HTTP 200, 966 entries < 1000 API cap). Candidates **without** an advisory directory: libc, rustix, saphyr, saphyr-parser, yaml-rust2, jsonschema, boon, serde, serde_json, keyring, pulldown-cmark, printpdf, genpdf, typst, tempfile.

| Crate | Advisory | Type | Affected / patched | Effect on candidate version |
|---|---|---|---|---|
| tokio | RUSTSEC-2026-0384 | vulnerability (`Interval` race) | affected 1.54.0 (yanked); patched ≥1.54.1 | 1.54.1 patched |
| tokio | RUSTSEC-2026-0386 | unsound (`AsyncFd::new` etc.) | patched ≥1.53.3 (and 1.51.6–<1.52) | 1.54.1 patched |
| tokio | RUSTSEC-2025-0023 | unsound | patched ≥1.44.2 (+ backports) | not affected |
| tokio | RUSTSEC-2023-0005 | unsound | patched ≥1.24.2 | not affected |
| tokio | RUSTSEC-2023-0001 | vulnerability (Windows) | patched ≥1.23.1 | not affected |
| tokio | RUSTSEC-2021-0124 | vulnerability | patched ≥1.13.1 | not affected |
| tokio | RUSTSEC-2021-0072 | vulnerability | patched ≥1.8.1 | not affected |
| nix | RUSTSEC-2021-0119 | vulnerability | patched ≥0.23.0 | 0.31.3 not affected |
| sha2 | RUSTSEC-2021-0100 | vulnerability (AVX2) | affected 0.9.7 only; patched ≥0.9.8 | 0.11.1 not affected |
| comrak | RUSTSEC-2021-0063, RUSTSEC-2021-0026 | XSS | ranges **not retrieved** | unknown; comrak not selected |
| serde_yml | RUSTSEC-2025-0068 | unsound + unmaintained | ranges not retrieved | excluded |
| serde_yaml | RUSTSEC-2018-0005 | uncontrolled recursion | ranges not retrieved | excluded (deprecated) |
| yaml-rust | RUSTSEC-2024-0320 (unmaintained), RUSTSEC-2018-0006 | — | not retrieved | not a candidate (predecessor of yaml-rust2) |

Scope: direct-candidate screening only. It does not review the eventual resolved dependency graph and says nothing about code or build safety.

## URLs consulted
`https://crates.io/api/v1/crates/<crate>` for each crate above; `…/libc/0.2.191/dependencies`; `…/tokio/1.54.1/dependencies`; `…/keyring/4.2.0` and `…/keyring/4.2.0/dependencies`; `https://docs.rs/crate/libc/0.2.191/source/`; `https://api.github.com/repos/rustsec/advisory-db/contents/crates`; `https://rustsec.org/packages/{tokio,nix,sha2,comrak,serde_yml,serde_yaml,yaml-rust}.html`; `https://rustsec.org/advisories/RUSTSEC-{2026-0384,2026-0386,2025-0023,2023-0005,2023-0001,2021-0124,2021-0072,2021-0119,2021-0100}.html`.
