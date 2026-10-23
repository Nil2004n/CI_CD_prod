# P00-T01 — Spike protocol sheets

Task: P00-T01 (owner-approved 2026-10-24). Plan reference: `plan.md` §5, §5.1. Base revision `1e4a5b3`.

Status: **protocols only.** No spike has been executed. Nothing here is a verified tool behaviour. Flags, file names and exit codes named below are **candidates from general prior knowledge**, written so the spike knows what to probe; every one must be confirmed or refuted against the pinned version's own documentation and observed behaviour, and the result recorded. Where this sheet and observed behaviour disagree, the observation wins and the sheet is corrected.

Fixture IDs (`FX-…`) refer to `spikes/P00-T01-fixture-specifications.md`.

## 1. Common protocol (applies to every spike)

### 1.1 Preconditions
- Owner approval for the specific spike task (T04–T09) and, where needed, the install/crate/network approval it depends on (`plan.md` §5).
- Tool installed per the approved installation request (P00-T02); version and binary SHA-256 recorded before any run.
- Fixtures generated from the fixture specification into a fresh scratch directory outside the repository; generated fake secrets are never committed (fixture specification §1, rules 2–5).

### 1.2 Isolation for every run
- Fresh working directory per run; the fixture tree is copied, never edited in place.
- `HOME` and `XDG_CONFIG_HOME` pointed at empty scratch directories, so user-level tool configuration cannot influence results. Record any config the tool still finds.
- Child environment built from an allowlist (e.g. `PATH`, `LANG`, scratch `HOME`/`TMPDIR`); model keys, cloud credentials and tokens are absent. Record the exact variable names passed (not values).
- Network: default runs are performed with networking unavailable to the child. The isolation mechanism (e.g. a network namespace) is itself unverified on this host and must be confirmed in the first spike; if no verifiable mechanism exists, record that and run offline-sensitive cases only with the owner's knowledge. Separately, observe whether the tool *attempts* network access (error messages, timeouts) — an attempt is a finding even if blocked.
- Bounded runtime (wall-clock timeout) and bounded captured output per stream; truncation recorded.
- Never run fixture repositories' own scripts, hooks or workflow `run:` steps. Only the tool under test is executed.

### 1.3 Record per run (into `evidence/P00/<task>/runs.md` or a JSON equivalent)
Run ID; spike task; tool name, version string, binary path and SHA-256; exact argv (as an array); cwd; environment variable names; network state; fixture IDs; exit code; signal (if any); wall time; stdout/stderr byte counts and truncation; output files created (paths, sizes); whether any fake-secret canary appears in **any** stream or file (match by canary ID, never by printing the value); observed network attempts; notes. Raw outputs that contain canaries are not committed; only sanitised excerpts and canary-presence booleans are kept.

### 1.4 Result format — contract sheet
Each tool spike ends with a contract sheet (one per adapter) containing: adapter ID; tool version and checksum; invocation template (argv array with placeholders, no shell); app-owned config content and digest; repository-config/ignore/inline-suppression policy and how it is neutralised (or why the capability is disabled); scope strategy (`explicit_files` / `staged_selection` / disabled); output format and parser ID; secret-exposure points and redaction approach; exit-code mapping into `ExitMeaning`; network policy and enforcement method; SCAN-08 behaviour; limitations; open questions. A capability that cannot be controlled is recorded as **disabled with reason**, never assumed safe.

### 1.5 Outcome classification
For each hypothesis: `confirmed`, `refuted`, `partially confirmed (conditions)`, or `not testable (reason)`. No hypothesis is marked confirmed from documentation alone; documentation and observation are recorded separately.

## 2. Cross-tool hypotheses (C1/C11 restated as testable claims)

