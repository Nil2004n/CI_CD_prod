# PWT YAML Security Workbench — Technical Specification

Consolidated revision 0.3. Includes all decisions through 23 October 2026: OpenRouter BYOK; local JSON/Markdown/PDF; named tool and wrapper plan; one core developer plus a Figma design team; Opus-led verification, Sonnet implementation, Haiku documentation and owner-approved debug cycles. This specification is not evidence of implemented code or completed tests. Earlier v0.1/v0.2 wording is superseded by this consolidated set.

Version: 0.3.5 (amendments CA-01, CA-02, CA-03, CA-04; store contract 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1) | Date: 2026-10-23 | Status: Proposed design for review

Change record 2026-10-31 (proposed, not implemented): **CA-05 automatic LLM evidence analysis** (PRD ANL-01–ANL-12; change contract `planning/CA-05-automatic-evidence-analysis.md`). Sections marked "CA-05" describe proposed behaviour pending owner approval; current contract versions above are unchanged until CA-05 is applied.
Related document: PRD v0.3. Requirement IDs refer to that PRD.

## 1. Technical decision

Rust with Ratatui, Crossterm, Tokio and Serde is the owner-confirmed reference implementation stack (D-01, 2026-10-23), with external scanners behind typed native adapters (D-03). All other dependency choices (YAML parser, schema validator, keyring, PDF renderer, process-tree control, HTTP client) remain subject to approved spikes. The first pilot release target is Linux x86_64 (D-02); the architecture and adapter interfaces stay portable, but Windows/macOS are extensions that are not verified or supported until their acceptance matrix passes.

Ratatui supports Linux, macOS and Windows. Its backend libraries handle terminal/input facilities; application code still owns event routing and interactions. [web:86][web:87]

Cross-platform means separate tested builds and compatible dependencies, not one executable for all operating systems or identical behaviour on every terminal.

## 2. Candidate stack

Versions must be selected, compatibility-tested and locked before implementation. No unverified “latest” dependency is a release requirement.

| Concern | Proposed selection | Design note |
|---|---|---|
| Language/build | Rust stable, Cargo workspace | Pin toolchain and dependency lockfile; review licenses/advisories |
| Dashboard | Ratatui + Crossterm | Panels/rendering plus input/resize; custom checkbox/hit-test logic |
| Async orchestration | Tokio | Candidate for tasks, process supervision, channels and timers |
| Data model | Serde + serde_json | Versioned typed JSON persistence and adapter DTOs |
| YAML parsing | Maintained parser selected by security/compatibility spike | Require multi-document support, diagnostics, duplicate-key policy, bounded aliases and source mapping |
| Schema validation | Candidate `jsonschema` plus framework validators | Not a substitute for dialect-specific semantics |
| File traversal | Candidate `walkdir` + explicit filtering | Checkout boundary and symlink policy enforced separately |
| Pattern matching | Candidate glob library | Stable project-relative path matching; no shell expansion |
| Git | Installed Git executable | Preserve user's existing credential mechanism; no embedded auth service |
| Time and IDs | UTC timestamp library + random UUIDs | Local timezone only for schedule/display; stable IDs for records |
| HTTP/provider API | Candidate `reqwest` with in-process permission checks | Direct approved OpenRouter HTTPS, streaming and enforced routing policy; not a hosted gateway |
| Logging | Structured sanitised application events | No unfiltered subprocess stdout/stderr logging |
| Storage | JSON files, app-owned reports/cache | No SQL database required in initial design |
| Testing | Rust unit/integration tests, snapshot/property tests as needed | Scanner fixtures and platform runners |

Candidate crates are selections for evaluation, not claims of complete integrated capabilities.

## 3. Deployment and prerequisites

Required: supported terminal, Git, configured Git credentials/credential manager for protected repositories, sufficient disk/network access, and enabled scanner/renderer dependencies.

- Package the core per OS/architecture.
- Maintain a dependency manifest identifying each scanner's version, origin, license and supported platforms.
- Preflight verifies tools and reports missing capabilities; never silently installs/executes arbitrary packages.
- External binaries or Python-based tools mean the complete product is not necessarily a single standalone binary.
- No background daemon; scheduler exists in the live application process only.
- Final supported OS versions, terminal matrix and packaging formats remain open beyond the Linux x86_64 first target.

Linux pilot prerequisites (D-05): scanners are separately installed by the user and detected by preflight; the application never installs, upgrades, bundles or uses `sudo`. yamllint and Checkov are Python tools: the prerequisite is a Python interpreter version that the pinned tool release declares supported, with each tool installed into an isolated environment (dedicated virtual environment or equivalent isolated installer) whose executable path is recorded in `tools.json`. Host Python is not assumed compatible: the planning host's Python 3.14 is unverified for the candidate versions until the approved spike checks it. Go/Rust release binaries (Gitleaks, actionlint, kubeconform) are installed from official releases with checksum/signature verification under a separate installation request (D-10).

## 4. Workspace layout proposal

```text
workbench/
  Cargo.toml
  crates/
    domain/          # IDs, records, states, invariants; no TUI dependency
    application/     # use cases, workflow/run coordinator, permission policy, analysis coordinator (CA-05, proposed)
    repositories/    # Git operations, checkout checks and inventory
    scanning/        # plans, adapters, parsing and normalisation
    retrieval/       # redaction, indexing, retrieval, provider integration
    persistence/     # schema checks, single writer, exports and recovery
    terminal-ui/     # Ratatui components, reducer, input routing
    cli/             # headless developer CLI (D-17, CA-04): same commands/events as the TUI; dev-only commands labelled
  tests/
    fixtures/        # synthetic safe repositories/configurations
    contracts/       # scanner output snapshots and schema checks
    integration/
```

