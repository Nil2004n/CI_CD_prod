# P00-T09 — per-tool scanner spikes (combined request, all five tools)

Status: prepared 2026-10-24 and **approved by the owner in the same instruction** ("prepare the T09 request, combined for all five tools … complete P00 T09 … perform the tests").

Scope: run the five **already-installed, hash-verified** tools (P00-T02 Request A + B1) locally, on **synthetic fixtures only**, to produce one contract sheet per adapter (P00-T01 protocol §1.4, §3.1–3.5, exit criteria §5).

**Not in scope:**
- installs, upgrades or downloads; network access;
- real repositories or credentials;
- remote schema fetching (kubeconform uses hand-made local stub schemas only, FX-K8S-SCHEMA-STUB; the D-11 cache needs a separate approval, via P00-T10);
- any change to the installed tools.

## 1. Tools (all re-hashed at preflight against `evidence/P00/P00-T02/install-record.md`)
| Adapter | Binary | Version (install record) |
|---|---|---|
| yamllint | `~/.local/share/pwt-dev-tools/yamllint/1.38.0/venv/bin/yamllint` (standalone CPython 3.12.15, hash-checked) | 1.38.0 |
| gitleaks | `~/.local/bin/gitleaks` (hash-checked; user-writable path, caveat in the install record) | 8.30.1 |
| checkov | `~/.local/share/pwt-dev-tools/checkov/3.3.30/venv/bin/checkov` (CPython 3.12.15) | 3.3.30 |
| actionlint | `~/.local/share/pwt-dev-tools/actionlint/1.7.12/actionlint` | 1.7.12 |
| kubeconform | `~/.local/share/pwt-dev-tools/kubeconform/0.8.0/kubeconform` | 0.8.0 |

Helper tools and operations (enumerated, per the T07 rule):
- `/usr/bin/python3 -I` (system 3.14, harness only; stdlib, no packages);
- `unshare`, `timeout`, `bash` (run scripts);
- the five tools above, including their own `--help`/`--version` style invocations.

No other executable is invoked. ShellCheck and Pyflakes are **absent** (verified at preflight), so actionlint's helper behaviour is observed only as absent.

