# P00-T09-D02 — H-3 redaction fix, D01 attribution, focused revalidation: record

Written by Opus, 2026-10-24. Owner-approved:
- the H-3 harness fix and focused revalidation;
- D01 marker-attribution resolution (bookkeeping correction and one Checkov re-verification **only if** attribution was ambiguous);
- moving the raw T09 run1 result out of the checkout;
- one scoped local checkpoint.

Owner status: Checkov revision 2 provisionally accepted as the candidate for the tested Kubernetes selected-file workflow; T09 final acceptance pending. T09 run1, D01 formal evidence and the original generated contract sheets are unchanged. R1a/R1b remain failed, retired application-invocation candidates.

## 1. D01 marker attribution — resolved from existing evidence (no new Checkov run)
- **Per-invocation evidence:** `Ctx.run` records `files_created` as a **path-level before/after diff of that invocation's own case directory**. In D01 run1 (`results.json` sha256 `70f35be5…`):

  | Invocations | Marker created |
  |---|---|
  | 3–6 (`CTRL-R1-FLAGS-CWD-REPO`, `CTRL-R1-APPCFG-CWD-REPO`, `CTRL-HOME-CONFIG`, `CTRL-UNFILTERED-COPY-D`) | each created `PWT_EXECUTED_FX-INJ-EXEC-CHECKOV-R2-ROOT` in its own directory; each of these directories was used by that single invocation |
  | the other 12 (`R2-REF`, `R2-REF-MULTI`, `R2-HOSTILE-SINGLE`/`-MULTI`/`-SHOW-CONFIG` (one shared directory), `R2-STAGING-RUN`, `R2-MALFORMED`, `R2-MISSING-DIRECT`, `R2-NO-RESOURCES`, `R2-INLINE`, both `R2-CONTEXT` comparisons) | none |
- **End-state listing:** exactly 4 marker paths, all in the four control directories.
- **Overwrite argument:** a later execution could overwrite without creating a file only if its module's marker path already existed. Every hostile module writes to an absolute path inside its own case directory (module code in the hashed `cases_checkov_r2.py`), and R2 invocations' cwd, HOME and inputs stay in their own directories. No R2 directory ever held a marker. Marker contents are identical (`executed`) and carry no invocation identity, so attribution rests on paths, not contents.
- **Conclusion:** attribution is unambiguous. The name-only `new_execution_markers` field is unreliable when names repeat and is not used. No bookkeeping change and no new Checkov run were needed.

## 2. H-3 fix (shared harness)
**Fixture (`fixtures.py`):**
- The synthetic PEM body (C05 canary line plus six random lines) is generated once per canary set (`_pem_body`). Previously each call drew a new body.
- `synthetic_components(c)` exposes every secret-bearing component: `PEM-L1`…`PEM-L7` (each 64-character body line; L1 is the C05 canary), `PEM-BODY` and `PEM-BLOCK`.
- Canaries are unchanged.

**Redactor (`common.py`):**
- **Full-value redaction:** components are registered as full-value patterns (`<synthetic:ID>`).
- **Fragments:** a fragment of ≥15 characters of any single-line component inside a base64-alphabet run is always detected (aligned 12-character windows at stride 4). The whole run is replaced by `<synthetic-fragment:ID>`.
- **Streams and guards:** full-stream detection and the per-tool and final fail-closed guards use the same patterns and windows.
- **Truncation boundaries:** a trailing component prefix of at least 4 characters is removed at the excerpt and retained-capture boundaries. Canary and marker prefixes keep a 1-character minimum.
- **Fail-safe:** an excerpt still containing registered material after redaction is withheld (`<excerpt withheld: …>`).
- **Persistence:** registered component **IDs only** are recorded (`registered_synthetic_components`); values are never persisted.
- **Paths covered:** excerpts, retained buffers given to case code, notes (`ctx.clean`), hypothesis details and exception text (`ctx.sanitise`) all pass through the same `Redactor.text`.

**Exact coverage and remaining limits:**
- Coverage is the registered run-time synthetic fixtures only (11 canaries with base64 forms and markers, plus the 9 PEM components).
- **Not detected:** interior fragments of a component shorter than 15 characters; trailing boundary prefixes shorter than 4 characters; transformed encodings of components (e.g. base64 of a PEM line) other than verbatim and JSON/YAML-escaped text; arbitrary real secrets.