Project structure is illustrative. Avoid splitting into microservices for the initial local product.

## 5. Repository acquisition and identity

1. Parse URL; reject embedded credentials, unsupported schemes and malformed input. Approved pilot transports are HTTPS and SSH (D-13); every other scheme (including `file://`, `git://` and `ext::`) is rejected.
2. Canonicalise provider/host/owner/path to detect duplicates, but use UUID as primary identity; preserve distinct repository refs in run targets.
3. Show acquisition destination and request required approval.
4. Clone into a unique app-managed directory; existing registered checkouts update under a per-repo exclusive lock.
5. Preflight local modifications, in-progress merges/rebases and untracked-overwrite hazards. Never stash/reset/force/clean automatically.
6. Proposed update policy: fast-forward-only pull. On divergence/failure, skip the repo for that run and preserve diagnostic evidence.
7. Capture exact commit OID after update and before inventory; prevent app-owned update/scan overlap.
8. Do not auto-init submodules, Git LFS downloads or alternate protocols without explicit capability approval.
9. Pilot Git safety controls (D-13), to be verified for the pinned Git version in the P00 Git spike before they are treated as effective: no submodule recursion; LFS smudge/filter-process execution disabled; repository and template hooks disabled; protocol allowlist limited to HTTPS/SSH; repository-local configuration not trusted to add helpers, filters or hooks. The user's existing credential helper or SSH agent is preserved. Where a control cannot be verified for a Git or helper version, the limitation is documented and the affected operation is refused or reported rather than assumed safe.

Execute Git with argument arrays and explicit working directory, never an interpolated shell command. Validate branch/ref input and URL options. Git inherits the approved credential mechanism; never capture credentials for storage.

Earlier plan used `repo_ext.sh` to extract names from `repo.json`. Proposed replacement: Rust reads `repo.json` directly and performs Git operations. Optional compatibility script may remain, but cannot be mandatory for platform independence without an explicitly supported shell prerequisite.

Repository provenance includes commit OID and file digest. A concurrent external edit should invalidate affected evidence or mark it stale rather than silently associating it with the wrong snapshot.

## 6. Discovery and scan planning

- Walk ordinary checkout paths including hidden configuration directories.
- Include `.yaml`/`.yml`; proposed case-insensitive extension matching, subject to review.
- Never delete non-YAML files. Exclude Git object metadata explicitly; do not silently apply `.gitignore` as scan exclusion.
- Default: do not follow symlinks; list excluded symlinks with reason. Any extension to in-root symlinks needs cycle/boundary tests.
- Enforce configurable file-size, file-count, depth and parse-resource limits; list skipped items as coverage gaps.
- Use forward-slash project-relative paths in stored records; translate to native paths only at filesystem boundary.
- Distinguish selected files, referenced context files, retrieved external inputs and generated/rendered artifacts.
- Reference resolution has depth/byte/time limits and requires approved network/data scope. Non-YAML context may be read only when needed and disclosed in the plan; it does not become a selectable YAML target.
- Freeze the run plan and workflow revision. New files require refreshed discovery/selection, not silent mid-run expansion.

## 7. Adapter contract

```rust
// Conceptual contract; not a version-pinned compilable public API.
trait ScannerAdapter {
    fn manifest(&self) -> AdapterManifest;
    async fn preflight(&self, env: &ExecutionEnvironment) -> PreflightResult;
    async fn assess(&self, request: ScanRequest, events: EventSink,
                    cancel: CancellationToken) -> AdapterResult;
}
```

Manifest fields: adapter ID/version, binary/dependency versions, supported platforms/frameworks/capabilities, input granularity, output schema version, network capabilities, command policy, license and limitations.

Request fields: run/task/repo IDs, frozen commit, selected targets, approved contextual inputs, framework/schema/renderer settings, resource budget and permission token.

Result fields: execution state, check outcomes, normalised findings, coverage ledger, sanitised errors, tool provenance and duration.

Rules:

- Never infer success from process exit code alone; map each tool's documented exit semantics and parse output.
- Preserve scanner findings even if exit code represents detection rather than infrastructure failure.
- Treat malformed output, missing evidence or tool crashes as failed/incomplete checks.
- Filter findings to selected targets; clearly label referenced artifacts or context findings. A directory-only scanner cannot silently redefine selected scope.
- Require adapters to honour approval settings; configure scanner-internal network access. Unsupported control means disable the capability or require a stronger execution boundary, not pretend it is constrained.
- Fixed allowlisted executables/arguments; no arbitrary command entry in saved workflows.

## 8. Named capability and wrapper matrix

