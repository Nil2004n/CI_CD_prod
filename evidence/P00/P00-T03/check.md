# P00-T03 document check

| Field | Value |
|---|---|
| Date | 2026-10-24 |
| Command | `python3 -I evidence/P00/P00-T03/check_t03_doc.py "$PWD" > evidence/P00/P00-T03/check-output.txt 2>&1; rc=$?` |
| cwd | repository root |
| Environment | Python 3.14.7 standard library; no network |
| Revision | `008e80c` (= `origin/main`) + uncommitted P00-T03 and documentation changes |
| Exit | 0 (captured directly, per `evidence/README.md`) |
| Counts | pass 6, fail 0, skip 0 |

Checks: 15 Tier S rows; every proposed-version cell is "unresolved" or "not proposed"; no version numbers in those cells; every stated licence marked "to confirm"; toolchain statement matches the local `rustc --version`; document states no lookup was performed.

Limitations: guards against guessed versions only; does not validate crate suitability. Not an application test.

## Revision 2 check (after C-1 research)

| Field | Value |
|---|---|
| Command | `python3 -I evidence/P00/P00-T03/check_t03_rev2.py "$PWD" > evidence/P00/P00-T03/check-rev2-output.txt 2>&1; rc=$?` |
| Revision | `f1bd80d` on `phase/P00-decisions-spikes` + uncommitted rev 2 / research files |
| Exit | 0 (captured directly) |
| Counts | pass 6, fail 0, skip 0 |

Checks: 19 crate/version pairs in the request match the research table; libc checksum consistent; no claim that transitive graphs were reviewed; states nothing downloaded/resolved/compiled; does not claim `--offline` is a sandbox; all declared MSRVs ≤ 1.97. The revision 1 check (`check_t03_doc.py`, expects unresolved versions) is superseded for rev 2 and kept for history.