| ID | Hypothesis to test per tool | Fixtures |
|---|---|---|
| H-CFG | The tool auto-loads one or more repository-controlled config files from the scan root or parents | FX-INJ-* |
| H-OVR | An explicit app-owned config argument fully replaces (not merges with) repository config | FX-INJ-* |
| H-INL | The tool honours inline suppression directives inside scanned files (e.g. comment annotations) — a repository-controlled suppression channel even when config files are neutralised | FX-INJ-INLINE-* |
| H-EXE | Repository content can cause the tool to load and execute code (plugins, custom checks, external policy dirs) | FX-INJ-EXEC-* |
| H-SCOPE | Given explicit file arguments, the tool reads only those files; otherwise it walks directories | FX-SCOPE-* |
| H-ARG | Hostile file names are passed safely when given as argv elements after an end-of-options marker where supported | FX-NAME-* |
| H-SEC | Raw secret material can appear in stdout, stderr or report files; a redaction option removes it from all of them | FX-SEC-* |
| H-EXIT | Exit codes distinguish clean / findings / tool error / usage error | all |
| H-NET | Default operation attempts network access; it can be disabled verifiably | FX-NET-* |
| H-MAL | On malformed YAML the tool still performs its non-parser checks (SCAN-08) | FX-YAML-MALFORMED-SECRET |
| H-OUT | Output is deterministic and parseable in the chosen machine format, including for hostile names/ANSI | FX-NAME-ANSI, FX-OUT-* |

## 3. Per-tool protocols (P00-T09)

### 3.1 yamllint (adapter `yamllint`, category `syntax`/`style`)
Candidate behaviours to verify (unconfirmed):
- Config discovery: repository files such as `.yamllint`, `.yamllint.yaml`, `.yamllint.yml`; user-level config under the XDG config dir; an environment variable naming a config file. Test each with FX-INJ-YAMLLINT.
- Override: explicit config argument (`-c` / config-data option). Verify whether discovery still happens when it is passed (H-OVR).
- Inline directives: comment directives that disable rules for a line/file (FX-INJ-INLINE-YAMLLINT). Decide policy: if they cannot be ignored, record that style/syntax results are repository-suppressible and surface this as a limitation.
- Ignore settings inside config (`ignore`/`yaml-files` patterns) — confirm an app config with explicit file args bypasses them.
- Output: machine-readable "parsable" format; confirm line/column, level, rule ID; confirm how syntax errors are distinguished from style rules (category split required by spec).
- Exit codes: candidate 0 = no problems, 1 = errors, 2 = warnings under strict mode. Verify with FX-YAML-VALID, FX-YAML-STYLE, FX-YAML-MALFORMED.
- Python environment: record interpreter version and isolated environment path; check behaviour on the host Python version only if that is what the approved install uses (B-09).
- Secrets: confirm whether diagnostics echo line content (FX-YAML-MALFORMED-SECRET).
Steps: (1) version/checksum; (2) baseline on FX-YAML-*; (3) each FX-INJ-YAMLLINT variant with and without app config; (4) FX-INJ-INLINE-YAMLLINT; (5) FX-NAME-* with explicit args; (6) FX-SCOPE-* (does a directory arg pick up non-YAML or symlinked files?); (7) FX-YAML-LIMITS for runtime/memory; (8) contract sheet.

### 3.2 Gitleaks (adapter `gitleaks`, category `secrets`)
Host has gitleaks 8.30.1 already (not yet pinned/approved as the pilot version).
Candidate behaviours to verify:
- Mode: directory/no-Git mode for current checkout scope versus Git-history mode (must not be used: SCAN-09 deferred). Confirm the subcommand/flag names for the pinned version.
- Config discovery: repository `.gitleaks.toml` at scan root; `.gitleaksignore` fingerprint file; environment variable config path. Test FX-INJ-GITLEAKS-*.
- Override: explicit config path; verify whether repository `.gitleaks.toml` or `.gitleaksignore` is still consulted (H-OVR); verify whether the default ruleset is fully replaced or extended by an app config.
- Inline allow annotations in scanned files (FX-INJ-INLINE-GITLEAKS).
- Scope (C11): whether explicit single files can be targeted; if only directories, design and test the staged-copy strategy (copy only selected files, preserving relative paths, then map results back) and confirm line numbers survive.
- Redaction: the redact option — verify that **report file**, stdout and stderr are all free of canaries (H-SEC); check partial-redaction modes if present; confirm the `Match`/`Secret` style fields are blanked in JSON.
- Output: JSON report to an app-owned path; confirm schema fields for rule ID, file, line, fingerprint.
- Exit codes: candidate configurable "leaks found" code distinct from error; verify with FX-SEC-*, FX-YAML-VALID and a bad-flag run.
- Allowlists: confirm whether vendor-documented example keys are allowlisted by default (affects fixture design; fixture specification §1 rule 4).
- SCAN-08: FX-YAML-MALFORMED-SECRET must still yield a secret finding.
Steps: version/checksum; baseline FX-SEC-*; injection variants; inline annotations; scope/staging; redaction across all channels; exit map; contract sheet.

