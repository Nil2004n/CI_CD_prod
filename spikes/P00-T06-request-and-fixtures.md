# P00-T06 — YAML parser + JSON Schema validator: request and test design

Status: **documentation only** (owner-approved 2026-10-24). No crate resolution, fetch, build, spike execution or new lookup has been performed for T06. Base: `phase/P00-decisions-spikes` at `bce65be`. Inputs: `spikes/P00-T03-crate-request.md` rev 2 (§2 selections), `evidence/P00/P00-T03/research.md`, `spikes/P00-T01-fixture-specifications.md` §2.1, current contract (store 1.3 / protocol 1.1, specs v0.3.3).

Legend: **[known]** = established from existing evidence or our own files; **[after A]** = only knowable after authorised resolution; **[after C/D]** = only knowable after authorised build / execution.

## 1. Pipeline under test and layer responsibilities

```
bytes ──► (1) input gate ──► (2) saphyr-parser events ──► (3) spike-owned composer ──► (4) JSON projection ──► (5) jsonschema validate
          size limit         syntax + markers             docs, anchors/aliases,        policy: what is      Draft 2020-12, formats,
                                                          duplicate keys, depth,        JSON-representable   local refs only
                                                          alias budget, tags, scalars
```

| Layer | Responsible for | Not responsible for |
|---|---|---|
| (1) Input gate (spike code) | Reject input above the byte limit before parsing | — |
| (2) saphyr-parser 0.2.2 | YAML syntax, event stream, source markers, syntax errors with location | Duplicate keys, alias budgets, depth policy, projection — these belong to (3)/(4) and are **not** failures of the parser |
| (3) Composer (spike code) | Document separation (1-based index); depth limit; anchor table; alias resolution with cycle detection and expansion budget; duplicate-key detection **before** any map is built; scalar resolution (YAML 1.2 core schema); tag policy; source spans per node | Schema validation |
| (4) Projection (spike code) | Convert composed nodes to `serde_json::Value` only when safe; otherwise classify `unsupported_for_validation` with reason | Deciding validity of YAML |
| (5) jsonschema 0.60.1 (defaults off) | Draft 2020-12 validation, explicit format assertion, local `$ref` | Fetching anything; YAML semantics |

**Why a spike-owned composer instead of `saphyr` (the full crate):** the requirements in layer (3) are application policy (fail-closed limits, duplicate keys detected before lossy map insertion, exact spans, explicit tag/scalar policy). Whether `saphyr`'s loader enforces any of them is **not established** [known: research recorded only its crates.io metadata], and adopting it would still require verifying each behaviour, plus an extra dependency. A small composer over saphyr-parser events keeps every policy decision in reviewable spike code. `saphyr` is **not** requested. It would only be proposed, with your approval, if saphyr-parser's event API proves insufficient (§6 criteria).

Outcome categories recorded per case: `syntax_error` (with location), `limit_exceeded` (which limit), `policy_rejected` (e.g. duplicate key, cycle), `unsupported_for_validation` (valid YAML, not safely projectable), `projected` → then `schema_valid` / `schema_invalid` (with instance path) / `schema_unavailable`. **`schema_unavailable` and `unsupported_for_validation` are never a validation pass.**

## 2. Limits proposed for the spike (pipeline-enforced)

| Limit | Value (spike) | Enforced by |
|---|---|---|
| Input bytes per file | 1 MiB | (1) before parsing |
| Documents per file | 100 | (3) |
| Nesting depth (collections) | 64 | (3) on each collection start event |
| Alias events per file | 1,000 | (3) |
| Projected nodes per document (after alias expansion) | 100,000 | (3)/(4) |
| Scalar length | 64 KiB | (3) |
| Process | `timeout -s KILL 120`; `ulimit -v 1048576` (1 GiB address space) inside the namespace | outer watchdog / shell |

Production values are a P05 decision; these exist to prove enforcement.

## 3. YAML fixtures and acceptance cases

Fixtures are small text files committed under `spikes/rust/t06-yaml/fixtures/yaml/`, except the size and depth bombs and the alias bombs, which the harness generates at run time (`gen:`). No secrets are involved.

