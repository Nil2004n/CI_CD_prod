# P00-T07 — Linux Secret Service spike: C/D request and isolation design (revision 2)

Status: **C/D proposal, not approved.** Revision 2 (2026-10-24) follows owner decision K-A and stage B (`evidence/P00/P00-T07/record.md`, `advisory-exception.md`). Nothing below has been built or run. Items marked *verify* are checked at run time and abort the run if they fail.

Purpose (KEY-01): decide whether OS credential-store mode is supportable on Linux. If it is not, the product stays **session-only, or AI is disabled**. Plaintext persistence is never a fallback. Persistent OS storage is not accepted for the pilot or product by this spike.

## 1. Decisions and stages so far
- §1 option chosen: **K-A** (spike-only exception for RUSTSEC-2026-0370/-0371/-0372, exact scope in `advisory-exception.md`). K-D has not been chosen; persistent storage stays on the roadmap pending a later product decision.
- Stage A/A2: done (revision 1 of this file).
- Stage B: done. Fetch locked, 149/0 checksums, lock stable, host graph and T06 delta, build exposure, source review (record).

## 2. Source findings that change the design
1. **Address resolution:** `DBUS_SESSION_BUS_ADDRESS`, else `$XDG_RUNTIME_DIR/bus`, else `/run/user/<euid>/bus`. Inside `unshare -r` the euid is 0, so the last fallback is `/run/user/0/bus`. The design therefore masks **all of `/run/user`**, not just `/run/user/1000`.
2. **The address can execute programs** (`unixexec:` and `ibus:` transports). The harness only ever sets `unix:path=<spike-root>/run/bus`, and asserts the variable's exact value in every case process before calling the library. P03 carry-forward: the product must accept only `unix:path=`/`unix:abstract=` session addresses.
3. **The store is initialised once per process and the result is cached** (`LazyLock`). Store configuration is rejected. So **each case runs in its own process**, and no case can reconfigure the store.
4. **No timeouts in the library path:** zbus `method_timeout` defaults to none, and prompt waits block on a signal without a timeout. The application-level timeout is therefore a worker thread plus `recv_timeout`. A worker still blocked after the timeout is a **leaked worker**: that is a finding, never "cancelled".
5. **Activation is not suppressed by the client** (no `NoAutoStart`). Activation control rests entirely on the private bus configuration (§4.4) plus masking the standard service directories.

## 3. Stage C (build)
```
unshare -rn env -i PATH=<toolchain bin>:/usr/bin:/bin HOME=<pwt>/_cargo-home \
  CARGO_HOME=<pwt>/_cargo-home CARGO_TARGET_DIR=<pwt>/_cargo-target RUSTC=<toolchain>/rustc \
  CARGO_TERM_COLOR=never <toolchain>/cargo build --frozen -p t07-keyring
```
(`<pwt>` = `~/.local/share/pwt-dev-tools`.)
- **Build per package (`-p`) only.** This avoids the T06/T07 feature unification recorded in stage B.
- The harness source is written by Opus in `spikes/rust/t07-keyring/src/` (std plus `keyring` only; no new direct dependencies).
- The run scripts go in `spikes/rust/t07-keyring/run/`.
- Lock hash is checked before and after the build. This build runs the 10 build scripts and 9 proc-macros listed in the record, including `proc-macro-crate`'s `cargo locate-project` spawn, inside the network namespace and without a filesystem sandbox.

## 4. Stage D isolation design

### 4.1 Command and containment layers
Env check first (no daemons, no keyring calls):
```
unshare -r -n -m --pid --fork --mount-proc --kill-child \
  env -i PATH=/usr/bin:/bin LANG=C.UTF-8 bash spikes/rust/t07-keyring/run/envcheck.sh <spike-root>
```
Run:
```
timeout -s KILL 240 unshare -r -n -m --pid --fork --mount-proc --kill-child \
  env -i PATH=/usr/bin:/bin LANG=C.UTF-8 \
  bash spikes/rust/t07-keyring/run/setup-and-run.sh <artifact> <spike-root> <evidence-out>
```
- **User + network namespace.** Host abstract-socket buses are unreachable. Filesystem sockets are handled by the mount layer.
- **Mount namespace.** util-linux `unshare -m` defaults to private propagation (*verify* in the env check by reading `/proc/self/mountinfo`). `setup-and-run.sh` mounts empty `tmpfs` (mode 0700) over:
  - `/run/user` (both `/run/user/1000` and the euid-0 fallback `/run/user/0`);
  - `/run/dbus`;
  - `/var/run/dbus` if it is a separate path;
  - `/home/kali/.local/share/keyrings`;
  - `/tmp/.X11-unix`;
  - `/usr/share/dbus-1/services`;
  - `/usr/share/dbus-1/session.d`;
  - `/etc/dbus-1/session.d`.

  If a mount fails, the run aborts as a **harness failure**.