## 3. Focused revalidation (`evidence/debug/P00-T09-D02/`)
**Harness hashes:** `harness-sha256.txt` (HEAD `0cd446f` plus the working-tree changes listed in it).

**Fresh preflight + self-test (`preflight/`):** preflight 25/25; self-test 29/29. The H-3 checks are:
- `h3_components_registered`;
- multiline YAML block;
- JSON-escaped block;
- single line mid-text;
- 20-character interior fragment;
- line across the excerpt boundary;
- line across the 16 MiB capture boundary;
- notes;
- exception text;
- guard detection of a fragment in JSON.

The existing S2/S3/S4/loopback checks also pass. The check details record booleans and lengths only.

**Fresh synthetic gitleaks run (`gitleaks/`):**
- outer exit 0; preflight 25/25; self-test 29/29; 60 gitleaks invocations; 0 timeouts; results not suppressed.
- Compared with T09 run1: hypothesis outcomes, case list, exit codes and exit map identical; `gitleaks:allow` / `--ignore-gitleaks-allow` behaviour identical.
- In `gl-sec-raw-verbose` (the case that leaked under H-3), all seven PEM lines and the body were detected in the stream and replaced by 12 synthetic labels in the persisted excerpt; nothing withheld.
- No `PWTFAKE` and no high-entropy 64-character run anywhere in the persisted output.
- The fixture is fresh: different random values from run1's fixture, which is not reproducible.

**Reprocessing of existing results (`reprocess-summary.json`, counts only; originals unmodified):**
- The corrected redactor changes 0 strings in each of: raw T09 run1 (private copy), the run1 derivative, T09 preflight-final, D01 run1, D01 preflight, D02 gitleaks, D02 preflight.
- `PWTFAKE` 0 everywhere; control/bidi characters after Markdown escaping 0 everywhere.
- High-entropy 64-character runs: **12 in the raw T09 run1** and 0 elsewhere.
- **Limit:** original run-time fixture values were never persisted, so the corrected path cannot retro-detect H-3 material in old results; this is idempotence and residual-indicator checking only. The committed run1 derivative remains the sanitised form (entropy-rule transformation, §5).

**Development regression (scratchpad, not evidence):** all five original suites on the corrected path matched T09 run1 in hypothesis outcomes, case lists and exit codes; 0 excerpts withheld; 0 residual indicators.

**Contract effect:** no tool's contract outcome changed. The T09 run1 contract sheets and the Checkov revision-2 contract stand.

## 4. Development notes (not evidence)
- The first self-test after the fix failed 3/29. With a 1-character minimum, a trailing padding character equal to a random body line's first character counted as a component prefix.
- The minimum for components was set to 4. Two further independent fixture draws passed 29/29.

## 5. Raw T09 run1 result
- **Moved:** `evidence/P00/P00-T09/run1/results.json` moved (not copied) to `~/.local/share/pwt-dev-tools/_private-evidence/P00-T09/run1/results.json`.
- **Integrity:** sha256 `e84a6c53f1e5652404a7c3fbc95f3f94bfdca30c5e424377e6b5bd5ccdb399f5` before and after, size 1278898 bytes. File mode 0600; directories 0700; owner `kali`; outside the Git checkout; contents never printed.
- **Retained status:** preserved original formal evidence containing synthetic PEM body lines (H-3).
- **In the repository:** the derivative `run1/results.pem-redacted.json` (sha256 `68e3f77b…`) and its `.meta` (source hash, rule, replacement count 12) remain committed.
- **Transformation:** every 64-character base64-alphabet run with entropy > 4.5 bits/char inside `*_excerpt` fields was replaced by `<synthetic-pem-body-line-redacted>`. Nothing else changed.
- **Assessment:** H-3 does not show a real credential compromise (the material was a synthetic, randomly generated fixture). It is a genuine guard-coverage defect: secret-shaped material reached persisted evidence undetected.

## 6. Cleanup
- No spike roots, T09 processes, listeners or `__pycache__` remain.
- Development outputs are only in the session scratchpad.