| ID | Content | Layer | Expected outcome |
|---|---|---|---|
| Y01 FX-YAML-VALID | flat mapping + nested map/seq | 2–4 | `projected`; values and types exact |
| Y02 FX-YAML-NESTED | 10-level nesting, block + flow styles | 2–4 | `projected`; equal to expected JSON |
| Y03 FX-YAML-MALFORMED | unclosed flow map; bad indentation; tab indentation | 2 | `syntax_error` with 1-based line/column matching the fixture's known error position |
| Y04 FX-YAML-MULTIDOC | 3 docs (`---`), doc 2 invalid by schema, doc 3 valid | 2–5 | per-doc results with indices 1–3; doc 2 `schema_invalid`, docs 1 and 3 `schema_valid` |
| Y05 multi-doc syntax error | doc 2 has a syntax error | 2–3 | doc 1 result kept; doc 2 `syntax_error`; later docs recorded as **not assessed** (not pass) |
| Y06 FX-YAML-DUPKEY | duplicate key top-level and nested; also `a:` vs `"a":` | 3 | `policy_rejected: duplicate_key` with both spans, detected before map construction |
| Y07 key collision on projection | `1: x` and `"1": y` (distinct YAML keys) | 4 | `unsupported_for_validation: key_collision_after_projection` |
| Y08 FX-YAML-SCALARS | `yes`, `no`, `on`, `off`, `True`, `TRUE`, `~`, `null`, empty, `0o17`, `0x1F`, `017`, `1_000`, `1e3`, `.inf`, `-.Inf`, `.nan`, `"true"`, `'123'`, `2026-10-24` | 3 | YAML 1.2 core schema: `yes/no/on/off` → **strings**; `True/TRUE` → bool; `~/null/empty` → null; `0o17` → 15; `0x1F` → 31; `017` → 17 (decimal int under the 1.2 core schema; recorded explicitly); `1_000` → **string** (1.2 core); `1e3` → float; quoted → string; date-like → string. The full mapping table is fixed in the harness and printed in the evidence |
| Y09 non-JSON numbers | `.inf`, `.nan`, integer above u64 | 4 | `unsupported_for_validation: non_json_number` (never silently changed) |
| Y10 FX-YAML-ALIAS-OK | anchor + alias reused 3× | 3–4 | `projected`; aliases expanded; spans of alias sites recorded |
| Y11 FX-YAML-ALIAS-BOMB (gen) | exponential nested aliases | 3 | `limit_exceeded: projected_nodes` or `alias_events`; bounded time/memory |
| Y12 FX-YAML-CYCLE | anchored node aliased inside itself (`&a [*a]`, and in a map) | 2–3 | if the parser accepts it: `policy_rejected: alias_cycle`; if the parser rejects it: recorded as `syntax_error` (either way never a pass, never unbounded) |
| Y13 undefined alias | `*missing` | 2–3 | `syntax_error` or `policy_rejected: undefined_alias` (recorded which layer) |
| Y14 FX-YAML-DEEP (gen) | depth 65 and depth 10,000 | 3 | `limit_exceeded: depth` without stack overflow or panic |
| Y15 FX-YAML-LIMITS (gen) | 1 MiB + 1 byte; 101 documents; 64 KiB + 1 scalar | 1/3 | `limit_exceeded` (`input_bytes` / `documents` / `scalar_length`) |
| Y16 FX-YAML-UNICODE | multibyte (é, 漢, emoji) and RTL text before a syntax error and before a schema-invalid value; CRLF line endings; UTF-8 BOM | 2–3 | reported line/column equal to fixture-declared positions; **both** byte offset and character column recorded; BOM not part of the first key |
| Y17 FX-YAML-ENCODING | invalid UTF-8 bytes; NUL byte | 1–2 | rejected with a diagnostic; raw bytes not echoed |
| Y18 FX-YAML-TAGS | `!!str 123`, `!!int "7"`, `!Ref x`, `!!python/object:os.system`, `!!binary` | 3–4 | standard core tags applied (`!!str 123` → "123"; `!!int "7"` → 7); custom/language tags → `unsupported_for_validation: tag`; never instantiated |
| Y19 non-string keys | `1: a`, `true: b`, `null: c`, `? [a, b] : d` | 4 | `unsupported_for_validation: non_string_key` (complex key distinguished) |
| Y20 merge key | `<<: *base` | 3–4 | `unsupported_for_validation: merge_key` (merge semantics not applied or guessed; decision for P05) |
| Y21 empty / comments-only / `---` only | | 2–4 | document count and null projection recorded; no false pass when a schema requires an object |

