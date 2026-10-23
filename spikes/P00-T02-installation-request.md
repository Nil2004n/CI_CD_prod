# P00-T02 — Installation request (D-10)

Task: P00-T02 (owner-approved 2026-10-24, with Q-06 read-only lookups). Base revision `db402ca`. Sources: `evidence/P00/P00-T02/lookups.md` (L01–L14).

Status: **request only. Nothing has been downloaded or installed.** Each request below (A, B, C) needs its own explicit owner approval. Approving a request authorises only the listed downloads, the checksum verification, extraction/installation into the stated location, and a `--version` (or equivalent) identity check. It does not authorise running any tool against fixtures (that is P00-T09) or any other change.

These are **development-host spike installs** for the Linux x86_64 pilot workstation. They are not product bundling (D-05) and do not change the product rule that end users install scanners themselves.

## 1. Rules for every install

1. **Location:** user-owned prefix `~/.local/share/pwt-dev-tools/<tool>/<version>/`. No `sudo`, no system package manager, no writes outside the prefix (plus the Cargo case in §5).
2. **No PATH or profile changes.** Shell profiles, `~/.local/bin` and global config are not modified. Spikes call tools by absolute path and record it.
3. **No installer scripts.** Release archives only (no `curl | sh`, no `install.sh`). Archives are extracted into a new empty directory.
4. **Verify before extract/use:** compute SHA-256 of each downloaded file and compare with the value in this document; also compare with the release's checksum file downloaded alongside. Any mismatch → stop, delete the download, report.
5. **Identity check:** run only the version command; record output, binary path and binary SHA-256 in `evidence/P00/P00-T02/install-record.md`.
6. **Existing host tools are left untouched.** `~/.local/bin/gitleaks` (8.30.1 by version string, origin not established) is not used for spikes, not removed and not overwritten.
7. **Network:** only the hosts listed per request. No telemetry opt-ins.
8. **Rollback:** delete the tool's prefix directory (and §5 Cargo root). No other state is created.

## 2. Integrity limitations (read before approving)

- gitleaks, actionlint and kubeconform publish **no detached signatures or attestation files** as release assets (L05, L07, L09). Integrity rests on HTTPS plus a SHA-256 that comes from the **same release origin** as the binary. This detects corruption/tampering in transit, not a compromised release.
- uv's release notes mention GitHub artifact attestations (L13). Verifying them needs `gh` (absent) or a sigstore client; `/usr/local/bin/cosign` exists but its origin is not established. Attestation verification is **not** part of this request; it can be added as an option.
- **Recency:** Checkov 3.3.30 (uploaded 2026-10-20), uv 0.13.5 (2026-10-22) and python-build-standalone 20261021 (2026-10-21) are only days old. The other tools are months old. A cooldown policy (e.g. only releases ≥14 days old) would replace those three with earlier versions; that needs another read-only lookup. See decision R-3.

## 3. Request A — Go release binaries (baseline + framework adapters)

Network: `github.com`, `release-assets.githubusercontent.com` (GitHub release-asset redirect target).

| Tool | Version (published) | Asset | SHA-256 (GitHub digest = checksum file) | Licence | Prefix |
|---|---|---|---|---|---|
| Gitleaks | v8.30.1 (2026-03-21) | `gitleaks_8.30.1_linux_x64.tar.gz` | `551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb` | MIT | `…/gitleaks/8.30.1/` |
| actionlint | v1.7.12 (2026-03-30) | `actionlint_1.7.12_linux_amd64.tar.gz` | `8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8` | MIT | `…/actionlint/1.7.12/` |
| kubeconform | v0.8.0 (2026-06-04) | `kubeconform-linux-amd64.tar.gz` | `9bc2bffbf71f261128533edaf912153948b7ff238f9a531ae6d34466ec287883` | Apache-2.0 | `…/kubeconform/0.8.0/` |

Checksum files downloaded alongside for comparison: `gitleaks_8.30.1_checksums.txt`, `actionlint_1.7.12_checksums.txt`, kubeconform `CHECKSUMS`. Release URLs: `https://github.com/<owner>/<repo>/releases/download/<tag>/<asset>`.

Privileges: none beyond the user account. Compatibility: static-style Go binaries for linux x86_64; confirmed by the identity check only. Approx. download: ~18 MB total (asset sizes L05/L07/L09).

## 4. Request B — Python tools (yamllint, Checkov)

### 4.1 Interpreter compatibility (D-05, B-09)
- Host has only CPython 3.14 (`/usr/bin/python3.14`); apt shows no `python3.12` package.
- Checkov 3.3.30 declares classifiers for Python 3.9–3.12 only and pins older dependencies exactly (e.g. `boto3==1.35.49`, `bc-detect-secrets==1.5.52`, `asteval==1.0.6`). Running it on 3.14 is outside its declared support; compatibility is **unknown**.
- yamllint 1.38.0 requires `>=3.10` with no upper bound and no per-version classifiers; 3.14 is plausible but **unverified**.

### 4.2 Options