### 3.3 Checkov (adapter `checkov`, category `misconfiguration`)
Candidate behaviours to verify:
- Input: single-file versus directory scanning; framework selection flag; confirm GitHub Actions and Kubernetes frameworks for the pinned version (supported-framework list is frozen from observation, D-04).
- Config discovery: repository `.checkov.yaml`/`.checkov.yml`; confirm explicit config file argument and whether discovery still occurs.
- Code-execution channel (H-EXE, highest priority): external custom-check directories / modules, including any option that a repository config file can set. FX-INJ-EXEC-CHECKOV contains a custom check that would write a marker file if executed; a marker appearing is a confirmed defect of that configuration.
- Inline suppressions: in-file skip annotations (FX-INJ-INLINE-CHECKOV).
- Platform/API: confirm no vendor-platform calls without an API key; confirm behaviour of any download/update of external policies or guidelines; run with network unavailable and record attempts (H-NET, DIST-01).
- Output: JSON; confirm passed/failed/skipped structures, check IDs, resource and line ranges; confirm how parse errors are reported.
- Exit codes: candidate nonzero on failed checks with a soft-fail option; verify findings vs crash vs usage error.
- Python environment and version compatibility (B-09).
Steps: version/checksum/Python version; baseline FX-K8S-*, FX-GHA-*; config/exec injection; inline skips; network-off run; exit map; contract sheet.