## 4. JSON Schema cases (jsonschema 0.60.1, `default-features = false`)

Schemas are committed under `spikes/rust/t06-yaml/fixtures/schema/`; the store-1.3 cases load the repository's canonical `schemas/workbench-store-v1.3.schema.json` and `schemas/wrapper-control-v1.1.schema.json` from local disk, passed to the validator in-process. Format assertion is **explicitly enabled**. Required formats are only those the current contract uses: `date-time` (`$defs/Timestamp`) and `uuid` (`$defs/Id`); regex `pattern`s are used by `GitOid` and `Digest` [known: our schema].

| ID | Schema / instance | Expected |
|---|---|---|
| J01 | Draft 2020-12 object: `required`, `type`, `additionalProperties: false` — valid + missing field + extra field + wrong type | valid / 3 distinct errors with instance paths |
| J02 | `enum`, `const` | valid / invalid |
| J03 | arrays: `items`, `prefixItems`, `minItems`, `uniqueItems` | valid / each violation detected |
| J04 | `format: date-time` | `2026-10-24T05:25:00Z` valid; `2026-13-40T00:00:00Z`, `2026-10-24 05:25` invalid **with format assertion on**; the same invalid values with assertion off → recorded as accepted (proves the switch matters) |
| J05 | `format: uuid` | valid UUID / malformed UUID (assertion on/off as J04) |
| J06 | `pattern` (GitOid, Digest) | 40-/64-hex valid; uppercase/short invalid |
| J07 | local `$ref` to `#/$defs/x` and nested `$defs` | resolved, valid/invalid as expected |
| J08 | `$ref: "https://example.invalid/s.json"` | schema build/validation fails explicitly → `schema_unavailable`; no network attempt (run in a network namespace; error class recorded) |
| J09 | `$ref: "file:///etc/hostname"` | `schema_unavailable`; file not read (resolve-file feature off) — evidence: error class; no file content in output |
| J10 | `$schema` of another draft (2019-09, draft-07) | recorded behaviour; our pipeline forces Draft 2020-12 and records any mismatch |
| J11 | Store 1.3 examples (Data Schema §8–§10) | valid |
| J12 | Same examples labelled `schema_version: "1.2"` | invalid (const) |
| J13 | Protocol 1.1 WrapperControl example (§23) | valid; with `protocol_version: "1.0"` invalid; missing `output_streams` invalid |
| J14 | YAML → projection → J01 schema (via Y01/Y04 instances) | end-to-end verdicts equal to JSON-input verdicts |
| J15 | Valid YAML + **unavailable** schema (J08 schema) | outcome `schema_unavailable`; **never** `schema_valid` |
| J16 | Differential check: J01–J07, J11–J13 verdicts compared with the host's Python `jsonschema` 4.26.0 (already installed, used only for comparison) | agreement recorded; disagreements reported, not hidden |

"Our schemas passed" is not standards conformance: the official JSON-Schema-Test-Suite is out of scope (it would need a download).

## 5. Minimal dependency, build and execution request (D3-3 for T06)

### 5.1 Direct dependencies

| Crate | Version | Features | Licence [known] | MSRV [known] | Why |
|---|---|---|---|---|---|
| `saphyr-parser` | `=0.2.2` | default (none besides optional `debug_prints`, not enabled) | MIT OR Apache-2.0 | 1.85.0 | event parser (layer 2) |
| `jsonschema` | `=0.60.1` | `default-features = false` (drops `resolve-http`, `resolve-file`, `tls-aws-lc-rs`, `idna`) | MIT | 1.85.0 | validator (layer 5) |
| `serde_json` | `=1.0.154` | default | MIT OR Apache-2.0 | 1.71 | `Value` for projection and for loading local schema files; already a transitive dependency of jsonschema [after A to confirm] |

Not requested: `saphyr`, `yaml-rust2`, `boon` (fallbacks), `serde` with `derive` (not needed: the harness builds `serde_json::Value` directly), `sha2`/`tempfile` (not needed). [known] Advisory status at research time: no RustSec entries for saphyr-parser, jsonschema or serde_json.

### 5.2 Steps (each exit code captured directly; same environment as D3-1)