## 2. Isolation (every run)
- `outer.sh` → `timeout -s KILL 1800 unshare -r -n --pid --fork --mount-proc --kill-child env -i PATH LANG` → `setup-and-run.sh` (PID 1, root prefix, `ulimit -v 4 GiB`) → `python3 -I run_t09.py` as PID 1.
- **Amendment (Opus, 2026-10-24, before any formal run):** the address-space limit was raised from 4 GiB to **16 GiB**. gitleaks 8.30.1 cannot start under 4 GiB: go-re2 on wazero reserves 4 GiB of virtual address space, giving `panic: allocator_unix: failed to reserve memory`, exit 2. It starts at 8 GiB (version check, same namespace setup). This is a virtual-address bound, not a memory-use measurement.
- **Preflight:** loopback-only network namespace; inherited environment exactly `{LANG, PATH}`; spike root 0700; tool/interpreter hashes; helpers absent.
- **Per invocation** (`common.Ctx.run`): a fresh case directory; allowlisted environment (`PATH=/usr/bin:/bin`, `LANG`, scratch `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `TMPDIR`); stdin closed; wall-clock timeout (default 120 s); bounded capture.
- **Recorded per run:** argv (sanitised), environment *names*, exit code or signal, wall time, byte counts; canary presence per stream and per named output file (by value and by `PWTFAKE<ID>` marker); new execution markers; files created; network-attempt signals (stderr/stdout heuristics, because the netns blocks real connections); sanitised excerpts.
- **Raw outputs never leave the spike root.** `outer.sh` copies only `results.json`, which a final guard rejects if any canary value or marker is present.

## 3. Fixtures (`spikes/t09-tools/fixtures.py`, P00-T01 spec)
- **Fake secrets:** generated at run time and never committed. C01–C09 are prefixed-token, AWS-like pair, PEM, URL-credential, generic, block-scalar and base64-in-Secret shapes; C11/C12 are the scope siblings. No vendor example keys.
- **Common trees:** FX-YAML-*, FX-SEC-*, FX-NAME-* (dash, ANSI, newline, glob, bidi, long), FX-SCOPE-*, FX-GHA-* (valid, invalid, injection with execution marker, refs), FX-K8S-* (valid, insecure, invalid, multidoc, CRD, template).
- **Tool-specific hostile files** (FX-INJ-*, inline directives, FX-INJ-EXEC-CHECKOV custom check, stub schemas) are built in each `cases_<tool>.py`.

## 4. Hypotheses per tool
Each of H-CFG, H-OVR, H-INL, H-EXE, H-SCOPE, H-ARG, H-SEC, H-EXIT, H-NET, H-MAL and H-OUT (P00-T01 §2), where applicable to the tool, gets an outcome: confirmed / refuted / partially confirmed (conditions) / not testable (reason) / observed. Each outcome is backed by named runs, and no outcome rests on documentation alone. Every repository-controlled suppression or execution channel found needs either a verified neutralisation (a negative test) or the capability marked **disabled with reason**. Tool-specific protocol: P00-T01 §3.1–3.5.

## 5. Work split
- **Opus (shared contracts):** `common.py`, `fixtures.py`, `run_t09.py`, `run/*.sh`, this request, all formal runs, the contract sheets, the record, status.
- **`sonnet-implementer` workers, two at a time, disjoint files:**
  - W1: `cases_yamllint.py`, `cases_gitleaks.py`, `cases_kubeconform.py`;
  - W2: `cases_checkov.py`, `cases_actionlint.py`.
- Workers may run their own tools' help/version and **development** runs through `run/outer.sh` into the session scratchpad. Those runs are not evidence.

## 6. Pass and stop rules
- **Stop and report** (no repair) if:
  - an execution marker appears in an app-configured run;
  - a canary reaches persisted results;
  - a network attempt cannot be bounded;
  - preflight fails.
- An uncontrollable channel is **not** a spike failure. It is a finding, and the capability is disabled with reason.
- Evidence: `evidence/P00/P00-T09/` (`preflight/`, `run1/results.json`, `contract-<adapter>.md`, `record.md`).

## 7. Amendments — owner-approved batch (2026-10-24, after the 429-interruption recovery)

Approved by the owner after the read-only recovery assessment (which was accepted as a state reconstruction only, not T09 acceptance). Implemented by Opus (shared harness and, as the disclosed fallback for the interrupted workers, the case-module corrections); no worker was restarted.

- **RLIMIT_AS:** 16 GiB for **gitleaks children only**; every other tool's children 4 GiB (unchanged); the orchestrator soft 4 GiB / hard 16 GiB. Set per child in `common.Ctx.run` (`preexec_fn` → `setrlimit`). It bounds virtual address space, not physical memory; no `GOMEMLIMIT`, no cgroup or host policy change.
- **Bytecode:** orchestrator runs `python3 -I -B -X pycache_prefix=<spike-root>/pycache`; preflight fails if `spikes/t09-tools/__pycache__` exists. (The attributed `__pycache__` with three `.pyc` files was removed after verification.)
- **S2 process handling:** each invocation in its own session/process group; timeout → SIGTERM to the group → SIGKILL after 2 s; direct child exited but pipes held → group SIGKILL after 2 s; draining then bounded to 2 s and our pipe ends closed. Descendants that leave the group are not contained (self-test shows this explicitly); only the PID-namespace teardown ends them.
- **S3/S4 output handling:** full-stream canary scanning (value, base64 form, embedded marker; C03's marker is `PWTFAKECD`); redaction before excerpt truncation; trailing partial canary removed at excerpt and retained-capture boundaries; marker-led fragments redacted anywhere; control/bidi escaping in excerpts, notes and hypothesis text; Markdown/HTML escaping in contract sheets (`make_sheets.py`); per-tool and final fail-closed guards (also reject any `PWTFAKE` substring). Analysis retention per stream raised from 1 MiB to 16 MiB in the development correction cycle (checkov JSON exceeded 1 MiB).
- **S1:** write inventory per tool (spike-root changes outside the tool's work directory, a pre-created synthetic sentinel's hash, new top-level names in `/tmp`, `~`, `~/.cache`, `~/.config`). Tracking only, not confinement.
- **Network:** preflight proves separate net/user/PID namespaces (inode comparison against the host values recorded by `outer.sh`), only `lo`, `lo` down at start, no IPv4/IPv6 routes, no proxy variables, no host DNS IPC sockets (nscd, systemd-resolved varlink, avahi), known NSS `hosts` sources. Outbound-attempt fixtures (kubeconform default schema location, remote `$ref`, checkov default downloads, actionlint/yamllint remote references) run only under that verified isolation.
- **Loopback server (kubeconform cache test, and the self-test):** in-process stdlib `ThreadingHTTPServer` on `127.0.0.1:<ephemeral>` serving only the stub-schema directory; `lo` brought up by an in-namespace `ioctl(SIOCSIFFLAGS)` (no `ip` binary) only for the server's lifetime and brought down in a `finally` path; shutdown/close/thread-join in `finally`; request paths logged; listeners and thread state verified afterwards.
- **Helper operations added to §1:** `/usr/bin/python3 -I -B -c <probe>` as the harness self-test child (spike-owned probes only); the in-process loopback server; in-namespace loopback ioctl; `make_sheets.py` (outside the namespace, sanitised results only).
- **Self-test (`cases_selftest.py`)** runs after preflight in every run (also `preflight-only`); a failing self-test stops the run before any tool.
