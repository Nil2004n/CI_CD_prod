# Handover — PWT Java rebuild, Phases 1–3

Checkpoint: 2026-11-12 (Phase 3). All six scanner integrations are
implemented and verified in the working tree (branch `phase/P04-git-acquisition`,
HEAD `c183267`, uncommitted by instruction). Next step needs the owner: accept
Phase 3 or direct Phase 4. Live state: `status.md`.

## 1. What exists and where (new in Phase 3)

- `pwt.adapters.CheckovAdapter` — kubernetes-framework scans, `-f` per
  applicable file, stdout JSON, severity preserved-or-UNKNOWN.
- `pwt.adapters.ActionlintAdapter` — `-format '{{json .}}'`, helpers off,
  `.github/workflows/*.yml|yaml` top-level rule, snippets dropped.
- `pwt.adapters.KubeconformAdapter` — `-output json -verbose`, local
  `-schema-location` only, statuses → findings/gaps; `countCachedSchemas`.
- `pwt.adapters.LocalSchemaAdapter` — networknt + SnakeYAML, bindings,
  2020-12/2019-09, ref policy, per-doc validation, no lines/values.
- `pwt.adapters.YamlSniff` — shared safe k8s-manifest sniff.
- `pwt.application.LocalSchemaConfig` — bindings + dialect validation.
- `pwt.application.ScanConfig` + `KubeconformConfig` (template/version/strict).
- `pwt.launcher.KubeconformSetup` — explicit schema download action.
- `pwt.domain.ScannerStatus.NOT_APPLICABLE` (neutral) — report schema updated.
- GUI: adapter checkboxes, bindings field, kubeconform version/strict,
  outcomes list, six-way availability. `pwt_info`: Phase 3, all INTEGRATED.
- Tests: Checkov/Actionlint/Kubeconform/LocalSchema parser + real-tool suites,
  setup command tests, 5 schema/schema-template fixtures under
  `pwt/src/test/resources/{localschema,kubeconform-local}/`. 121 total.
- `phase/P04` branch/HEAD unchanged; old Rust tree untouched.

## 2. How to verify (from repo root)

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64; export PATH="$JAVA_HOME/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"   # checkov/actionlint/kubeconform/gitleaks
cd pwt && ./mvnw -B clean test        # expect 121/0/0/0, BUILD SUCCESS
./mvnw -B -q package -DskipTests      # target/pwt-0.3.0.jar
java -jar target/pwt-0.3.0.jar kubeconform-setup --k8s-version 1.29.0 \
  --kinds Pod,Service --data-dir <dir>     # explicit schema cache fill
java -jar target/pwt-0.3.0.jar scan --root <dir> --file <rel>... \
  --schema-binding 'GLOB=/abs/schema.json' # same services as GUI
java -jar target/pwt-0.3.0.jar gui    # needs display; screenshot-proven 2026-11-12
python3 /tmp/opencode/mcp_check.py    # independent MCP harness (in /tmp)
```

Toolchain reality: default `java` is 25 — do not use; Maven only via wrapper;
kubeconform reports version `development`; real X on `:0.0`, `xvfb-run` OK.

## 3. Evidence from this session (all in `/tmp/opencode`, not in git)

- `mcp_check.py` (v0.3.0, 6× INTEGRATED) — ALL PASSED.
- `GuiProbe.java` + run — six-adapter in-GUI scan (ALL CHECKS PASSED).
- `pwt-gui3.png` — Phase 3 screenshot; `KcDebug/LsDebug/RefDebug/SvcDebug.java`
  — throwaway root-cause drivers (kubeconform template casing, networknt
  SchemaLocation+config, $id base-URI, gap propagation).
- `kube-schemas/` — real upstream schemas downloaded explicitly (Deployment,
  Pod, Service 1.29.0); `kclocal/` — local-template proof.
- Fixtures: `mix/`, `mixdata/` (combined six-adapter CLI runs).

## 4. Traps for the next implementer

- kubeconform lowercases `{{.ResourceKind}}`: local schema files must be
  lowercase (`widget.json`, NOT `Widget.json`); full versioned template shape
  is `.../{{.NormalizedKubernetesVersion}}-standalone{{.StrictSuffix}}/
  {{.ResourceKind}}{{.KindSuffix}}.json`.
- A schema-location that is not a full `...json` template gets the default
  template APPENDED (kubeconform registry.go) — always pass full templates.
- networknt 1.5.9: `getSchema(String)` parses CONTENT — load files via
  `getSchema(SchemaLocation.of(uri), SchemaValidatorsConfig.builder().build())`.
- `$id` sets the ref base URI (a remote `$id` + relative refs = network
  fetch); vetting rejects that combination, not every remote `$id`.
- ScannerOutcome now has 8 components incl. `coverageGaps`; ScanService
  collects them into run gaps (SUCCEEDED outcomes can carry gaps).
- `checkov -d` on a missing dir exits 0 — never trust exit codes without
  parsing; checkov stdout JSON needs `--quiet` + `-o json` (no file).
- Gitleaks uses `--exit-code 7`; yamllint non-strict 0/1 = executed.
- Phase 2 tests pin `--adapter yamllint,gitleaks` for clean-run expectations;
  bare defaults now include all six (honest INDETERMINATE with gaps).
- `System.setProperty("pwt.tools.<id>", ...)` must be cleared in `finally`.
- Never assert on `~/.pwt-data`; every test uses `@TempDir`.

## 5. Deferred to Phase 6 (do not sneak into Phase 4/5)

Fault injection, sandboxing, secure deletion, crash/orphan recovery, output
tuning under adversarial tools, YAML parser abuse limits, Checkov framework
expansion, actionlint helper integration, broader kubeconform kind coverage,
remote-ref support, cross-platform validation, performance budgets, log-leak
fuzzing.

## 6. Next task (Phase 4, per status.md)

MCP `list_repositories/list_runs/get_run/list_findings/get_report` plus
opt-in `start_scan/cancel_scan` (`--allow-scans`, default read-only) over the
same SQLite store; real-client verification incl. GUI-visible authorized scan.
No edit/Git tools. Keep `pwt_info` as-is.