| Step | Command (cwd `spikes/rust/`) | Network | Stop if |
|---|---|---|---|
| A | add member `t06-yaml` with the three exact pins; `cargo generate-lockfile` | crates.io sparse index | resolution needs anything other than crates.io; libc entry changes |
| A-review | list every locked package/version/source/checksum; diff against the committed D3-1 lockfile | none | git/path sources; unexpected TLS/crypto/native-link crates despite defaults off (e.g. `aws-lc-*`, `ring`); yanked versions |
| A2 advisory screen | read-only RustSec lookup for **every** newly locked package (advisory-db `crates/` listing + per-advisory pages for any match) — **needs approval as a lookup** | rustsec.org / api.github.com | unresolved advisory affecting a locked version → stop, propose alternative |
| B | `cargo fetch --locked` | static.crates.io | checksum mismatch |
| B-review | for each fetched package: does `Cargo.toml` declare `build`, `links`, `proc-macro = true`? list them; read any build script involved before C (pattern review, as D3-1) | none | build script with network/file-write/process behaviour beyond toolchain probing |
| C | `unshare -rn env -i … cargo build --frozen` | none (namespace) | — |
| D | spike run: `timeout -s KILL 120 unshare -rn --pid --fork --mount-proc --kill-child env -i PATH=/usr/bin:/bin LANG=C bash -c 'ulimit -v 1048576; exec <t06-yaml> <fixtures> <results.json>'` (env check first, as T04/T05) | none | — |

### 5.3 Known vs unknown

| Item | Status |
|---|---|
| Direct versions, licences, MSRV, default-feature contents | [known] research |
| saphyr-parser's own dependencies | **[after A]** (research did not fetch its dependency list) |
| jsonschema's dependency set with defaults off (count, regex engine, `referencing` etc.) | **[after A]** |
| Build scripts / proc-macros / native links in the graph | **[after B]** — likely at least the already-reviewed `libc` (via other crates) and `serde_json`'s build script, but not established |
| saphyr-parser event API: marker units (bytes vs chars), span start/end, anchor IDs, error locations, tag reporting | **[after C/D]** (no new docs lookup authorised) |
| jsonschema API: explicit format-assertion switch, in-process resource registration, error class for unresolvable `$ref` with resolvers disabled, draft selection | **[after C/D]** |
| Behaviour of all §3/§4 cases | **[after D]** |

### 5.4 Files and cleanup
- Repository: `spikes/rust/Cargo.toml` (add member), `spikes/rust/Cargo.lock` (resolved, reviewed), `spikes/rust/t06-yaml/{Cargo.toml,src/*.rs,fixtures/**}`; evidence `evidence/P00/P00-T06/`.
- Outside the repository: `~/.local/share/pwt-dev-tools/_cargo-home/` (new crate downloads), `_cargo-target/` (build output); nothing else.
- Generated bombs exist only in a run-scoped scratch directory under `~/.local/share/pwt-dev-tools/_spikes/`, removed and verified at the end (as T04).
- Rollback: delete `t06-yaml/`, revert the workspace manifest and lockfile hunk, remove the new crates from the isolated cargo home.

## 6. Primary-candidate failure criteria (fallback only with owner approval)

| Candidate | Fails if | Fallback that would be proposed |
|---|---|---|
| saphyr-parser | cannot report positions sufficient for Y03/Y16 (no line/column, or no way to obtain spans); does not expose anchors/aliases as distinct events; panics or is unbounded on Y11/Y12/Y14 before the composer can stop it | `saphyr` (full) or `yaml-rust2` — separate request |
| jsonschema (defaults off) | cannot validate Draft 2020-12 with local `$ref` and explicit `date-time`/`uuid` assertion without re-enabling network/file resolution or TLS features; fetches or reads anything for J08/J09 | `boon` — separate request |

## 7. Open questions for approval
1. Approve the §5 request (A → A-review → **A2 advisory lookup** → B → B-review → C → D) for exactly three direct dependencies, including implementing the spike composer/projection/harness and committing fixtures?
2. Accept the §2 spike limits and the §3 scalar/tag/merge-key/non-string-key policies as **spike** policies (P05 decides production)?
3. Approve using the host's existing Python `jsonschema` 4.26.0 for the J16 differential comparison (no installation)?
