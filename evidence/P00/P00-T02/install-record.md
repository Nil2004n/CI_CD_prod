# P00-T02 install record

Date: 2026-10-24. Authority: owner decisions R-1 (Request A), R-2 (B1), R-3 (listed versions), R-5 (upstream verification) in the 2026-10-24 approval message. Executed by Opus. Host: Kali Rolling x86_64; curl 8.21.0; downloads over HTTPS only (`--proto '=https' --proto-redir '=https' --tlsv1.2`).

Prefix: `~/.local/share/pwt-dev-tools/` (user-owned; no sudo; no PATH/profile changes). Staging: `~/.local/share/pwt-dev-tools/_staging/2026-10-24-*` (downloads, checksum files, attestation JSON; not in the repository).

## Request A — result

| Tool | Version | Archive SHA-256 vs reviewed request | vs release checksum file | Archive paths | Installed binary | Binary SHA-256 | Offline identity check |
|---|---|---|---|---|---|---|---|
| Gitleaks | 8.30.1 | match | match | safe | **Reused existing** `~/.local/bin/gitleaks` (byte-identical to the binary in the verified official archive; no second copy installed) | `88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509` | `version` → `8.30.1`, exit 0 |
| actionlint | 1.7.12 | match | match | safe | `~/.local/share/pwt-dev-tools/actionlint/1.7.12/actionlint` | `c872d6db8c6bf83a8eaa704fc93999f027d55dffbc63b8a6abdccb47df5f4cd4` | `-version` → `1.7.12`, built with go1.26.1, exit 0 |
| kubeconform | 0.8.0 | match | match | safe | `~/.local/share/pwt-dev-tools/kubeconform/0.8.0/kubeconform` | `457722b36e98d7bdbc91525e594951ef5d7ea3a29da2d13b9b28a2fb8a5097b7` | `-v` → `v0.8.0`, exit 0 |

Identity checks ran as `unshare -rn env -i PATH=/usr/bin:/bin HOME=/nonexistent LANG=C.UTF-8 <binary> <version-flag>` (new user + network namespace; only a down loopback interface). Archives were inspected for absolute/parent paths before extraction and extracted with `--no-same-owner`. A first identity run did not capture exit codes (zsh has no `PIPESTATUS`); a second run captured exit 0 for all three.

Gitleaks reuse caveat: `~/.local/bin/gitleaks` lives in a user-writable directory outside the tool prefix. Its origin was not established, but its content is proven identical to the verified official binary as of 2026-10-24. Every spike must re-check its SHA-256 before use.

## R-5 — signatures / attestations

| Artifact | Upstream mechanism found | Claimed signer (read from certificate, **not** cryptographically verified) | Status |
|---|---|---|---|
| Gitleaks 8.30.1 linux_x64 | None (no signature assets; GitHub attestation API 404) | — | Limitation: same-origin checksum only |
| kubeconform 0.8.0 linux-amd64 | None (no signature assets; attestation API 404) | — | Limitation: same-origin checksum only |
| actionlint 1.7.12 linux_amd64 | GitHub artifact attestation (SLSA provenance v1, Sigstore bundle v0.3), subject digest matches | `https://github.com/rhysd/actionlint/.github/workflows/release.yaml@refs/tags/v1.7.12` | **SLSA provenance verified (V1)** |
| uv 0.13.5 x86_64 gnu | 2 attestations (SLSA provenance v1; GitHub release attestation), subject digest matches | `https://github.com/astral-sh/uv/.github/workflows/release.yml@refs/heads/main`; `https://dotcom.releases.github.com` | **SLSA provenance verified (V1)**; release attestation not verified |
| CPython 3.12.15+20261021 | 2 attestations (SLSA provenance v1; GitHub release attestation), subject digest matches | `https://github.com/astral-sh/python-build-standalone/.github/workflows/release.yml@refs/heads/main`; `https://dotcom.releases.github.com` | **SLSA provenance verified (V1)**; release attestation not verified |
| yamllint 1.38.0 wheel | PyPI provenance endpoint HTTP 200 (PEP 740) | not inspected | Not verified; not downloaded |
| Checkov 3.3.30 wheel | PyPI provenance endpoint HTTP 200 (PEP 740) | not inspected | Not verified; not downloaded |

Sequencing disclosure: actionlint was extracted and its `-version` executed (offline namespace) **before** the attestation lookup was made. Its checksum verification passed; its attestation remains unverified.

Sequencing note: the attestation check for actionlint was completed after it had been extracted and its version printed; see the V1 result below.

### V1 — attestation verification (owner-approved 2026-10-24)
- Verifier provenance: `/usr/local/bin/cosign` v3.1.2 (SHA-256 `f7622ed3cf22e55e1ae6377c080979ff77a22da9981c11df222a2e444991e7cf`, 141,150,460 B) is **byte-identical to the official `sigstore/cosign` v3.1.2 release asset `cosign-linux-amd64`** (GitHub release digest, published 2026-07-17; read-only lookup). The file is user-writable, so it is re-hashed before any reuse.
- Method: `cosign verify-blob-attestation --bundle <SLSA v1 bundle> --new-bundle-format --certificate-identity <exact workflow URI> --certificate-oidc-issuer https://token.actions.githubusercontent.com --type https://slsa.dev/provenance/v1 <artifact>`, with `HOME=~/.local/share/pwt-dev-tools/_cosign-home` (Sigstore public-good trust root fetched over the network into that directory). `--new-bundle-format` prints a deprecation notice only.

