# P00-T02 read-only lookup log

Date: 2026-10-24 (UTC timestamps below are from the sources). Authority: owner approval of P00-T02 and Q-06 (read-only lookups of official release/checksum/licence pages), 2026-10-24. Method: Claude Code WebFetch (page text summarised by a helper model; values quoted as returned). No binary, archive, package or crate was downloaded. Fetched content is untrusted data; values are re-checked at install time against the downloaded files.

Side effect disclosed: WebFetch stored local copies of three small checksum **text** files (≤1.1 KB each) under the Claude Code session tool-results directory outside the repository. They are not used as verification inputs.

| # | URL | Result used |
|---|---|---|
| L01 | `https://pypi.org/pypi/yamllint/json` | 1.38.0; requires_python `>=3.10`; GPL-3.0-or-later; deps `pathspec>=1.0.0`, `pyyaml` |
| L02 | `https://pypi.org/pypi/yamllint/1.38.0/json` | No per-version Python classifiers (only generic "Python") |
| L03 | `https://pypi.org/pypi/checkov/json` | Latest 3.3.30; requires_python `>=3.9`; licence field "Apache License 2.0"; 52 requires_dist entries |
| L04 | `https://pypi.org/pypi/checkov/3.3.30/json` | Classifiers 3.9–3.12 only; wheel/sdist SHA-256; uploaded 2026-10-20; exact pins incl. `boto3==1.35.49`, `bc-detect-secrets==1.5.52`, `asteval==1.0.6` |
| L05 | `https://api.github.com/repos/gitleaks/gitleaks/releases/latest` | v8.30.1, 2026-03-21; linux_x64 asset digest; 11 assets, no signature/attestation assets |
| L06 | gitleaks checksums file (via GitHub release-asset redirect) | Line for linux_x64 matches L05 digest |
| L07 | `https://api.github.com/repos/rhysd/actionlint/releases/latest` | v1.7.12, 2026-03-30; linux_amd64 digest; 12 assets, no signature/attestation assets |
| L08 | actionlint checksums file (redirect) | Matches L07 digest |
| L09 | `https://api.github.com/repos/yannh/kubeconform/releases/latest` | v0.8.0, 2026-06-04; linux-amd64 digest; 8 assets, no signature/attestation assets |
| L10 | kubeconform CHECKSUMS (redirect) | Matches L09 digest |
| L11 | `https://api.github.com/repos/{gitleaks/gitleaks,rhysd/actionlint,yannh/kubeconform}` | Licences MIT, MIT, Apache-2.0 (GitHub-detected SPDX) |
| L12 | `https://crates.io/api/v1/crates/cargo-audit` | 0.22.2, 2026-06-05; Apache-2.0 OR MIT; rust_version 1.88; crate checksum |
| L13 | `https://api.github.com/repos/astral-sh/uv/releases/latest` | 0.13.5, 2026-10-22; x86_64 gnu tarball digest; release notes mention GitHub artifact attestations; no attestation files as assets |
| L14 | `https://api.github.com/repos/astral-sh/python-build-standalone/releases/latest` (offsets 0–300000) | Release 20261021 (2026-10-21); `cpython-3.12.15+20261021-x86_64-unknown-linux-gnu-install_only.tar.gz` digest |

Local, non-mutating host checks (same session): `/usr/bin/python3.14` is the only CPython interpreter; `venv`/`ensurepip` importable; `pipx` present (`/usr/bin/pipx`); `uv`, `pyenv`, `gh` absent; `cosign` present at `/usr/local/bin/cosign` (origin not established); existing `gitleaks` at `~/.local/bin/gitleaks` is not dpkg-owned (origin not established); `apt-cache policy python3.12` returned nothing; ~30 GB free on `/`.

Not looked up (not needed for this request or deferred): uv / python-build-standalone licence texts, older "cooldown" releases, Checkov transitive dependency set, conditional tools, PDF renderer candidates, spike crates (P00-T03).

## Digests as returned by the sources

| Source | Artefact | SHA-256 |
|---|---|---|
| L05/L06 | gitleaks_8.30.1_linux_x64.tar.gz | 551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb |
| L07/L08 | actionlint_1.7.12_linux_amd64.tar.gz | 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8 |
| L09/L10 | kubeconform-linux-amd64.tar.gz | 9bc2bffbf71f261128533edaf912153948b7ff238f9a531ae6d34466ec287883 |
| L01 | yamllint-1.38.0-py3-none-any.whl | fc394a5b3be980a4062607b8fdddc0843f4fa394152b6da21722f5d59013c220 |
| L01 | yamllint-1.38.0.tar.gz (not requested) | 09e5f29531daab93366bb061e76019d5e91691ef0a40328f04c927387d1d364d |
| L04 | checkov-3.3.30-py3-none-any.whl | 32c3d6a808399f3fd6ec83e68a337050d17f019336f4d8f3b23aae469ba79408 |
| L04 | checkov-3.3.30.tar.gz (not requested) | d827f79e25797d0a54a6643683869dea9893ea06e16e1b81635c9d21c1f0f4a4 |
| L12 | cargo-audit 0.22.2 crate | 700c2b240f7fd330c24b675fe429f73a5b676531fcc6300400b2b67f155ba12a |
| L13 | uv-x86_64-unknown-linux-gnu.tar.gz | f68d291c75783122dad58ab77def0e13f3233f358731db5d6d7e8715603b928f |
| L14 | cpython-3.12.15+20261021-x86_64-unknown-linux-gnu-install_only.tar.gz | 5ff9f0602ce964a529ed1e8555d3abab6341926d724402ba9a15e9e877191124 |