### 3.4 actionlint (adapter `actionlint`, category `workflow_validation`)
Candidate behaviours to verify:
- Config discovery: repository `.github/actionlint.yaml`/`.yml`; explicit config option; whether discovery depends on cwd/project root detection.
- Helpers: integration with ShellCheck/Pyflakes when on `PATH`; verify they can be disabled explicitly and that, when enabled, they only analyse text (do not execute `run:` scripts). ShellCheck is not installed on the host; record behaviour when absent.
- Output: JSON via a format template or other structured output; confirm fields and escaping for hostile names.
- Explicit file args vs repository auto-discovery.
- Exit codes: candidate 0 none, 1 problems, 2 invalid options, 3 internal failure.
- Network: confirm no network access (e.g. for action metadata) in the pinned version.
- FX-GHA-INJECTION must be reported or at least not executed; FX-GHA-REF records the unpinned `uses:` reference (SCAN-05 gap behaviour is a product concern, not actionlint's).
Steps: version/checksum; baseline FX-GHA-*; config injection; helper on/off; hostile names; exit map; contract sheet.

### 3.5 kubeconform (adapter `kubeconform`, category `schema`)
Candidate behaviours to verify:
- Schema location: default remote location (must not be used in pilot); explicit local schema-location template pointing at the approved cache layout (Tech §25). Spike uses a tiny hand-made local schema set (FX-K8S-SCHEMA-STUB), **not** a downloaded cache, until D-11 download approval exists.
- Missing schema behaviour: option that ignores missing schemas (must not convert gaps to passes); default behaviour on unknown kind/CRD (FX-K8S-CRD-MISSING) — product maps to `unsupported`.
- Strict mode for unknown fields; Kubernetes version selection option.
- Cache option: confirm whether it writes downloaded schemas and where.
- Output: JSON; per-resource status values (valid/invalid/error/skipped) and how multi-document files are reported (FX-K8S-MULTIDOC).
- Exit codes for valid/invalid/error.
- Network-off run must not hang; record timeouts.
Steps: version/checksum; local stub schemas; baseline FX-K8S-*; missing CRD; strict; network-off; exit map; contract sheet.

## 4. Other spike protocol sheets

### 4.1 P00-T04 Git safety (local only)
Goal: verify which configuration controls prevent hook execution, filter/LFS process execution and submodule recursion during clone and fast-forward update, while preserving the user's credential helper/SSH agent.
Steps: (1) record Git version; (2) build FX-GIT-* bare repositories locally; (3) clone/update using local paths with candidate per-invocation config overrides (e.g. empty hooks path, submodule recursion off, protocol allowlist, LFS/filter neutralisation) — record argv; (4) observe marker files (FX-GIT-HOOK, FX-GIT-FILTER) — a marker means execution occurred; (5) confirm `.gitmodules` entries are not fetched (FX-GIT-SUBMODULE); (6) confirm unsafe-state detection inputs (dirty, diverged, in-progress merge: FX-GIT-STATE-*); (7) check that user/global config (scratch `HOME`) and system config influence is understood (template directories, `include` directives); (8) document the credential-helper preservation question for the remote part, which needs separate network approval. Protocol note: the product rejects `file://`; local-path clones here are spike scaffolding only.

### 4.2 P00-T05 Process-tree supervision (Linux)
Goal: verify timeout, cancel and process-tree kill per CA-01 using only spike code we write.
Fake tools (FX-PROC-*): sleeps forever; spawns a grandchild that ignores SIGTERM; double-forks a daemon-like child; floods stdout; floods stderr; writes a canary to stderr; exits with each code 0–3 and by signal; emits ANSI; closes stdout early but keeps running.
Steps: run each under the candidate supervisor design (new process group/session, graceful signal, grace period, forced kill of group); after termination check that no fixture process remains (by group and by a unique marker in argv); measure kill latency; confirm output bounds stop reading and terminate the tree; record `TerminationRecord` fields that can be honestly populated, especially whether `process_tree_exited` can be determined.

### 4.3 P00-T06 YAML parser and JSON-schema validator (crates; needs approval)
Parser criteria: multi-document with one-based index; duplicate-key detection as an error; bounded alias expansion (FX-YAML-ALIAS-BOMB), depth limits (FX-YAML-DEEP); line/column mapping for nodes; behaviour on custom/unsafe tags (FX-YAML-TAGS) — never constructs objects; BOM, tabs, invalid UTF-8 (FX-YAML-ENCODING); merge keys; no panics on any fixture (fuzz-style corpus = all FX-YAML-*).
Validator criteria: Draft 2020-12 including `$ref`/`$defs`, `format` checking enabled explicitly, no remote `$ref` resolution unless a resolver is supplied, error paths mapped back to YAML locations; validated against the CA-01 schemas and the Data Schema examples.

### 4.4 P00-T07 Linux keyring
Check availability of a Secret Service provider in desktop and headless sessions; behaviour when locked/absent; that failure surfaces as "OS store unavailable" and never falls back to plaintext. Use a dummy credential value; never a real key.

### 4.5 P00-T08 PDF renderer comparison
Fixtures FX-PDF-*. For each candidate: remote resource fetch blocked (FX-PDF-REMOTE, observe network attempts); local file reads blocked (FX-PDF-LOCALFILE); active content not executed (FX-PDF-ACTIVE); Unicode/RTL/CJK and wide tables render (FX-PDF-UNICODE, FX-PDF-TABLE); large input bounded (FX-PDF-LARGE); nonempty valid output detection; runtime dependencies, licence, Linux packaging footprint. Installation of absent candidates needs separate approval. WeasyPrint being installed is not a criterion (D-07).

## 5. Exit criteria for P00-T09 (per tool)
A contract sheet exists; every H-* row has an outcome; every confirmed repository-controlled suppression or execution channel has a neutralisation verified by a negative test or the capability is marked disabled; exit mapping covers clean, findings, error, usage error; secret-exposure points are known with a verified redaction path; limitations listed. The owner reviews the sheets at P00-T11 before versions are frozen.