| Adapter ID | Tool | Wrapper / native equivalent | Capability | Proposed release status |
|---|---|---|---|---|
| git | Git | `pull.sh` / core Git invocation | Clone new managed repos; safe update existing repos | Foundation |
| yamllint | yamllint | `scan_yaml.sh` | Generic YAML diagnostics; style separated from syntax | Pilot baseline |
| gitleaks | Gitleaks | `scan_secrets.sh` | Current-checkout suspected-secret detection | Pilot baseline |
| checkov | Checkov | `scan_misconfig.sh` | Security policies for supported configuration frameworks | Pilot baseline |
| actionlint | actionlint | `scan_actions.sh` | GitHub Actions structure/expressions; controlled embedded-script analysis | Pilot framework adapter |
| kubeconform | kubeconform | `scan_kubernetes.sh` | Versioned Kubernetes schema checks, approved CRD schemas | Pilot framework adapter |
| zizmor | zizmor | `scan_actions_security.sh` | GitHub Actions security-specific analysis, no fixes | Conditional extension X1 (owner D-04) |
| docker-compose | Docker Compose CLI | `validate_compose.sh` | Explicit Compose model validation, not security certification | Conditional on Compose scope |
| helm | Helm | `scan_helm.sh` | Chart lint and controlled local template rendering | Conditional on Helm scope |
| trivy | Trivy | `scan_referenced_images.sh` | Known vulnerabilities in approved referenced images | Conditional on image-vulnerability scope |
| rust-schema | Chosen Rust schema validator | In-process adapter, not mandatory shell | Automatic bindings/local supplied schemas for supported dialects | Pilot baseline |

Invocation (owner D-03, CA-01): the Linux pilot invokes every tool through an application-owned native Rust process adapter (Git directly). The named `.sh` wrappers are the optional parity extension X6, not a mandatory runtime layer; the tool-to-capability mapping above is retained. Exact versions, flags and supported-framework coverage are frozen only after the approved per-tool spikes; a baseline capability is never silently downgraded. The canonical pilot/conditional/deferred label register is PRD §16.

Versions and exact command flags are selected in a dependency spike, pinned and tested. This matrix is not proof of complete coverage. Standalone Checkov policies are the baseline; vendor-platform SCA/severity enrichments are not assumed free or enabled. Helm rendering, Compose validation and image scans are conditional. Generic parser validation and yamllint style diagnostics are kept distinct.

## 9. Syntax, schema and template details

- Syntax validity is not equivalent to secure semantics or schema conformity.
- Parse every YAML document separately with one-based document index.
- Proposed duplicate-key handling: explicit validation error, not silently using the last value.
- Preserve line/column mappings where possible; never fabricate a source line for generated artifacts.
- Store schema ID, dialect, content digest and origin. Cache only under approved data/retention rules.
- Schema `$ref` resolution is local/cache-first; external retrieval requires permission, including redirect destinations and transitive references.
- Render with explicit values/settings. Refuse arbitrary repository scripts, renderer plugins, hooks or command expansions.
- Local rendering must not contact a cluster by default. Any live check is a separate approved task.
- Record generated-artifact digest and source inputs. Generated files live in app-owned temporary storage, not the source checkout.

## 10. Concurrency, scheduling and cancellation

Proposed initial defaults for benchmarking, not contractual performance promises:

| Limit | Initial proposal |
|---|---|
| Active repo assessment slots | 2 |
| Total external scanner processes | 4 |
| Same-repo Git operations | 1 exclusive writer; no app scan/update overlap |
| Workflow overlap | Skip a new trigger if same workflow already active |
| Process timeout | Per adapter, visible and configurable; select during integration |
| Missed schedules while closed | Record on next open; no backlog replay by default |
| Waiting for approval | No child process started; no capability used until approved |

Use bounded queues; prioritise UI/approval events over verbose progress. Coalesce progress updates, never discard findings or permission decisions. Pause for approval at safe task boundaries. Stop cancels queued work and terminates child process trees using tested platform-specific handling. Already completed results remain.

On normal exit, request cancellation, persist state and restore terminal settings. On crash/restart, unfinished runs become interrupted, not completed. Cross-platform process-tree cleanup must be explicitly tested; it is not assumed from killing one PID.

## 11. Permission model

Capabilities: repository acquisition/update, external reference retrieval, vulnerability database/API access, schema/doc retrieval, external model/embedding transmission, local rendering and explicit non-mutating verification.

Scope includes workflow ID/revision (if any), repository IDs, category, approved hosts/resources, permitted data categories, purpose, validity and approval ID. Secrets are never an approved transmission category.

Automatic evidence analysis (CA-05, §26) uses the existing `model_transmission` capability with a matching inference scope (provider, model, routing policy, source IDs, redaction version). A saved workflow's workflow-scope grant lets analysis run without prompting; an ad-hoc run or a changed model/provider/policy/data category raises one approval request and the analysis waits (`awaiting_approval`) without blocking scanner evidence. Automatic does not mean pre-approved.

Every action produces a concrete permission request. Compare normalised arguments and destinations against grants; a changed workflow, provider, destination or data category requires fresh approval where scope changes. Denials are persisted without sensitive payloads. Do not treat “external access allowed” as a wildcard permanent network grant.

## 12. RAG subsystem

Scope note (2026-10-31, CA-05): this section describes **interactive chat**. The required automatic evidence analysis is specified in §26; it reuses the provider adapter, redaction and citation validation described here but never depends on a chat session. Chat can retrieve the saved analysis as AI-generated secondary material (`SourceRef.kind = ai_analysis`, proposed).

Runtime provider: OpenRouter with user-owned BYOK and an explicit configurable model. No PWT-hosted inference gateway. Claude Code is development tooling, not a runtime requirement. Keep a provider interface for future integrations.

