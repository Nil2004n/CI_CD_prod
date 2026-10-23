# CA-02 contract check

| Field | Value |
|---|---|
| Date | 2026-10-24 |
| Commands | `python3 -I evidence/P00/CA-02/apply_ca02.py "$PWD"` (exit 0), then `python3 -I evidence/P00/CA-02/check_ca02.py "$PWD" > evidence/P00/CA-02/check-output.txt 2>&1; rc=$?` |
| Environment | Python 3.14.7; jsonschema 4.26.0 (pre-installed system package); `format` checking enabled; no network |
| Revision | `2649849` on `phase/P00-decisions-spikes` + uncommitted CA-02 / P00-T04 work |
| Exit | 0 (captured directly) |
| Counts | pass 21, fail 0, skip 0 |

Checks: meta-validity of the three schema files; wrapper `$defs` and Data Schema §7 embedded copy identical to the canonical schema; the three store examples (§8–§10) and the new §23 WrapperControl example validate; negative/positive variants (missing new fields rejected, signal 0 rejected, the spike-only value `leftover_descendants` rejected, spawn-failure record with `not_applicable` streams accepted, cleanup-only `descendant_cleanup` reason accepted, unknown stream state rejected); `Check.term_signal` required; CA-01 `allow_fallbacks` const retained; version identifiers unchanged.

Limitations: document/schema checks only, not application tests. Semantic rules in Data Schema §23 (primary reason not overwritten; at most one of exit code/signal non-null) are runtime rules, not schema-enforced. Version handling pending owner confirmation (Q-03).