| Artifact | Expected identity | cosign exit |
|---|---|---|
| actionlint 1.7.12 linux_amd64 archive | `https://github.com/rhysd/actionlint/.github/workflows/release.yaml@refs/tags/v1.7.12` | 0 (Verified OK) |
| uv 0.13.5 x86_64 gnu archive | `https://github.com/astral-sh/uv/.github/workflows/release.yml@refs/heads/main` | 0 (Verified OK) |
| CPython 3.12.15+20261021 install_only archive | `https://github.com/astral-sh/python-build-standalone/.github/workflows/release.yml@refs/heads/main` | 0 (Verified OK) |
| Negative: actionlint archive, identity `github.com/attacker/...` | must fail | 1 (failed as expected) |
| Negative: actionlint archive against the uv bundle/identity | must fail | 1 (failed as expected) |

A first verification run piped output through `tail`, so its printed exit codes were `tail`'s; the table shows the second, unpiped run.
- Not verified: the GitHub "release" attestations (`https://dotcom.releases.github.com` signer) use GitHub's own trust root, not Sigstore public-good; only the SLSA provenance attestations were verified. uv and CPython were built from `refs/heads/main` workflows (normal for those projects), not a tag ref.
- PyPI provenance (PEP 740) for the yamllint and Checkov wheels: **not verified** (V3 was not selected). Their 94 transitive wheels' provenance was not checked either. Integrity basis: TLS to `pypi.org`/`files.pythonhosted.org` plus SHA-256 hashes enforced from the lock files; top-level wheel hashes equal the reviewed request values. Proposed fallback for owner review: V3 (verification-only venv with a PEP 740 verifier) or accept as recorded.

## R-2 — B1 result (resumed after V1)

| Component | Version | Path | Checks |
|---|---|---|---|
| CPython (python-build-standalone) | 3.12.15+20261021 | `~/.local/share/pwt-dev-tools/python/3.12.15+20261021/python/bin/python3.12` (SHA-256 `58fe899223901d346e9d3a0184bdd04d82a276ff1f99be0e68d5873ea71c1b68`) | archive digest match + SLSA attestation verified; offline `sys.version` 3.12.15, OpenSSL 3.5.9, exit 0 |
| uv | 0.13.5 | `~/.local/share/pwt-dev-tools/uv/0.13.5/uv-x86_64-unknown-linux-gnu/uv` (SHA-256 `8a70db72685b7138121bfcda71fa1a00cb709ed9f1fdc4ecf1dd871e8e167e52`) | archive digest + `.sha256` match + SLSA attestation verified; offline `--version` exit 0 |
| yamllint | 1.38.0 | venv `~/.local/share/pwt-dev-tools/yamllint/1.38.0/venv` (3 packages) | lock `locks/yamllint.lock.txt`; offline `yamllint --version` → `yamllint 1.38.0`, exit 0; `uv pip check` compatible |
| Checkov | 3.3.30 | venv `~/.local/share/pwt-dev-tools/checkov/3.3.30/venv` (96 packages) | lock `locks/checkov.lock.txt`; offline `checkov --version` → `3.3.30`, exit 0 (120 s limit, not hit); `uv pip check` compatible |

Procedure: `uv pip compile` per tool (`--python` = the standalone interpreter, `--python-platform x86_64-unknown-linux-gnu`, `--only-binary :all:`, `--generate-hashes`, index `https://pypi.org/simple`); `uv venv --python <standalone>`; `uv pip install --require-hashes --only-binary :all: --no-deps -r <lock>`. All uv runs used `env -i`, `UV_NO_CONFIG=1`, `UV_PYTHON_DOWNLOADS=never`, cache `~/.local/share/pwt-dev-tools/_uv-cache`, home `~/.local/share/pwt-dev-tools/_uv-home`. Locks are fully `==`-pinned with hashes; no source distributions were built.

Errors encountered: the first install attempt passed both `--only-binary` and `--no-build`, which uv rejects as a usage error (exit 2); nothing was installed. Re-run without `--no-build` (redundant with `--only-binary :all:`) succeeded. The compile step's printed exit codes were `tail`'s (zsh pipe); success was confirmed from the complete lock files and reviewed hashes.

System Python untouched: `/usr/bin/python3 -> python3.14`; no alternatives, `sudo`, system `pip` or `--break-system-packages` used.

Disk use (approx.): python 228M, checkov 227M, uv 37M, `_uv-cache` 39M, `_staging` 120M, others <20M.

## Not done
- cargo-audit: deferred to P01 (R-4); pending dependency, not a check.
- No tool was run against fixtures; no scanner spike.
- Staging (`_staging/`) holds the verified archives, checksum files, attestation JSON/bundles and `gitleaks-compare/`; `_uv-cache/`, `_uv-home/`, `_cosign-home/` hold machine-local caches. None is in the repository; they can be deleted without affecting installed tools.

## Rollback
`rm -rf ~/.local/share/pwt-dev-tools` removes everything created here. `~/.local/bin/gitleaks` predates this work and is not touched by rollback.