1. User chooses repo/run/finding context.
2. Read eligible source/report/doc content; redact secrets before chunking.
3. Create chunks with source kind, repo/commit/run, file/range, content digest and redaction version.
4. Proposed baseline: local lexical retrieval over sanitised chunks. It provides retrieval without requiring an embedding service.
5. Optional embedding retrieval is a later/provider-specific implementation with separately approved disclosure. Merge lexical/vector rankings only after isolation and relevance tests.
6. Filter eligible chunks by explicit context before ranking; never retrieve unrelated repository content then merely hide its labels.
7. Enforce token/context budget; record selected source IDs without raw prompts by default.
8. Call approved local/API model and stream answer. Render citations as navigable evidence references.
9. Mark output as AI interpretation; do not turn it into an engine-confirmed finding.

No action-execution tools are exposed to the model. Untrusted content cannot approve capabilities. Report comparisons require compatible scope/rules or display comparison limitations. Code suggestions are text only; copying is user initiated. Raw chats are not persisted by default; approved sanitised persistence is an explicit option.

## 13. Persistence

Authoritative schema is specified in the Data Schema document using JSON Schema Draft 2020-12, a published schema dialect. [web:132]

- JSON-based store, not a relational server.
- Single application writer and exclusive application-instance lock.
- Validate -> write temporary file in same directory -> flush -> platform-correct replace -> retain recovery metadata as appropriate.
- Replacement/durability semantics require OS testing; do not promise transactionality across files.
- Recover stale in-progress runs and incomplete JSON/Markdown/PDF export sets at startup.
- JSON report is machine-readable evidence source; Markdown is derived from the same sanitised model.
- User-approved report naming: timestamp + repo name + run identifier, `.json`, `.md` and `.pdf`.
- `log.json` is valid JSON, not NDJSON. Retention/compaction limits remain a policy decision; writes are serialised.

## 14. Testing and release engineering

Required test suites:

- Domain state transitions and permission scope/revision matching.
- Path traversal, URLs containing shell metacharacters, symlink escapes and control-character injection.
- Malformed YAML, alias/resource bombs, multi-document files and duplicate keys.
- Synthetic secrets; raw values must not reach app logs, reports, RAG chunks or provider payloads.
- Adapter golden outputs: findings exit code, crash, timeout, truncated JSON, unsupported version and dependency absent.
- Workflow scheduling/overlap, DST/timezone behaviour, app-close interruptions and cancellation.
- Multi-repo isolation, selection semantics, template-source/generated mapping and schema fetch denial.
- JSON schema checks, write interruption/recovery, report/export-set consistency and migrations.
- TUI resize/input snapshots and manual mouse tests on approved terminals.
- (CA-05) Automatic evidence analysis with mock HTTP: trigger without chat, evidence-snapshot digest and scope binding, structured-output and citation validation (fabricated IDs, unsupported assertions), seeded-secret absence from request/record/report/index, prompt injection in excerpts and tool output, cross-repository isolation, provider denial/outage/timeout/rate limit, bounded retries and limits, cancellation/restart/idempotency, staleness and re-analysis, historical records without analysis, scanner usability when analysis cannot complete plus attempts/authority/recovery, AI-only finding rejection and the four separate dimensions (acceptance AC-01–AC-18, `planning/CA-05-automatic-evidence-analysis.md` §11). Model-quality evaluation is a separate, human-reviewed corpus exercise, not part of the deterministic suite.
- Platform-specific Git, dependency installation, subprocess cleanup and packaging.

Release artifacts: signed/checksummed builds as policy requires, dependency manifest/SBOM, locked versions, known limitations, installation/prerequisite guide, sample sanitised reports and schema migration notes. Scanner licensing and redistribution need review.

## 15. Performance and observability

Measure UI input-to-render latency, discovery throughput, queue wait, scanner runtime by adapter, peak memory, report-write latency, retrieval latency, analysis stage duration, attempts and model token/cost usage where the provider reports it (never estimated as fact). No secret values in metrics. Establish p95 targets on agreed hardware/data before sign-off; do not attach unsupported numeric guarantees.

## 16. Outstanding sign-offs

Resolved 2026-10-23: Rust/Ratatui reference baseline (D-01), Linux x86_64 first target (D-02), native adapters (D-03). Still outstanding: confirm exact OS/architecture/terminal and scanner matrix, YAML/schema/PDF dependencies, pinned tool flags/versions, OpenRouter model and provider policy, release licensing, retention and solo pilot cut. The developer owns coding; the separate team supplies Figma, not assumed Rust implementation.


## 17. Wrapper tree and runtime boundary

```text
scripts/
  preflight.sh
  pull.sh
  scan_yaml.sh
  scan_secrets.sh
  scan_misconfig.sh
  scan_actions.sh
  scan_actions_security.sh
  scan_kubernetes.sh
  validate_compose.sh
  scan_helm.sh
  scan_referenced_images.sh
  tests/                   # Bats tests, development only
```

Status (D-03): this tree is the optional parity extension X6, not part of the pilot runtime. The pilot invokes Git and every scanner through native Rust process adapters that implement the same protocol DTOs in-process. ShellCheck/Bats apply only to scripts actually implemented.

Scripts are application-owned wrappers, not the vulnerability engine. Rust owns selection, approval, process supervision, native-output parsing, normalisation, persistence and UI events. Direct executable adapters provide equivalent operation on native platforms; mandatory Bash/Make is not a platform-independent installer. If a shell route is used on Windows, document/test its prerequisite rather than pretending it is native.

Clone first; fast-forward-only update on existing checkouts. Do not `git pull` into an absent repo or delete non-YAML files. `pull.sh` reads only validated core-provided arguments; the core reads `repo.json`. No shell interpolation of user URLs/refs/paths. Disable unapproved Git hooks/filters/helpers/submodules and unexpected protocols while retaining the approved credential mechanism. Scan/update locks and working-tree digest checks preserve provenance.

