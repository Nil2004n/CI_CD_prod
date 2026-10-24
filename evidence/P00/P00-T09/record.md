# P00-T09 — per-tool scanner spikes: record (formal run1)

Written by Opus, 2026-10-24. Spike evidence for synthetic fixtures, this host and the pinned tool versions only. Not production adapter acceptance and not T09 acceptance (owner decision pending).

## 1. Basis
- **T09 approval:** owner, 2026-10-24 07:12Z (transcript `d0711084`): "prepare the T09 request, combined for all five tools. spawn the required sonnet agents and complete p00 t09, once merged into the main session perform the tests and notify me." Request `spikes/P00-T09-request.md` (§7 lists the later amendments).
- **Interruption:** workers W1/W2 stopped by a session-wide HTTP 429 (07:31Z). The only worker report is W1's hand-back (07:30Z). Its statements are not used as evidence. Worker development outputs were lost in a host reboot. Recovery assessment accepted by owner as a state reconstruction only.
- **Batch approval (owner, after recovery):**
  - continuity corrections;
  - 16 GiB RLIMIT_AS for gitleaks only;
  - `__pycache__` removal and bytecode prevention;
  - S2 timeout/descendant handling;
  - S3/S4 output handling;
  - S1 write inventory (tracking only);
  - kubeconform loopback server and outbound-attempt probes under verified isolation;
  - one development correction cycle;
  - one formal run.
  
  Not authorised: T10 lookups, tool upgrades, dependency changes, commit, push, merge.
- **Implementation:**
  - Opus wrote the shared harness changes.
  - Opus also made the case-module corrections, as a disclosed fallback; no worker was restarted.
  - The original case modules are from W1 (yamllint, gitleaks, kubeconform) and W2 (checkov, actionlint), configured model `claude-sonnet-5-5`, unverified.

