# Status — PWT Java rebuild

Updated: 2026-11-12 (Phase 3). This file tracks the **Java rebuild** (baseline
docs `prd.md`/`tech-spec.md`/`architecture.md`/`data-schema.md`, v0.1 draft;
roadmap: Phase 3 integrations → Phase 4 MCP → Phase 5 desktop → Phase 6
hardening → Phase 7 release, see `plan.md` header note). The older Rust
P00–P03 track and its `workbench/` tree are preserved as reference only.

## Where the project stands

**Phase 3 implementation and verification are complete in the working tree:
121/121 tests pass, 0 skipped; all four remaining integrations run against
real tools/schemas with recorded versions. Not committed (no commit
requested).**

| Item | State |
|---|---|
| Phase 1 | Implemented + verified 2026-11-12: buildable project, JavaFX shell, safe discovery, headless discovery, MCP `pwt_info`. 33 tests. |
| Phase 2 | Implemented + verified 2026-11-12: yamllint + Gitleaks adapters, process runner, SQLite V1, reports, GUI/headless scan. 70 tests. |
| Phase 3 | Implemented + verified 2026-11-12 (this session): Checkov, actionlint, kubeconform, local-schema validation. **121/121 tests, 0 skipped.** |
| Phase 4 | Not started (next task at the bottom of this file). |

## Implemented behavior (Phase 3)

- **Checkov 3.4.4** (`--framework kubernetes`, `-f` per applicable staged
  file, stdout JSON): failed checks → MISCONFIGURATION findings (severity
  preserved or UNKNOWN — never invented); passed/skipped counted; exit 0/1 =
  executed, else failure. Scope: content-sniffed K8s manifests only.
- **actionlint 1.7.12** (`-format '{{json .}}'`, helpers disabled):
  syntax-check → SYNTAX/MEDIUM else LINT/LOW; `snippet` excerpts dropped.
  Scope: files directly in `.github/workflows/*.yml|yaml` (subdirs excluded —
  GitHub ignores them). Exit 0/1 = executed.
- **kubeconform (dev build, version `development`)**: `-output json
  -verbose`, explicit local `-schema-location` only (never network during
  scans); explicit `kubeconform-setup` action fills the managed cache.
  statusValid → no finding; statusInvalid → SCHEMA/MEDIUM with per-resource
  validation errors; statusError/Skipped → coverage gaps. K8s-manifest
  content sniff shared with Checkov.
- **local-schema (bundled networknt 1.5.9 + SnakeYAML 2.7)**: explicit
  `--schema-binding GLOB=PATH`, dialects 2020-12/2019-09; safe parsing
  (bounded aliases/nesting, dup keys → SYNTAX finding); multi-doc with index;
  findings carry binding/keyword/JSON-Pointer, never line numbers or instance
  values; remote `$ref` never fetched, local refs contained to schema root,
  remote `$id` + relative refs rejected.
- **Shared flow**: all six adapters run through the same snapshot → same
  SQLite records → same JSON (schema-validated, now incl. NOT_APPLICABLE) +
  Markdown reports → same GUI/headless summary. New `ScannerStatus.
  NOT_APPLICABLE` (neutral: correctly out of scope) vs SKIPPED/missing-config
  (breaks NO_FINDINGS → INDETERMINATE with gaps). No new store tables; V1
  migration untouched (status column has no CHECK constraint).
- **GUI**: six adapter checkboxes, bindings field, kubeconform version +
  strict, per-scanner outcomes list, all-six availability probing. **MCP**:
  `pwt_info` Phase 3 content, all six INTEGRATED; no scan tools (Phase 4).

## Files changed (working tree vs HEAD `c183267`)

- `M plan.md` — header note with the new phase order (old content untouched).
- `M prd.md` — (Phase 1–2 edits only; scope text already covers all six).
- `M pwt/pom.xml` — SnakeYAML 2.7 + networknt 1.5.9 promoted to main scope; version 0.3.0.
- `M Makefile` — jar 0.3.0 (scan target unchanged, still valid).
- `pwt/` sources: +8 main files (4 adapters, YamlSniff, LocalSchemaConfig,
  KubeconformSetup, ScanConfig.KubeconformConfig), extended ScanService/
  ScanCommand/PwtMain/GUI/reports-schema; +8 test files, 5 test resources.
- `pwt/README.md` — Phase 3 commands, bindings, setup action, applicability table.
- `status.md`, `handover.md` — this update. `?? ui_files/` — pre-existing, unused.

## Exact commands and actual results