## 18. Wrapper protocol and tool-output mapping

Wrapper protocol v1.1 (v1.0 + pre-implementation amendment CA-01 + versioned amendment CA-02), distinct from store schema v1.4 (CA-03, Data Schema §24). Native adapters use the same `WrapperRequest`/`WrapperControl` DTOs in-process; stdin or an app-owned request file is the transport only for the optional `.sh` extension.

Requested scope (CA-01, resolves C3): every request carries `category` (the single assessment `Category` the task is asked to produce, e.g. `secrets`) and `required_capabilities` (the permission `Capability` values the task may exercise; empty means local-only with no network). The core must hold a valid grant in `approval_ids` covering each listed capability before launch; the adapter rejects work outside `category`.

Timeout and cancellation contract (CA-01): `timeout_seconds` and `termination_policy` (`control="supervisor_out_of_band"`, `grace_ms`, `kill_scope="process_tree"`) are declarations, not enforcement. The core process supervisor owns the timer and the cancellation token; on timeout, cancel or output-limit breach it requests graceful termination of the process tree, waits at most `grace_ms`, then forcibly kills the tree using platform-specific handling verified in the P00 process spike. `WrapperControl.termination` records the observed `reason`, `method` and whether the tree was confirmed exited (`null` when unknown); `transport_status` adds `output_limit_exceeded`. A JSON field alone never stops a running process.

CA-02 (Data Schema §23; protocol 1.1 / store 1.3): the direct child's exit code and terminating signal are recorded separately (`tool_exit_code`, `tool_term_signal`); cleanup of leftover process-group members is recorded in `termination.descendant_cleanup` without overwriting the primary `reason`; per-stream reader outcome is recorded in `output_streams` (`not_applicable` for spawn failures).

Output-limit requirement (P06, from P00-T05 finding F-1): the supervisor enforces consumption and retention budgets **at the reader boundary**: a reader stops consuming at its budget and notifies the coordinator immediately (event, not periodic polling); the coordinator starts bounded termination/escalation; pipes are closed or drained within a bounded byte/time budget without deadlock; truncation and output-limit termination are recorded explicitly. Measured quantities are kept distinct: bytes consumed by readers, bytes retained in memory, bytes persisted in durable evidence (sanitised), and child-generated output (not knowable beyond what was consumed). The supervisor cannot guarantee that a child produces no bytes beyond the limit; the acceptance target is bounded and documented overshoot (proposed in `plan.md` §5.2, FR-01).

- Core provides a validated request JSON via stdin or app-owned request file: request/task/run/repo IDs, checkout root, frozen file IDs/paths/digests, capability, approved context/network resources, config identity and resource limits. Never include the LLM key.
- Wrapper/native adapter has a pinned executable and argument template; do not accept an arbitrary executable, shell string, plugin directory or repo-authored script.
- Wrapper reports transport metadata (adapter/tool/version/request ID, invocation success, underlying exit, timeout/cancel state) through a bounded structured control channel. Native findings are parsed using the pinned tool parser, not concatenated raw logs.
- Use separate control/findings channels or app-owned output descriptors. A wrapper must never mix human banners, raw secrets and JSON on one undocumented stdout stream.
- Wrapper success is not security success. Preserve actual tool exit code and parsed findings. Every adapter version documents nonzero findings versus execution errors; no universal rule such as “all nonzero means failed.”
- Suppress streaming raw stdout/stderr into terminal/logs. Parse/redact promptly; durable output contains only sanitised records. If a tool requires sensitive temp output, either use tested tool redaction or explicitly approved restricted transient storage with cleanup; no assertion that ordinary unlink securely erases SSD data.
- Strip unrelated environment secrets from child scanners. Do not export the OpenRouter key globally. Only Git gets the minimum credential access it needs; scanner-specific network credentials require separate scope.

Per-tool parser plans:

| Tool | Input / result plan | Special handling |
|---|---|---|
| yamllint | Explicit files, pinned machine-readable/parsable diagnostics | Separate style from syntax; honour template classification |
| Gitleaks | Current-tree selected-file scope; JSON with tested redaction | Do not silently enable Git history or repo allowlists; verify report redaction, not just terminal redaction |
| Checkov | Supported file/framework scope; JSON | Fixed app config/custom-policy policy; platform APIs disabled unless explicit integration approval |
| actionlint | Explicit workflow files; documented JSON output template or tested structured parser | ShellCheck/Pyflakes helpers are declared optional executables; do not run workflow commands |
| zizmor | Explicit workflow scope; supported structured/SARIF format | No fixes; do not infer clean result from SARIF exit status alone; network-backed audits separately approved |
| kubeconform | Kubernetes resources; JSON diagnostics, strict/schema-version policy | Local/cache-first schemas; missing CRD schema is a gap, never pass |
| Docker Compose | Explicit config/value files; validation status and sanitised diagnostics | Quiet/no content dump; variable/include resolution stays inside approved inputs; no up/build/run |
| Helm | Explicit chart/values; lint outcome and local rendered artifact | No install/upgrade/cluster contact/plugins/hooks; dependency fetch separate; preserve source mapping only when real |
| Trivy | Explicit approved referenced images, resolved digest and JSON | Vulnerability capability only when that task is selected; database/registry access/freshness recorded; no cluster scan |
| Rust schema | Local/approved schema, chosen dialect and YAML projection | Non-JSON YAML/tag/type limitations explicit; approved `$ref` resolution only |