## 2. Revision and environment
- **Repository state:** HEAD `ac4cf67`, branch `phase/P00-decisions-spikes`. Harness files are untracked; exact sha256 values are in `run1/harness-sha256.txt`.
- **Host:** Linux 6.19.14, 9.3 GB RAM, 0.95 GB swap, `vm.overcommit_memory=0`.
- **cgroup:** `user.slice/.../session-2.scope` with `memory.max`, `memory.high`, `memory.swap.max` and `pids.max` all `max` (no cgroup memory or pids limit).
- **Isolation per run:** `outer.sh` → `timeout -s KILL 1800 unshare -r -n --pid --fork --mount-proc --kill-child env -i PATH LANG` → `setup-and-run.sh` → `python3 -I -B -X pycache_prefix=<root>/pycache run_t09.py`.
- **RLIMIT_AS (verified by the self-test from the child's `/proc/self/limits`):**
  - gitleaks children: 16 GiB (soft = hard);
  - all other children: 4 GiB;
  - orchestrator: soft 4 GiB, hard 16 GiB.
  
  This is a virtual-address-space bound, not a physical-memory cap. No `GOMEMLIMIT` was set; no cgroup or host policy was changed.
- **Observed memory:** `ru_maxrss` of the direct child, max per tool. This includes descendants only if the child reaped them.

  | Tool | Max RSS |
  |---|---|
  | yamllint | 22.2 MiB |
  | gitleaks | 69.9 MiB |
  | checkov | 181.5 MiB |
  | actionlint | 77.8 MiB |
  | kubeconform | 77.9 MiB |
  | self-test | 156.4 MiB (its 16 MiB-output probe) |
- **Other limits (unchanged):** per-invocation wall timeout (default 120 s, per case lower), whole-run 1800 s watchdog, 16 MiB retained per stream for analysis, 4096-char persisted excerpts.

## 3. Runs (formal evidence only)
| Item | Command / location | Result |
|---|---|---|
| Historical preflight (before modules; 4 GiB) | `preflight/` | 17/17, kept as history only |
| Fresh final preflight + self-test | `outer.sh evidence/P00/P00-T09/preflight-final <5 tools> preflight-only` → `preflight-final/` | exit 0; preflight 37/37; self-test 19/19 |
| **Formal run1** | `outer.sh evidence/P00/P00-T09/run1 yamllint,gitleaks,checkov,actionlint,kubeconform` (cwd repo root) → `run1/` | 2026-10-24T13:03:47Z–13:06:58Z, outer exit 0, spike root removed; preflight 37/37; self-test 19/19; 352 invocations (16 self-test + yamllint 89, gitleaks 60, checkov 51, actionlint 71, kubeconform 65); all statuses `completed`; 0 timeouts; 0 spawn errors; results not suppressed |
| `results.json` sha256 | `run1/results.json` | `e84a6c53f1e5652404a7c3fbc95f3f94bfdca30c5e424377e6b5bd5ccdb399f5` |
| Contract sheets | `make_sheets.py run1/results.json run1/outer-summary.txt evidence/P00/P00-T09` | `contract-{yamllint,gitleaks,checkov,actionlint,kubeconform}.md`: generated, no control/bidi characters, no raw `<`, no `PWTFAKE` |

**Development runs** were written to the session scratchpad and are **not evidence**:
- harness: `pf1`, `pf2`, `pf3`
- gitleaks: `gl1`
- checkov: `ck1`
- actionlint, yamllint, kubeconform, gitleaks: `multi1`
- all five: `all2`
- actionlint, kubeconform: `kc3`

## 4. Harness verification in the formal run (preflight 37/37, self-test 19/19)
- **Preflight:**
  - net, user and PID namespace inodes differ from the host values recorded by `outer.sh`;
  - only `lo`, down at start; no IPv4/IPv6 routes; no default route;
  - inherited environment exactly `{LANG, PATH}`; child allowlist has no proxy variables;
  - no nscd, systemd-resolved varlink or avahi sockets; NSS `hosts` = `files myhostname mdns4_minimal [NOTFOUND=return] dns`;
  - loopback toggle works (up → connected; down → connect failed);
  - no TCP listeners;
  - Python isolated mode with `-B`; `pycache_prefix` inside the spike root; no repository `__pycache__`;
  - orchestrator limits soft 4 GiB / hard 16 GiB;
  - gitleaks/actionlint/kubeconform and the CPython 3.12 interpreter hashes match the install record;
  - ShellCheck and Pyflakes absent.
- **S2 process handling:**
  - normal child: no kill;
  - same-group grandchild holding stdout after the child exits: group SIGKILL after the 2 s linger; returned in about 2.5 s; grandchild gone;
  - timeout: SIGTERM to the group, child and grandchild gone, about 2.1 s;
  - SIGTERM-ignoring processes: SIGKILL escalation after 2 s, about 4.1 s;
  - **escaping descendant (`setsid`): the harness returned in a bounded time (about 4.4 s) by closing its pipe ends, but the descendant was not contained by the process group.** It was removed by exact PID after a command-line identity check. Process groups do not contain deliberately escaping descendants; only the PID-namespace teardown does.
- **RLIMIT:** child `/proc/self/limits` showed 4294967296 (default) and 17179869184 (the gitleaks value).
- **S3:**
  - a canary crossing the excerpt boundary at three offsets: redacted before truncation, no trailing fragment;
  - a canary crossing the 16 MiB retained-capture boundary (C02, C03, C04): detected by the full-stream scan, trailing partial removed;
  - C03 full value plus partial: `<canary:C03>` and `AKIA<canary-partial:C03>`;
  - marker-led fragments of C04/C05 redacted;
  - base64 form of C09 detected and redacted.
- **S4:**
  - ESC, BEL, U+202E and CR escaped in excerpts;
  - note keys and values escaped and redacted;
  - Markdown/HTML escaping verified on `| ` `` ` `` `[]()` `<script>` `*_#`, newline and ESC.
- **Loopback server:**
  - `lo` down before, up only during the block, down after;
  - endpoint `127.0.0.1:<ephemeral>`;
  - GET of the served file → 200; traversal `/../outside.txt` → 404;
  - after stop: no listener, thread not alive, connect refused;
  - exactly 2 requests logged.

## 5. Formal stop conditions (request §6)
| Condition | Result |
|---|---|
| Execution marker in an app-configured run | **FIRED — checkov.** Markers appeared in `CK-EXE-CFG-F-RUN` ("app flags (-f, --framework), cwd=repo") and `CK-EXE-APPCFG-RUN` ("app --config-file + -f, cwd=repo"), both labelled app-configured by the case module, and in `CK-EXE-CFG-D-RUN` (unprotected repo config, `-d .`). **No marker** in: the control (module present, nothing references it); app with cwd outside the repo and `-f` (`--skip-download`); app `--config-file` with cwd outside the repo; app with cwd outside the repo and `-d <repo>`; `--external-checks-public-key` with cwd = repo (exit 2, signature gate). |
| Canary reached persisted results | Not fired. No full value, base64 form or `PWTFAKE` marker in `results.json` or in the sheets. **Disclosure:** checkov's own masked display leaves public-prefix fragments of at most 6 characters (`PWTFAK`, `ghp_PW`, `AKIA`, `AKIAP`) with no random characters. The guard does not treat these as leaks. |
| Network attempt could not be bounded | Not fired. 0 timeouts. Checkov tried `api0.prismacloud.io` in 42 runs (no `--skip-download`), each failing at name resolution (max wall 2.4 s). kubeconform's default location tried `raw.githubusercontent.com` (DNS to 192.168.0.1:53, "network is unreachable", 3.06 s). The actionlint "signals" are documentation URLs in help/diagnostic text matched by the heuristic, not connection attempts. |
| Preflight failed | Not fired (37/37). |

**Diagnosis of the checkov stop (no repair made):**
- Checkov discovers `.checkov.yaml` from the working directory (and from the `-d` directory).
- An explicit `--config-file` is merged with it, not substituted for it (H-OVR refuted).
- A repository config's `external-checks-dir` makes checkov import repository Python. The executed code was the spike-authored marker module, inside the user/net/PID namespaces. It wrote only its marker inside its case directory; the write inventory and sentinel are clean.
- This is a repository-controlled code-execution channel, not a harness containment failure.
- In the tested cases it was neutralised only by the working directory (app cwd outside the repository, `-f`/`-d` with paths into it) or by the signature-gate option.
- The two "app-configured, cwd = repo" runs are deliberately adversarial variants, not the intended production invocation (contract note `scope_strategy`). The literal stop rule nevertheless fired and is reported as such.
- Corrective direction, a proposal only: the checkov adapter must run with cwd outside the repository (never cwd = repo), with no `-d` on a raw repository, and preferably on an app-staged copy without `.checkov.yaml`/`.checkov.yml`. Whether `--external-checks-public-key` belongs in the contract is an owner decision.

## 6. Hypothesis outcomes (formal run1)
Classification:
- **refuted** = contradicted hypothesis;
- **not testable** = blocked;
- **observed** = no verdict;
- **partially confirmed** = conditions apply.

Full detail and evidence runs are in each contract sheet.

| H | yamllint | gitleaks | checkov | actionlint | kubeconform |
|---|---|---|---|---|---|
| H-CFG (repo config auto-loaded) | confirmed (cwd + parents only) | confirmed (`.gitleaks.toml`, `.gitleaksignore`, env) | confirmed (cwd and `-d` dir) | confirmed (`.github/actionlint.*`; needs `.git` root; cwd-independent) | refuted (only candidate names probed) |
| H-OVR (explicit config neutralises) | confirmed (`-c`/`-d`) | partially: `-c` neutralises configs; **repo `.gitleaksignore` not neutralised by `--gitleaks-ignore-path`**; single-file target with cwd outside the repo + empty ignore path restored the finding | **refuted** (merged; cwd governs) | confirmed (`-config-file` replaces) | not testable (no config channel) |
| H-INL (inline suppression) | confirmed (no off-switch) | confirmed, neutralised by `--ignore-gitleaks-allow` | confirmed (annotations/comments) | refuted for 7 forms tried; ShellCheck/Pyflakes directives blocked (helpers absent) | not testable |
| H-EXE (repo code execution) | observed (no channel found) | observed (no channel found) | **confirmed** (§5) | refuted (helpers absent; helper-on path unverified) | observed (no channel; schema data) |
| H-SCOPE | confirmed (explicit files) | partially (one path per run) | confirmed (`-f`) | confirmed (explicit files) | confirmed; **`kind` with `../` escapes the schema root**; symlinks followed |
| H-ARG | confirmed (`--`) | confirmed | confirmed (`--file=` form) | confirmed | confirmed |
| H-SEC (secret echo) | confirmed (duplicate-key text) | partially: raw in report and `-v`; **`--redact=100` with `--log-level debug` leaks C09 to stderr**; `--redact=50` leaves canary material | partially (`code_block`) | partially (`snippet`/`message`) | confirmed (`metadata.name`, `kind`) |
| H-EXIT | partially (invalid UTF-8 crash = exit 1, same as findings) | partially (**nonexistent target exits 0**) | partially (missing file and malformed input exit 0; soft-fail repo-controllable) | confirmed | partially (exit 1 for invalid, missing schema and bad flag alike) |
| H-NET | refuted | refuted (dir mode) | confirmed (`api0.prismacloud.io`; `--skip-download` + `CKV_SKIP_PACKAGE_UPDATE_CHECK=True` removed visible attempts) | refuted (indirect evidence) | confirmed (default remote location) |
| H-MAL | confirmed | confirmed (text scan) | observed (**malformed YAML → exit 0, zero results**) | observed | partially (error entry without document index) |
| H-OUT | partially (raw ESC/newline names) | partially (order varies) | confirmed | confirmed | partially (non-deterministic at `-n 4`; `-n 1` stable) |
| Extra | — | — | DIST-01 observed | H-EXE-HELPERS-ABSENT observed | — |

**kubeconform HTTP cache (directly observed through the controlled server):**
1. First run: 1 request (`GET /v1.30.0-standalone/deployment-apps-v1.json 200`), 1 cache file written, `statusValid`.
2. Second run while the server was still serving: 0 requests, `statusValid` (cache hit).
3. Server stopped, warm cache: `statusValid`.
4. Server stopped, no cache: `statusError` (connection refused).

A local `-schema-location` wrote no cache files.

## 7. Development correction cycle (one cycle; before the formal run)
Harness defects found by development runs and fixed:
- `ulimit` order (setting hard before soft gave EINVAL);
- two self-test check-design errors (excerpt test assumed truncation; request log counted error-log lines);
- 1 MiB analysis cap truncated checkov's hostile-name JSON (H-ARG/H-OUT were false negatives) → 16 MiB;
- marker listing was root-wide, so other tools' markers appeared in actionlint/kubeconform notes → scoped to the tool's work directory.

Case-module corrections (Opus):
- **gitleaks:**
  - H-EXIT was "confirmed" despite fail-open exit 0 for a missing target;
  - exposure matrix persisted 12-character value previews → categories only;
  - H-OVR text recommended a flag its own evidence disproved;
  - added explicit-file ignore probes (cwd = repo, and cwd outside with an empty ignore path);
  - H-SEC detail made specific.
- **kubeconform:** cache section moved onto `common.LoopbackSchemaServer`; requests and endpoint recorded.

No assertion was weakened, no regression case removed, and no canary changed (C03's actual marker `PWTFAKECD` was added to detection only).

## 8. Disclosures and limitations
- **Scope of the evidence:** worker statements and all development runs are not evidence. Spike results cover the tested fixtures, this host and these versions only.
- **Tool hashes:** yamllint and checkov are covered only through the trusted CPython 3.12 hash. Venv entry scripts and packages are not hash-checked (existing PyPI provenance exception).
- **Filesystem:** tools run as namespace-root mapped to the user, with no filesystem confinement. Writes are tracked only inside the spike root (sentinel hash; changes outside each tool's work directory) and as new top-level names in `/tmp`, `~`, `~/.cache`, `~/.config`. All were clean in run1, but this is not confinement.
- **Network:**
  - H-NET evidence is stderr/stdout heuristics; there is no syscall tracing.
  - With `lo` down, a loopback connect in preflight failed with a timeout-type error (errno None), not ENETUNREACH; it was recorded as failed.
  - Host D-Bus sockets (system and user bus) exist and are not hidden. They are not a DNS or proxy path for these tools; this was not traced.
- **Probes that run code or make outbound attempts:**
  - Deliberate execution probes: checkov's custom-check module (writes a marker) and actionlint's planted `shellcheck`/`pyflakes` scripts (never executed).
  - Deliberate outbound-attempt probes: kubeconform's default location and remote `$ref`, checkov's default downloads, actionlint's remote action references, yamllint's schema modeline.
  - All ran inside the verified private namespace.
- **Loopback server:** in-process stdlib HTTP server, harness code, owner-approved. `lo` was brought up by in-namespace `ioctl` only for its lifetime.
- **Helpers:** ShellCheck and Pyflakes are absent, so actionlint's helper path and the helpers' inline directives are not exercised. The adapter must disable them explicitly.
- **Not in scope:** visual or terminal rendering of outputs was not checked.

## 9. Cleanup
- Every run's `outer.sh` removed its own spike root (`cleanup: root removed`), and `~/.local/share/pwt-dev-tools/_spikes/` is empty.
- No T09 processes or listeners remain. Namespaces end with their PID 1.
- The escaped self-test descendant was killed by exact PID inside the namespace.
- No `spikes/t09-tools/__pycache__` exists.
- Development outputs remain only in the session scratchpad.

## 10. Post-run finding at checkpoint review (2026-10-24; run1 unchanged)
- **H-3 (harness detection gap, S3):** the run-time PEM fixture body is the C05 canary (first 64-character line) followed by six further random synthetic lines that are not canaries. The guard therefore does not detect them.
- **Where it shows:** they survived in gitleaks `gl-sec-raw-verbose` stdout/stderr excerpts (12 lines: two copies of 6) in `run1/results.json`. No canary value, base64 form or marker was persisted, so the formal stop condition "canary reached persisted results" is not met. The material is nevertheless secret-shaped synthetic data.
- **Contract sheets:** contain no excerpts and no high-entropy runs.
- **Checkpoint handling:**
  - `run1/results.json` is kept unchanged on disk but **not committed**.
  - The commit carries `run1/results.pem-redacted.json`, derived deterministically by `spikes/t09-tools/derive_public_results.py`: the 12 lines were replaced by a label, and the file is identical to the original outside `*_excerpt` fields. Its `.meta` records the source sha256 `e84a6c53…`.
- **Proposed harness fix (not made; outside the Checkov-only debug cycle):** detect every line of synthetic multi-line fixtures (or give each PEM body line its own marker), then re-validate all tools' redaction. Owner decision.
- **Session disclosure:** while locating this, about 30 characters of one synthetic body line were echoed in the session console; nothing was written elsewhere.

## 11. Follow-up
Checkov contract revision 2 and its evidence: `evidence/debug/P00-T09-D01/record.md`, `contract-checkov-rev2.md`. Run1 above is unchanged; R1 stays rejected.