- **PID namespace.** After the mounts, `setup-and-run.sh` `exec`s the harness, which therefore runs as PID 1. As in T05, the harness refuses to run unless it is PID 1. All daemons and case processes die when it exits (`--kill-child`).
- **Outer watchdog.** `timeout -s KILL 240` is containment only.

### 4.2 Private directories
- The spike root is created outside by `mktemp -d ~/.local/share/pwt-dev-tools/_spikes/t07.XXXXXX`, mode 0700.
- Per case: `cases/<id>/{home,run,config,data,cache,keyring-ctl,out}`, each 0700.
- Shared: `bus/` (bus config and socket) and `bus-services/` (always **empty**; K03 starts its daemon explicitly, so nothing needs activation).
- A fresh tree per case means no state carries between cases.

### 4.3 Environment (per case process; built by the harness, not inherited)
- Exactly the following, set per case:
  - `PATH=/usr/bin:/bin`
  - `LANG=C.UTF-8`
  - `HOME=<case>/home`
  - `XDG_RUNTIME_DIR=<case>/run`
  - `XDG_CONFIG_HOME=<case>/config`
  - `XDG_DATA_HOME=<case>/data`
  - `XDG_CACHE_HOME=<case>/cache`
  - `DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent`
  - `DBUS_SESSION_BUS_ADDRESS=unix:path=<root>/bus/socket` (only in cases that need a bus)
- Absent: `DISPLAY`, `WAYLAND_DISPLAY`, `XAUTHORITY`, `XDG_SESSION_*`, `XDG_CURRENT_DESKTOP`, `XDG_DATA_DIRS`, `XDG_CONFIG_DIRS`, `DBUS_STARTER_*`, `GNOME_KEYRING_CONTROL`, `GNOME_KEYRING_PID`, `SSH_AUTH_SOCK`, `GPG_AGENT_INFO`, `DESKTOP_SESSION`, `SESSION_MANAGER`, every key or token variable.
- Each case process re-reads its own environment and refuses to call `keyring` unless it matches this list exactly.

### 4.4 Private bus and activation control
- Start command:
  ```
  dbus-daemon --nofork --nopidfile --nosyslog --config-file=<root>/bus/bus.conf \
    --address=unix:path=<root>/bus/socket --print-address=1
  ```
  The harness reads stdout to confirm the address.
- `bus.conf` is written by the harness. It contains:
  - `<type>session</type>`;
  - `<listen>` as above;
  - an `<auth>EXTERNAL</auth>`;
  - an allow-all policy for the single local user;
  - exactly one `<servicedir><root>/bus-services</servicedir>`, which is empty.

  It contains **no** `<standard_session_servicedirs/>`, **no** `<include>`, and no systemd activation element.
- Binding: `keyring` exposes no connection object, so binding is shown by construction plus assertions, not by a library-level bus ID check. Each case process asserts all of the following before calling `keyring`:
  - `DBUS_SESSION_BUS_ADDRESS` equals exactly `unix:path=<root>/bus/socket`;
  - that socket was created by the daemon the orchestrator started, and the daemon printed the same address;
  - `/run/user/{0,1000}/bus` do not exist inside the namespace.

  Limitation: this does not use an independent peer-credential check (that would need `libc` in this crate or an unstable std API).
- **K03 daemon:**
  ```
  gnome-keyring-daemon --foreground --components=secrets --control-directory=<case>/keyring-ctl --unlock
  ```
  The unlock password goes to **stdin**. The daemon gets the case environment from §4.3 plus `DBUS_SESSION_BUS_ADDRESS`. *verify*: its keyring files appear under `<case>/data/keyrings/`, and nothing appears under the masked real path.
- **K04 daemon:** started without `--unlock`. Optionally a second sub-case starts with `--unlock`, then locks the collection; whether that is possible without new tools is a *verify* item. If it isn't, it is reported as not tested.
- **K06:** no `dbus-daemon`. A harness-owned Unix listener at `<root>/bus/socket` accepts connections and never replies.

### 4.5 Real-path exclusion without touching the real bus
- Outside, before and after the run: `stat` of `/run/user/1000/bus` (inode, type, mtime) and a names-only listing plus mtimes of `~/.local/share/keyrings`. Nothing is opened, read or connected to. Any difference is an isolation failure.
- Inside: the masked paths must be empty tmpfs (checked from `/proc/self/mountinfo` and an empty listing).
- PAM, autostart, systemd user units, global D-Bus config and the real collections are not touched.