| Option | Interpreter | Adds | Notes |
|---|---|---|---|
| **B1 (recommended)** | CPython 3.12.15 from python-build-standalone release `20261021`, asset `cpython-3.12.15+20261021-x86_64-unknown-linux-gnu-install_only.tar.gz`, SHA-256 `5ff9f0602ce964a529ed1e8555d3abab6341926d724402ba9a15e9e877191124` (L14) | uv 0.13.5 as resolver/installer: `uv-x86_64-unknown-linux-gnu.tar.gz`, SHA-256 `f68d291c75783122dad58ab77def0e13f3233f358731db5d6d7e8715603b928f` (L13), plus its `.sha256` file | Within Checkov's declared range. Interpreter extracted by us from the verified archive (uv's automatic Python download is not used). Licences of uv and the standalone build not yet looked up |
| B2 | Host Python 3.14 with stdlib `venv` | nothing new | Smallest footprint; Checkov outside declared support; a failure or subtle misbehaviour is likely to be hard to distinguish from a spike finding. Acceptable as a compatibility probe, not as the pilot baseline |
| B3 | Distribution Python 3.12 | requires `sudo`/apt and no package is visible | Not recommended; conflicts with the no-sudo rule for this request |

### 4.3 Procedure (B1)
1. Download and verify uv and the CPython archive (§1 rule 4); extract to `…/uv/0.13.5/` and `…/python/3.12.15+20261021/`.
2. One virtual environment per tool, created with that interpreter: `…/yamllint/1.38.0/venv`, `…/checkov/3.3.30/venv`.
3. Resolve each tool's full transitive dependency set into a lock file **with hashes**, wheels only (no source builds, so no package build scripts run). Lock files are saved to `evidence/P00/P00-T02/` for review. If any dependency has no wheel for the interpreter/platform, stop and report instead of allowing a source build.
4. Install from the lock with hash enforcement into each venv.
5. Identity check: `yamllint --version`, `checkov --version`; record interpreter path/version and venv paths.

Tool artefacts (top level):

| Tool | Version | Wheel | SHA-256 | Licence |
|---|---|---|---|---|
| yamllint | 1.38.0 | `yamllint-1.38.0-py3-none-any.whl` | `fc394a5b3be980a4062607b8fdddc0843f4fa394152b6da21722f5d59013c220` | GPL-3.0-or-later |
| Checkov | 3.3.30 | `checkov-3.3.30-py3-none-any.whl` | `32c3d6a808399f3fd6ec83e68a337050d17f019336f4d8f3b23aae469ba79408` | Apache License 2.0 (PyPI field) |

Network: `github.com`, `release-assets.githubusercontent.com` (uv, CPython), `pypi.org`, `files.pythonhosted.org` (wheels). Checkov's transitive set is large (52 direct requirements) and will be listed in the lock; its size is unknown until resolution (estimate hundreds of MB installed, unverified).

Checkov runtime telemetry/network behaviour is **not** exercised by installation; it is examined in P00-T09 with networking unavailable.

## 5. Request C — development tool: cargo-audit (optional now, needed by P01 checks)

- `cargo-audit` 0.22.2 (crates.io, published 2026-06-05), licence Apache-2.0 OR MIT, crate checksum `700c2b240f7fd330c24b675fe429f73a5b676531fcc6300400b2b67f155ba12a`, requires Rust ≥ 1.88 (host 1.97.1).
- Procedure: `cargo install cargo-audit --version 0.22.2 --locked --root ~/.local/share/pwt-dev-tools/cargo-audit/0.22.2`. Cargo verifies crate checksums against the registry and the crate's published lockfile.
- **Caveat:** building from source runs the build scripts of cargo-audit's dependencies on this host. This is normal for Cargo but is code execution from downloaded crates.
- Network: `index.crates.io`, `static.crates.io`, `crates.io`. Cargo's registry cache under `~/.cargo/registry` is also written; rollback deletes the install root (cache removal optional).
- When run later, `cargo audit` downloads the RustSec advisory database (network); that is a separate per-use approval.

## 6. Not requested here

- ShellCheck, Bats-core: not needed (D-03; no shell scripts exist).
- zizmor, Docker Compose, Helm, Trivy: conditional (X1–X4), separate requests.
- PDF renderer candidates (P00-T08) and Rust crates for spikes/P01 (P00-T03): separate requests.
- Kubernetes schema downloads (D-11): separate request.

## 7. Decisions requested

| ID | Decision | Recommendation |
|---|---|---|
| R-1 | Approve Request A (Gitleaks, actionlint, kubeconform) | Approve |
| R-2 | Python approach: B1, B2, or B2-then-B1 | B1 |
| R-3 | Recency: accept the listed latest versions, or require ≥14-day-old releases (needs one more read-only lookup for Checkov, uv, CPython build) | Your call; ≥14 days is the more conservative supply-chain choice |
| R-4 | Approve Request C (cargo-audit) now or defer to P01 | Defer to P01 unless P00 needs it |
| R-5 | Add attestation/signature verification where available (needs a verified sigstore client) | Defer; record the limitation |