Repository-config and scope hypotheses (C1/C11) are verified per pinned tool, not assumed: for each tool the P00 spike records which repository config/ignore/allowlist/plugin files it auto-loads (for example `.yamllint*`, `.gitleaks.toml`/`.gitleaksignore`, `.checkov.y*ml`, `.github/actionlint.y*ml`), whether an explicit app-owned config fully neutralises them, whether explicit file arguments are honoured or the tool walks directories (requiring staged selected-file copies), how findings appear in its output with and without its redaction option, and its exact exit-code semantics. No single universal override flag is assumed to solve these for every tool; a tool that cannot be controlled has that capability disabled with the reason shown.

Do not fabricate current flags. Each pinned version must have a contract test proving input/output/exit behaviour before release. Directory-only tools require a safe selected-file staging strategy or an explicit wider read scope, not scanning everything and merely hiding unwanted findings.

## 19. Routing and security assessment policy

```text
Selected ordinary YAML -> parser/yamllint + Gitleaks
GitHub workflow        -> actionlint + supported Checkov + optional zizmor
Kubernetes manifest    -> kubeconform + supported Checkov
Compose config         -> approved Compose validation + verified applicable policies
Helm chart             -> source checks + approved Helm renderer -> Kubernetes adapters
Image reference        -> approved resolution -> optional Trivy task
Unknown configuration  -> applicable generic checks; framework/security/schema gaps explicit
```

Use path plus content cues; do not classify only by filename or assume every YAML file is Kubernetes. Never invoke arbitrary embedded `run` commands. App-controlled configuration must prevent hidden repository exclusions/executable policy loading; if an integration cannot safely control these, disable that capability and report why.

Cross-engine duplicates retain engine/check provenance; deduplicate only compatible rule/resource identities. A lint style warning is not a vulnerability, a CVE is not a syntax error, and render success is not schema/live validation.

## 20. OpenRouter BYOK and RAG

Credential mode: session-only (pilot) or OS credential store (conditional on the Linux keyring spike verifying a supported backend); no plaintext fallback, shared key or PWT account. Config contains a reference, not a secret. Origin: `https://openrouter.ai/api/v1`; do not forward auth headers across unapproved-origin redirects.

```json
{
  "provider": {
    "data_collection": "deny",
    "zdr": true,
    "allow_fallbacks": false
  }
}
```

`allow_fallbacks` is `const false` in the store schema (CA-01) and is also enforced in the request builder; neither layer alone is relied upon. Explicit model and optional provider allowlist. The model ID is supplied by the owner/user; development agents do not invent or silently choose a production model ID. Empty internal allowlist means omit `only`, not generate a wildcard. No automatic model/policy downgrade. OpenRouter intermediary/downstream access is disclosed; routing settings are not a claim that all intermediary metadata disappears. No tools/web/plugins by default.

Local lexical retrieval filters context before ranking, then redacts/chunks with source/commit/run provenance and budgets tokens. Validate citation IDs, stale-source warnings and cross-repo isolation. Seeded secret redaction tests are mandatory, not a universal confidentiality guarantee. Raw prompts/response bodies are not routine logs. Assistant has no command or write tools.

Handle invalid key, credits, rate limits, incompatible/private route, streaming truncation, timeout and cancellation distinctly. Bound retries; avoid blindly duplicating uncertain billable generations. Requested/actual model/provider metadata recorded when actually returned. Use mocked HTTP for development; real requests need owner approval.

## 21. Deterministic reports and restricted PDF

Normalised evidence -> JSON evidence report -> automatic evidence analysis (§26) -> combined deterministic Markdown -> local PDF. The JSON evidence report never contains or waits for AI content. The combined Markdown/PDF renders, without AI, the evidence plus the validated analysis or an explicit analysis-stage notice (`blocked`, `failed`, `awaiting_approval`, …). No key/model outage can block scanning or the evidence report; it makes the assessment `incomplete` and says so. A new analysis version produces a new export revision, never a silent mutation of accepted history. (Amended 2026-10-31, CA-05; the earlier "optional cited AI narrative" is superseded.)

The renderer is not yet selected (D-07). Candidates are compared in an approved spike against restricted-rendering requirements, runtime dependencies, platform implications and packaging; a tool being installed on the planning host (e.g. WeasyPrint) is not a selection criterion. Spike/select a renderer with Unicode/table support, license/runtime and platform packaging checked. Disable active HTML/scripts, automatic remote assets and unrestricted local filesystem references. Bound process resources; check nonempty output before success. JSON/MD/PDF have independent manifests/digests/errors; retry derived exports from retained evidence without rescanning. Keep the installation directory separate from writable app data.

## 22. Development verification tools and regression policy

| Tool/test layer | Purpose | Customer prerequisite? |
|---|---|---|
| Cargo fmt / Rustfmt | Rust formatting | No |
| Clippy | Rust lint/static checks | No |
| Cargo test/build | Unit, integration, regression and build | No for prebuilt release |
| cargo-audit | Published Rust dependency advisory checks | No |
| ShellCheck | Application-owned wrapper analysis | No; optional actionlint helper separately declared |
| Bats-core | Bash wrapper behaviour, fake commands and exit/output contracts | No |
| Fixtures/golden snapshots | Scanner parser, scope, redaction, schema, UI and report regressions | No |
| Mock HTTP | OpenRouter auth/privacy/stream/error cases without real credentials | No |

