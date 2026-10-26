# P01-T03 — cargo-audit installation request and offline advisory-DB plan

Status: **request only (A4 = 4a: the install stays deferred).** Prepared by Opus on 2026-10-26.
- Nothing was looked up, fetched, installed or run for this document.
- Facts come from the P00 records; any value not taken from them is marked *unverified*.
- P01 acceptance criterion 8 records cargo-audit as **"not run — install deferred by owner (A4 = 4a)"**, unless the owner approves this request separately.

## 1. Facts carried from P00 (not re-checked)
| Item | Value | Source |
|---|---|---|
| Version | `cargo-audit` 0.22.2, crates.io, published 2026-06-05 | P00-T02 Request C, lookup L12 |
| Licence | Apache-2.0 OR MIT | L12 |
| Crate checksum | `700c2b240f7fd330c24b675fe429f73a5b676531fcc6300400b2b67f155ba12a` | L12 |
| Rust requirement | ≥ 1.88 (host 1.97.1) | L12 |
| State | requested, not installed, not verified; no advisory scan exists | `spikes/p00-freeze/tools.md` |

## 2. Why it matters, and why later is fine
- **What it adds:** cargo-audit checks a `Cargo.lock` against the RustSec advisory database. It is the only automated dependency-advisory check in Tech §22.
- **Why P01 can wait:** P01's graph is 11 crates, all screened directly against RustSec in P00 (2026-10-24) and all byte-equal to accepted P00 entries (`evidence/P01/T02/record.md`).
- **The cost of installing:** cargo-audit is built from source. Its own dependency graph (git/HTTP/TLS-capable crates for fetching the database) is *unverified*, but almost certainly much larger than P01's whole graph. Installing it adds more build-time code exposure than P01 itself.
- **When the value rises:** as soon as dependencies grow (P02 jsonschema, P03/P16 HTTP, P06 tokio). **Recommendation: approve this request before P02 starts.**

## 3. Proposed procedure (each stage needs owner approval; stop rules as in P00-T06–T08)
| Stage | Action | Network | Stop if |
|---|---|---|---|
| C-A | Fetch the single `cargo-audit-0.22.2.crate` into an isolated staging `CARGO_HOME`, verify the checksum against §1, and read its packaged `Cargo.lock` without building anything | crates.io (one file plus index) | checksum mismatch; no packaged lockfile |
| C-A2 | From that lockfile, list every crate, build script, proc-macro and `links`/`-sys` crate (native linking: openssl, libgit2, zlib); RustSec-screen the graph by read-only lookup | read-only lookup | any `links` crate not justified; unpatched advisory; native TLS/git library required |
| C-B | `cargo fetch --locked` of that graph only, in the staging `CARGO_HOME`; checksums verified | crates.io | any checksum mismatch |
| C-C | `cargo install --locked --offline --version 0.22.2 --root <pwt>/cargo-audit/0.22.2` inside `unshare -rn` (build scripts run; no filesystem sandbox, same as P00) | none | build failure (report; no repair without approval) |
| C-D | Advisory DB (§4): one pinned snapshot | GitHub (one fetch) | integrity cannot be pinned |
| C-E | First run against `workbench/Cargo.lock` inside `unshare -rn`, offline DB only; record the result as evidence | none | any network attempt; flags not as expected |

- **Location:** `~/.local/share/pwt-dev-tools/cargo-audit/0.22.2/`, user-owned; no sudo, no PATH change, invoked by absolute path. Removal = delete that directory and the DB directory.
- **Exact flags:** *unverified* candidates, to be confirmed against the installed binary's `--help` in C-E: `cargo audit --no-fetch --db <dir> --file <lock>`, plus `--stale` if the binary refuses an old database.

## 4. Offline advisory-DB plan
- **What the database is:** the RustSec advisory database (`github.com/rustsec/advisory-db`, *unverified* URL from general knowledge, to be confirmed in C-D).
  - cargo-audit normally fetches it at run time. The application product never does: development only.
  - Its licence is *unverified*, to be checked in C-D.
- **Snapshot:** a single owner-approved fetch at a recorded commit OID, into `~/.local/share/pwt-dev-tools/cargo-audit/advisory-db/<commit>/`.
  - Record the fetch time and commit OID, plus a tree hash computed with Git after the fetch.
  - Whether cargo-audit 0.22.2 accepts a plain directory or needs a Git checkout is *unverified*; decide in C-D.
- **Use:** every audit run uses `--no-fetch` with that directory, inside `unshare -rn`, so the check is offline and reproducible.
  - Evidence records the DB commit, its date and the age of the snapshot at the time of the run.
- **Refresh:** a refresh is a new approved fetch. A stale snapshot is shown as stale and is never treated as "no advisories".
  - The staleness threshold is an owner policy decision. The proposal is to refresh at each phase start where dependencies change.
- **Interpreting results:**
  - an `unmaintained`/informational notice is reported, not hidden (as K-A in P00-T07);
  - a vulnerability advisory on a P01+ dependency stops the phase for an owner decision;
  - "no advisories found" is only as current as the snapshot.

## 5. Approvals that would be needed (none requested now)
C-A + C-A2 (crate fetch plus read-only lookups) → C-B (locked fetch) → C-C (build/install, build-code exposure) → C-D (DB fetch) → C-E (first run). Each stage reports and stops.