### 4.6 Synthetic secrets
- Per case run, two values are made: credential `C` and unlock password `U`. Each is 32 bytes from `/dev/urandom`, hex-encoded in memory (no new crate).
- `U` goes to gnome-keyring via its stdin pipe only. `C` goes in process to `Entry::set_password`.
- Neither appears in argv, the environment, logs, outputs, reports or fixtures. Results record only booleans.
- Entry: service `pwt-p00-t07-<random-hex>`, user `synthetic`.

### 4.7 Timeouts and cleanup
- **App level:** each `keyring` call runs on a worker thread, and the case waits with `recv_timeout(10 s)`.
- **Case level:** the orchestrator (PID 1) kills a case process at 30 s and records R5.
- **Run level:** the outer 240 s watchdog.
- **Daemons** are children of PID 1, terminated explicitly (SIGTERM, then SIGKILL after 2 s) and then reaped. Namespace teardown is the backstop.
- **Files:** the orchestrator never deletes outside `<spike-root>`. After the run, outside, the exact `mktemp` path is removed after checking its `_spikes/t07.` prefix and ownership.
- **Post-check:** `pgrep -a -u kali 'dbus-daemon|gnome-keyring-d'` before and after. Any new process is reported, and no pre-existing process is signalled.

## 5. Cases and verdict rules
Every case records:
- **R1** whether the library call returned, and the typed `keyring_core::Error` variant;
- **R2** whether a prompt was requested (from the error or observed `Prompt` path; best effort);
- **R3** whether the app-level timeout fired;
- **R4** whether the worker was still blocked after R3. If so, the verdict is "leaked worker — not cancelled";
- **R5** whether the orchestrator or watchdog killed the case process;
- **R6** whether all spike daemons, sockets and files were cleaned up.

A **harness failure** (a mount, environment, address or daemon-start assertion failing) is never reported as a service verdict.

| Case | Setup | Pass condition | Harness-failure vs service distinction |
|---|---|---|---|
| K01a no bus, runtime dir set | no `DBUS_SESSION_BUS_ADDRESS`; `XDG_RUNTIME_DIR=<case>/run` (empty) | `store_status()` returns a typed platform/no-storage error; `Entry::new` → `NoDefaultStore`; R1 returned | preflight asserts `<case>/run/bus` and `/run/user/*/bus` absent; a successful connection here would be an isolation failure |
| K01b no bus, runtime dir unset | as K01a but `XDG_RUNTIME_DIR` unset (exercises the `/run/user/0/bus` fallback) | as K01a | as K01a |
| K02 bus, no service | private dbus-daemon, empty servicedir | typed error (expected: service unknown → platform failure), returned within 10 s | harness failure if the daemon did not start or did not print the expected address |
| K03 round-trip | private bus plus explicit unlocked gnome-keyring-daemon | set → get (equal) → replace → get (new) → delete → get (`NoEntry`), each within 10 s; entry absent at the end | harness failure if the daemon did not start, its stdin unlock failed, or its files were not under `<case>/data` |
| K04 locked / no default collection | daemon without `--unlock`; no prompter activatable | **API-level handling only if** a typed error returns within 10 s (R1). Otherwise R3/R4/R5 are recorded as a timeout-containment observation (source review predicts a blocking prompt wait) | the impossibility of a prompter is by construction (empty servicedir, masked standard dirs) |
| K05 no plaintext fallback | after all cases | `C`/`U` (raw and hex) absent from case stdout/stderr captured by the orchestrator, from `<case>/{home,config,cache,run,out}` and from harness output | `<case>/data/keyrings/` is scanned separately and reported as **service storage** (expected encrypted, not an app fallback); only spike-owned paths are scanned |
| K06 unresponsive bus | harness listener that never replies | same rule as K04: only a typed error within 10 s counts as API handling; otherwise containment observation | harness failure if the listener did not bind |

P03 carry-forward (not decided here):
- If K02, K04 or K06 show no typed, bounded failure, OS-store mode needs an application-owned connection with `method_timeout` and a validated address. That means a direct `secret-service`/`zbus` dependency, which needs separate approval, or the mode stays unavailable.
- The fallback is always session-only or disabled AI.

## 6. Limitations stated in advance
- No filesystem sandbox beyond the listed masks.
- gnome-keyring 50.0 and dbus-daemon 1.16.2 on this host only. KWallet and KeePassXC are absent and untested.
- A headless private bus is not a desktop login session, so PAM auto-unlock is out of scope.
- Leaked worker threads die with their case process; that is containment, not cancellation.

## 7. Approvals requested
1. **C:** harness implementation (Opus, std + `keyring` only) and the per-package offline build in §3.
2. **D:** the env check, then the run in §4–§5. This includes the new mount namespace with the listed tmpfs masks, starting `dbus-daemon` and `gnome-keyring-daemon` inside it, the synthetic secrets, and K01a–K06.
3. Separately: commit/push of the C/D results.