ShellCheck and Bats-core are required only for shell scripts actually implemented (D-03); the pilot baseline has none. After workspace creation, verify canonical commands before calling them valid: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace`, `cargo audit`; `shellcheck scripts/*.sh` and `bats scripts/tests` once paths/files exist. Document network/dependency needs; do not execute blindly during planning.

Negative cases: missing dependency, findings exit vs crash, timeout/kill, malformed/oversized output, secret-bearing stderr, hostile filename/ANSI, repo config injection, schema denial, stale CVE data, missing CRDs, source-template vs YAML distinction, model outage, raw secret in PDF context, filesystem escape, interrupted write/run, same-day report collision and UI mock/real separation.

Relevant regression runs with feature/debug verification; full agreed regression and manual OS/TUI/PDF checks gate release. Do not weaken expected behaviour, skip failed checks or declare manual tests performed from snapshots.

## 23. Distribution and licence requirements

Standalone open-source engines are the baseline; no per-scan licence fee is assumed for these components. This does not include OpenRouter usage, coding-agent access, hosted vendor enrichments, CI/storage or Docker Desktop entitlement.

Provisional licence inventory to verify for pinned distributions: yamllint and ShellCheck GPL-3.0 family; Gitleaks/actionlint/zizmor/Bats-core MIT; Checkov/kubeconform/Trivy/Helm/Docker Compose Apache-2.0. Record exact SPDX/version/license/NOTICE/source obligations before redistribution; do not treat this provisional summary as legal clearance or a transitive-dependency audit. Bundled binaries, separately installed tools and linked libraries have different review implications. Docker Desktop has separate subscription terms; do not make it a mandatory presumed-free dependency.

Ship `THIRD_PARTY_NOTICES.md`, necessary licenses/source offers and a versioned dependency/tool manifest with binary origins/checksums and supported platforms. No automatic dependency download or installation without approval. Signature/checksum policies and secure updates are release decisions.

## 24. Solo Claude Code development and Figma integration

A feature phase approval authorises its bounded implementation plus the explicitly listed local functional/regression checks. Opus delegates implementation to Sonnet 5.5, inspects the integrated changes and invokes the real verification commands. It diagnoses confirmed failures versus suspected risks and proposes a bounded solution. Haiku drafts/updates `status.md` and `handover.md` from Opus's evidence packet; Opus validates the diff before the checkpoint is accepted. STOP for the owner. No failed test automatically authorises a fix. An explicitly approved debug cycle authorises the named repairs plus listed retests; repeat only with new owner approval. Starting the next feature requires approval even if every mandatory test passes. After two unsuccessful cycles on the same defect, escalate the diagnosis and scope rather than retry indefinitely. This replaces the earlier requirement for a separate human pause between implementation and its already-authorised tests.

Workers receive phase/cycle ID, current revision, allowed files, contracts, tests, tool/network scope and a stop condition. Start with one mutating worker; use at most two disjoint writers once stable. Shared interfaces/lockfiles have one owner. Use reviewed branches/worktrees, verify starting commit and integrate before final tests. Main Opus invoking actual checks is the verification authority, not a Sonnet success message.

Haiku may edit only status/handover and specified report drafts from Opus's sanitised packet. It cannot decide pass/fail, classify vulnerabilities as confirmed, record invented approvals, edit source/plan/specs or advance phases. Opus reviews/reconciles its changes. Native permissions and owner review are needed; Markdown instructions are not enforcement/sandboxing.

Root plan includes implementation and preauthorised verification in each feature phase, followed by stop/report. Owner-approved debug cycles include scoped fixes and retests, followed by stop/report. Fresh review checks requirements/coverage; passing tests alone do not certify safety. Manual blockers remain open. Two unsuccessful cycles -> revised diagnosis/owner decision.

UI team supplies Figma frames, tokens, states and interactions; developer implements demo and final TUI. Use terminal-cell layout, stable mock/application contracts, no Figma auto-code assumption. Design changes are presentation-only unless a contract change is proposed/approved. Preserve live scan state and selections through restyling.

Root controls: concise CLAUDE.md, owner-approved plan.md, compact status.md and handover.md. Opus validates Haiku drafts and genuine approval records. On resume inspect Git/dirty worktree, active phase/cycle and changed/stale evidence, loading relevant spec sections only. No zero-token claim and no assumption that five agents equal five engineers. Four-week cap is a scoped pilot target.

## 25. Versioned Kubernetes schema cache (planned, D-11)

kubeconform needs Kubernetes JSON schemas. The pilot plans an approved, versioned, local cache; no download or bundling is authorised until a separate approval.

| Aspect | Planned rule |
|---|---|
| Origin | A named upstream schema source recorded with URL, publisher and retrieval revision; candidates are evaluated in the spike, none is assumed |
| Target Kubernetes version | One or more explicitly selected minor versions recorded in `tools.json` and each check's engine snapshot; no implicit "latest" |
| Licence / redistribution | Licence of the schema set verified before any redistribution; until then the cache is user-populated, not shipped |
| Integrity | Cache manifest with per-file SHA-256 and retrieval timestamp; mismatches make the cache unavailable, not partially trusted |
| Freshness | Retrieval date and target version shown in diagnostics and reports; staleness thresholds are an open policy decision |
| Network | Populating or refreshing the cache is a `schema_fetch` operation requiring approval; scans use the cache only (`network_policy="cache_only"`), verified per pinned kubeconform version |
| Missing CRD / kind | A resource without a cached schema yields `unsupported` (gap), never `passed`; user-supplied CRD schemas are local inputs with recorded digest |

## 26. Automatic LLM evidence analysis (CA-05; design direction accepted with qualifications 2026-10-31; not implemented)

Authoritative requirement and full tables: `planning/CA-05-automatic-evidence-analysis.md` r2 (ANL-01–ANL-12, four dimensions, attempts and authority, failure table, acceptance AC-01–AC-18). Proposed limits and retry ownership (not ratified): `planning/CA-05-decision-proposals.md` §1. Draft store definitions: Data Schema §26. Canonical schema/application changes are not authorised yet; the lead contract task precedes any dependent implementation.

1. **Trigger.** When a per-repository evidence report JSON is finalised and validated, the coordinator asks the analysis coordinator to start attempt 1 of a new logical analysis. Eligibility is deterministic: explicit model ID, credential present, valid `model_transmission` grant with matching inference scope, snapshot within limits, useful evidence present, report digest matches. Missing grant → `awaiting_approval` (one approval request; approval resumes the same attempt); other unmet conditions → terminal `blocked`/`not_performed` with a typed reason. No chat session or manual action is involved.
2. **Evidence snapshot builder** (deterministic, offline). Reads only that report: findings (redacted evidence, rule, engine/version, severity, locations), check outcomes and execution failures, coverage gaps and unsupported/skipped checks, suppression data when RES-06 exists, engine/rule/schema versions and freshness, plus bounded approved redacted excerpts around cited locations (from the checkout at the report's commit, through the redaction module) and local approved rule/schema descriptions. Priority under the byte budget: execution failures, coverage gaps and finding headers first (mandatory), then excerpts by severity. Eligible and included counts and exclusions (with reasons) are recorded; they determine **AI evidence coverage** (`complete` / `partial` / `none`), which is separate from scanner coverage, workflow completion and finding presence. Canonical encoding → SHA-256 snapshot digest. The snapshot is not persisted verbatim; its manifest is.
3. **Request.** Through the shared model adapter and network gateway: explicit model, routing policy `data_collection=deny`, `zdr=true`, `allow_fallbacks=false`, no tools/plugins/web, structured-output schema (analysis-schema version) where supported, versioned prompt template stating that the evidence is untrusted data. Repository text and tool output are delimited data and cannot change instructions, scope or approvals.
4. **Validation.** Parse against the analysis schema (no field can express a check outcome, severity change, executed test or resolution). Every citation and finding/check ID must be in the snapshot; recommendations must reference a finding or check. Free-text rule checks flag claims that a check passed, a test ran or an issue is resolved. In the pilot any independent AI-only finding (a problem no scanner reported, presented as a result) is rejected (`ai_only_finding_out_of_scope`); cited interpretation, recommendations and uncertainty are kept. Rejected items are counted; an unsupported summary or schema failure allows at most one repair request, then `failed`. Only a validated response completes an attempt.
5. **Limits and retry ownership.** Snapshot bytes, excerpt count/size, output tokens, response bytes, connect/first-byte/total timeouts, ≤ 2 transport retries with bounded backoff and bounded `Retry-After`, ≤ 1 repair, ≤ 4 provider requests per attempt, per-run logical-analysis and request caps, optional reported-cost cap (values proposed in `CA-05-decision-proposals.md` §1, not ratified). The HTTP library's own retries and redirects are disabled; the coordinator owns and records every request. Transport retries only when no generation happened (connection failure before sending, 429, 502–504 before any body). Timeout or disconnect after sending → `uncertain_generation`, no automatic retry. No provider or model fallback. The idempotency key (report digest, snapshot digest, template, schema, redaction version, model, policy digest) prevents a new request for unchanged inputs that already have a completed attempt.
6. **Attempts and persistence.** Each attempt ends in one immutable `AnalysisFile` record written through the single writer, then the run log manifest (per logical analysis: attempts and `completed_record_id`). Non-terminal state exists only in the manifest. A write failure halts the writer (P02 rule); the attempt is not completed. Raw prompts and responses are not persisted; usage/cost only when the provider reports it. The scanner evidence report is never modified.
7. **Recovery.** At startup every non-terminal attempt is written as `interrupted`/`application_interrupted`, recording an in-flight request as possibly billed. No automatic replay. Pending approvals never become grants.
8. **Authority, retry and re-analysis.** Only attempt 1 starts automatically. `ReanalyseReport` adds an attempt to the latest logical analysis when it has no completed attempt and unchanged inputs (`explicit_retry`), otherwise creates a new logical analysis with `analysis_version + 1` and `supersedes_analysis_id` (`explicit_reanalysis`; for unchanged inputs only after the user confirms the possible charge). The authoritative analysis of a report is the completed attempt of the highest version that has one. Staleness is derived at read time and never written back. No exactly-once execution or billing is claimed.
9. **Reports, history and RAG.** The combined Markdown/PDF renders validated content deterministically with origin labels and the four dimensions, or an explicit stage notice; each export records the analysis record it used. Store 1.4 `ai_enrichment` content is presented as a legacy narrative "not validated under CA-05", distinct from absent AI content and from a CA-05 analysis; it is never certified or deleted. RAG may index the authoritative analysis as `ai_analysis` secondary chunks under the same redaction and isolation rules.
10. **Evaluation.** Deterministic tests use mock HTTP. Model quality is evaluated separately (O-AN-6 proposal: synthetic 30-case corpus, expected-outcome sheets, rubric, blind human review, thresholds). Real-provider runs need separate owner approval of the request in `CA-05-decision-proposals.md` §3.