```
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64; export PATH="$JAVA_HOME/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"   # gitleaks/actionlint/kubeconform/checkov live here
cd pwt && ./mvnw -B clean test     # Tests run: 121, Failures: 0, Errors: 0, Skipped: 0 — BUILD SUCCESS
./mvnw -B -q package -DskipTests   # target/pwt-0.3.0.jar (32 MB shaded)
java -jar target/pwt-0.3.0.jar gui
java -jar target/pwt-0.3.0.jar scan --root <dir> --file <rel>... [--adapter ...]
  [--schema-binding GLOB=SCHEMA ...] [--kubeconform-schema T] [--kubeconform-version V]
java -jar target/pwt-0.3.0.jar kubeconform-setup --k8s-version 1.29.0 --kinds Pod,Service [...]
java -jar target/pwt-0.3.0.jar discover|mcp
python3 /tmp/opencode/mcp_check.py    # ALL MCP CHECKS PASSED (v0.3.0, 6× INTEGRATED)
make scan ROOT=<dir> FILES="a.yaml"   # verified
```

- GUI: real :0.0 launch + screenshot; xvfb reflection probe (open/select/all
  six adapters/scan/FINDINGS/findings-table/reports): ALL CHECKS PASSED.
- Real-tool evidence: Checkov 3.4.4 (20 findings on privileged pod),
  actionlint v1.7.12 (syntax + rule findings), kubeconform vs real upstream
  schemas (Deployment invalid flagged, Pod valid) AND hand-written local
  schemas, local-schema vs real schemas (valid/invalid/multi-doc/syntax/
  dup-keys/refs). Reports validate against report-1.0.json.
- One honest skip in one run (upstream schema download timed out); re-ran
  green when network cooperated — recorded, not hidden.

## Dependency versions (all verified, none invented)

JDK 21.0.11-ea · Maven 3.9.11 (wrapper 3.3.2) · OpenJFX 21.0.8 (linux) ·
`io.modelcontextprotocol.sdk:mcp` 0.8.1 · Jackson 2.17.0 · SnakeYAML 2.7 ·
`com.networknt:json-schema-validator` 1.5.9 · slf4j 2.0.16 ·
`org.xerial:sqlite-jdbc` 3.49.1.0 · JUnit 5.13.4 ·
yamllint 1.38.0 · Gitleaks 8.30.1 · Checkov 3.4.4 · actionlint 1.7.12 ·
kubeconform (unversioned dev build, `kubeconform -v` → `development`).

## Known limitations (deferred, mostly Phase 6)

Phase 2 list still applies (no sandbox, best-effort cleanup, reconcile limits,
env sanitation, sqlite stderr warning), plus: Checkov policy set rides tool
upgrades (only `kubernetes` framework enabled); actionlint shellcheck/pyflakes
helpers disabled (reported in probe); kubeconform needs schemas cached or
configured or it reports gaps (by design); local-schema supports two dialects
only, no remote refs, YAML→JSON normalization documented in adapter (dates →
ISO-8601 UTC, binary → base64, non-string keys stringified). Network flakiness
to raw.githubusercontent affects only explicit setup/test downloads, never scans.

## Phase 3 acceptance

| Criterion | Result |
|---|---|
| Checkov real scans, exit codes, severity preserved-or-UNKNOWN | PASS (3.4.4) |
| actionlint real scans, format, helpers disabled, scope rule | PASS (v1.7.12) |
| kubeconform local schemas, strict/version config, statuses | PASS (setup action + gaps) |
| Local schema: bindings, dialects, multi-doc, refs, separation | PASS (13 tests) |
| Applicability vs availability vs success distinguished | PASS (NA/SKIPPED/UNAVAILABLE tests) |
| No second pipeline/storage/report format | PASS (same services) |
| Secret hygiene in new adapters | PASS (dropped snippets/matches, seeded-fake checks) |
| GUI + headless wiring for all six | PASS (probe + CLI runs) |
| Combined six-adapter run + report aggregation | PASS |
| Regressions (70 Phase 1–2 tests) | PASS (all still green) |

Whole-phase status: implementation + verification PASS; owner acceptance NOT
YET RECORDED. Do not begin Phase 4 without the owner.

## Next executable Phase 4 task: MCP scan/results/report tools

Implement the Phase 4 MCP toolset against the SAME SQLite store and data
directory: `list_repositories`, `list_runs`, `get_run`, `list_findings`,
`get_report` (paginated, bounded, sanitized), plus `start_scan`/
`cancel_scan` behind explicit user opt-in (`--allow-scans`, default read-only)
with validated repository IDs, relative paths, and adapter allowlists.
Verify with a real MCP client: GUI-created run retrievable, authorized scan
visible in GUI. No source-edit/Git tools. Keep `pwt_info` untouched.
