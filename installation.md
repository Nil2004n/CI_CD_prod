# PWT Installation Guide

How to install, launch, configure, verify, update and uninstall the current
PWT 1.1.0 **binary** packages for Linux and Windows. For building PWT from
source (Maven, `make test`), see `README.md` instead. The binary packages
contain no Java source and cannot build or test it.

## Contents

- [Release Status](#release-status)
- [Choose the Correct Package](#choose-the-correct-package)
- [Linux x86-64](#linux-x86-64)
- [Windows x86-64](#windows-x86-64)
- [Claude Code and OpenCode](#claude-code-and-opencode)
- [Updating](#updating)
- [Uninstalling](#uninstalling)
- [Troubleshooting](#troubleshooting)
- [Verification and Known Limitations](#verification-and-known-limitations)

---

## Release Status

**This document is not authorisation to distribute either package.**

| | Linux | Windows |
|---|---|---|
| Candidate | **PWT 1.1.0 beta rc2** (`pwt-1.1.0-beta-rc2-linux-x86_64`) | **PWT 1.1.0 beta rc2 working directory** (`pwt-release1.1.0-win-rc2/`) |
| Technical status | **Technical packaging verification completed:** TECHNICALLY CLOSED — DISTRIBUTION APPROVAL PENDING | **Native Windows verification pending:** WINDOWS ZIP PREPARED — NATIVE VERIFICATION PENDING |
| Tested on | **Kali GNU/Linux Rolling, x86-64 only** | **Not run on Windows.** Checks so far ran on Linux (static and PowerShell 7 logic only). |
| Use of these instructions | Controlled beta testing once approved | **Controlled validation only.** Not a client-ready release. |

On both platforms, the **PWT beta terms** and the applicable **third-party
and source-distribution approvals** are pending. Neither package is approved
for client delivery. The current gates are in
`release-artifacts/RELEASE-STATUS.md` and
`release-artifacts/OWNER-APPROVAL-CHECKLIST.md`.

---

## Choose the Correct Package

| Platform | Use | Name | Notes |
|---|---|---|---|
| Linux x86-64 | **Preferred** | `pwt-1.1.0-beta-rc2-linux-x86_64.tar.gz` | Keeps the launchers' executable bits |
| Linux x86-64 | Alternative | `pwt-1.1.0-beta-rc2-linux-x86_64.zip` | Some unzip tools drop executable bits (see the Linux section) |
| Windows x86-64 | **Current working candidate** | the folder `pwt-release1.1.0-win-rc2/` | **No rc2 Windows ZIP exists yet.** Copy the folder to the Windows machine. |
| Windows x86-64 | Earlier candidate, frozen | `pwt-1.1.0-beta-windows-x86_64.zip` | **Does not include the rc2 launcher corrections.** Do not use it for new validation. |

Superseded and not for use: `pwt-1.1.0-beta-linux-x86_64.tar.gz` and
`pwt-1.1.0-beta-linux-x86_64.zip`.

Download location: `<download location supplied by the PWT owner>`. There is
no public URL.

### Checksums

The values below are copied from the checksum files in `release-artifacts/`.

```text
# release-artifacts/pwt-1.1.0-beta-rc2-linux-x86_64-SHA256SUMS.txt
e74a5c92c429a7f5c3c462a6d27f1aa23e7b0d53ce9a8eaf294a6cc0fdeaf33d  pwt-1.1.0-beta-rc2-linux-x86_64.tar.gz
573716b9e343989843b87ff026c806748b86718e0a6fb0c3fb94545ba9a2b817  pwt-1.1.0-beta-rc2-linux-x86_64.zip
d4e08039b878fad47fd9d15238f7835c8c60adbc603d4248e92616b66e853927  openjfx-21.0.8+2-src.tar.gz
8887c20649383450f6363161165e0aa16079c4f0f3432b996d37c2ca34761608  openjfx-21.0.8+2-src.PROVENANCE.txt

# Windows rc2 working directory: SHA-256 of its manifest file
# pwt-release1.1.0-win-rc2/SHA256SUMS (from WINDOWS-NATIVE-TEST-HANDOFF-rc2.md)
9c0033739ff1ef32b2d796c6b003f58771d9cf38a729a325912820e37f980984  SHA256SUMS

# release-artifacts/pwt-1.1.0-beta-windows-x86_64-SHA256SUMS.txt  (earlier candidate, frozen)
1cd2721b5bb4fc2ba749c7880b250d782f5788e1709b4815199e3c47769deb4a  pwt-1.1.0-beta-windows-x86_64.zip
```

**What a checksum establishes:** a matching SHA-256 shows that your copy is
byte-identical to the file the checksum was computed from. It detects
corruption and accidental substitution. These checksums are **unsigned**, so
they do **not** prove who published the package. Compare them against values
received through a separately trusted channel. The `SHA256SUMS` files
*inside* each package only detect corruption.

---

## Linux x86-64

### Platform and prerequisites

- **Supported:** Linux on x86-64 with glibc. `make build` refuses other
  platforms.
- **Tested:** Kali GNU/Linux Rolling only. Other distributions are untested.
- **Not supported:** ARM64, musl/Alpine, Windows, macOS.
- **No `sudo`:** everything installs per user.

| Requirement | Bundled? | Notes |
|---|---|---|
| Java **21 or newer** | No | Found via `PWT_JAVA`, then `$JAVA_HOME/bin/java`, then `java` on `PATH` |
| Python 3 (standard library only) | No | Used by the installer and setup helpers |
| GNU make, POSIX `/bin/sh` | No | Used by the `make` targets and launchers |
| GTK 3 and an X11 or Wayland session | No | Only for the desktop dashboard |
| PWT application jar (with JavaFX for Linux x86-64) | **Yes** | `pwt/target/pwt-1.1.0.jar` (the published GitHub v1.1.0 artifact) |
| Scanners | No | Optional; see [Scanners](#scanners-linux) |

### Verify, extract and validate

The archive contains one folder, `pwt-version1.1.0_release/`. Extract it to a
**stable, user-writable** location. The installed launcher and the client
configuration point into this folder, so it must stay where it is.

```sh
# Linux shell, in the folder where you downloaded the archive
sha256sum pwt-1.1.0-beta-rc2-linux-x86_64.tar.gz
# compare with the value received through the trusted channel

mkdir -p ~/.local/opt
tar -xzf pwt-1.1.0-beta-rc2-linux-x86_64.tar.gz -C ~/.local/opt
cd ~/.local/opt/pwt-version1.1.0_release
make build
```

`make build` **does not compile**; this package contains no source. It
validates the packaged executable:

- the platform;
- Java 21 or newer;
- the jar's SHA-256 against `SHA256SUMS`;
- the version embedded in the jar.

It never runs Maven. `make test`, `make test-integrations` and `make clean`
are not available in the binary package; they print an error.

**If you used the ZIP and see "Permission denied":** some unzip tools drop
executable bits. Restore them on exactly these three launchers, from inside
`pwt-version1.1.0_release`:

```sh
# Linux shell
chmod 755 integrations/pwt.sh integrations/pwt-mcp-stdio.sh integrations/pwt-dashboard.sh
```

Do not use `chmod -R`, and do not mark other files executable.

### Install (per user)

```sh
# Linux shell, inside pwt-version1.1.0_release
make install
```

**What it installs:**

- `~/.local/bin/pwt` (change the location with `PREFIX=/abs/dir`);
- a desktop entry;
- the OpenCode plugin, agent and `/pwt-ask` under `~/.config/opencode`;
- when Claude Code is detected, the `/pwt` commands and the read-only `pwt`
  subagent under `~/.claude`.

**What it does not do:**

- register a project (even if you pass `ROOT=`, or for the extracted PWT
  folder);
- add any MCP entry;
- overwrite files that PWT doesn't own.

If `~/.local/bin` is not on your `PATH`, the installer says so.
`make install-shell-rc` (opt-in) appends it to `~/.bashrc`.

### Data location (Linux)

All PWT parts (dashboard, CLI, MCP and client setup) use **one** data
directory, outside the release folder. It's chosen in this order:

1. an explicit `--data-dir /abs/dir` (launchers), or `make DATA_DIR=/abs/dir`
   **on the command line**;
2. a nonempty `PWT_DATA_DIR`, which must be absolute;
3. `$XDG_DATA_HOME/pwt`, only if `XDG_DATA_HOME` is an absolute path;
4. `~/.local/share/pwt`.

Notes:

- A `DATA_DIR` set in the *environment* (not on the `make` command line) is
  ignored, with a warning. `make help` prints the directory in effect.
- An existing store is used as-is: never reset or migrated. A new directory
  is created with mode 0700.
- `make install` and the setup targets **record** the directory they
  resolved. To change it, rerun them with the new `DATA_DIR=`.
- **Direct `java -jar` caveat:** running `java -jar pwt/target/pwt-1.1.0.jar`
  without `--data-dir` bypasses this policy. It uses `PWT_DATA_DIR`, otherwise
  `<release folder>/.pwt-data`. Use `pwt …` or the `make` targets.

### Launch and check

```sh
# Linux shell
pwt doctor          # Java, artifact, data directory, scanner availability (or: make doctor)
make start          # dashboard in this terminal; errors are shown here
make dashboard      # dashboard through the helper: one window per machine
pwt repo-list       # registered projects (or: make repo-list)
```

**`make start` vs `make dashboard`:**

- `make start` runs the dashboard directly, with output in your terminal.
- `make dashboard` goes through `integrations/pwt-dashboard.sh`. That helper
  refuses to open a second window if **any** PWT dashboard is already running
  on the machine, even one using another data directory. `/pwt` in the
  clients uses the same helper.

Scans are started and cancelled **in the dashboard**. According to the
application's implementation (`ScanService`), the first dashboard or CLI scan
of a directory registers it automatically; MCP never scans.

### Register a project (explicit)

```sh
# Linux shell, inside pwt-version1.1.0_release
make repo-add ROOT=/absolute/path/to/project   # register only
make enable   ROOT=/absolute/path/to/project   # register + OpenCode project configuration
```

`ROOT` must be an absolute path to an existing directory, given on the
command line. The release folder, and anything inside it, is refused.

### Scanners (Linux) <a id="scanners-linux"></a>

| Scanner | Bundled? | How to get it | Package verification | When missing |
|---|---|---|---|---|
| yamllint | No | Install it on `PATH` from your distribution or the upstream project. The package prescribes no method. | Ran successfully on Kali (1.38.0) | Reported UNAVAILABLE |
| gitleaks | No | as above | Ran successfully on Kali (8.30.1) | Reported UNAVAILABLE |
| checkov | No | as above | Ran successfully on Kali (3.4.4) | Reported UNAVAILABLE |
| actionlint | No | as above | Ran successfully on Kali (v1.7.12) | Reported UNAVAILABLE |
| kubeconform | No | as above. Also needs local schemas: `pwt kubeconform-setup`. | Ran on Kali (development build); without schemas a coverage gap is reported | Reported UNAVAILABLE |
| local-schema | Built in | Needs schema bindings | Skipped when no bindings are given | SKIPPED |

**A missing or unavailable check does not mean the project passed.** It
appears as UNAVAILABLE, SKIPPED or a coverage gap, never as clean.

---

## Windows x86-64

> **Native verification pending.** None of the `.cmd` entry points, the
> Windows-only installer steps, the bundled runtime, the dashboard or the
> scanners have been run on Windows yet. Use these steps for **controlled
> validation** (see `release-artifacts/WINDOWS-NATIVE-TEST-HANDOFF-rc2.md`).

### Platform and prerequisites

- **Target:** Windows 10/11 on x86-64 (AMD64). The installer refuses other
  architectures.
- **Not required by design:** WSL, Git Bash, Maven, GNU make, or a separate
  Java installation. Python is needed only for the Python-based scanners.

| Requirement | Bundled? | Notes |
|---|---|---|
| Java runtime | **Yes** | Eclipse Temurin JRE 21.0.13+9 for Windows x64, in `runtime\` |
| JavaFX for Windows | **Yes** | OpenJFX 21.0.8 `win` jars in `lib\` |
| PWT application | **Yes** | `lib\pwt-app-1.1.0.jar`: the published v1.1.0 classes, repackaged unchanged |
| Windows PowerShell 5.1 | Built into Windows 10/11 | Used by install, uninstall and setup. Each `.cmd` passes `-ExecutionPolicy Bypass` for its own PowerShell process only; no policy setting is changed. Do **not** change the machine-wide execution policy. |
| Scanners | No | Optional; see [Scanners](#scanners-windows) |

### Copy the rc2 folder and verify its manifest

There is no rc2 Windows ZIP. Copy the folder `pwt-release1.1.0-win-rc2\` to
the Windows machine (for example to `C:\PWT Test\pkg\`), then check the
manifest file's hash before installing:

```bat
:: Windows Command Prompt
powershell -NoProfile -Command "(Get-FileHash 'C:\PWT Test\pkg\pwt-release1.1.0-win-rc2\SHA256SUMS' -Algorithm SHA256).Hash.ToLower()"
:: expected: 9c0033739ff1ef32b2d796c6b003f58771d9cf38a729a325912820e37f980984
```

`install.cmd` then re-checks every file listed in that `SHA256SUMS` and
refuses to install on any mismatch.

### Three command contexts (not interchangeable)

| Context | Location | Commands |
|---|---|---|
| **Extracted package** | the copied `pwt-release1.1.0-win-rc2\` folder | `install.cmd`, `uninstall.cmd` |
| **Installed application** | default `%LOCALAPPDATA%\Programs\PWT` | `bin\pwt.cmd`, `bin\pwt-dashboard.cmd`, `bin\pwt-setup.cmd`, `uninstall.cmd` |
| **Optional PATH entry** | only after `install.cmd -AddToUserPath` and a **new** terminal | `pwt.cmd`, `pwt-dashboard.cmd` and `pwt-setup.cmd` by name |

`pwt-setup.cmd` works only from the **installed** copy. Run from anywhere
else, it refuses: "Run install.cmd first, then use the installed
bin\pwt-setup.cmd".

### Install (current user; no administrator rights)

Open **Command Prompt** (or PowerShell) in the copied package folder:

```bat
:: Windows Command Prompt, inside pwt-release1.1.0-win-rc2
install.cmd
```

**Supported options** (from `tools\pwt-install.ps1`):

| Option | Meaning |
|---|---|
| `-InstallDir "<absolute path>"` | Install location (default `%LOCALAPPDATA%\Programs\PWT`) |
| `-DataDir "<absolute path>"` | Data folder (default: see below) |
| `-AddToUserPath` | Add `<install>\bin` to **your user** PATH (opt-in; never the system PATH) |
| `-SkipClaude` | Don't install the Claude Code commands and subagent |

**What `install.cmd` does:**

- verifies the package;
- **copies** it to the install folder, so the copied package folder can be
  removed afterwards;
- creates the data folder if missing;
- writes a Start Menu entry, **PWT YAML Workbench**;
- installs the Claude Code commands and subagent if Claude Code is detected
  (a `.claude` folder in your profile, `CLAUDE_CONFIG_DIR`, or `claude` on
  PATH).

**What it doesn't do:** register a project, add an MCP entry, or replace a
folder or file that PWT doesn't own.

It refuses while PWT is running from the install folder.

### Data location (Windows)

1. explicit `--data-dir "<absolute path>"` (`pwt.cmd`, `pwt-dashboard.cmd`) or
   `-DataDir` (`install.cmd`, `pwt-setup.cmd`);
2. a nonempty absolute `PWT_DATA_DIR`;
3. `%LOCALAPPDATA%\PWT`.

If none of these can be resolved, PWT reports an error. There is **no**
fallback to the application folder.

Existing data is used as-is. MCP entries carry the data folder explicitly
(`PWT_DATA_DIR` in their environment).

### Launch (installed application)

```bat
:: Windows Command Prompt
"%LOCALAPPDATA%\Programs\PWT\bin\pwt.cmd" --version
"%LOCALAPPDATA%\Programs\PWT\bin\pwt.cmd" doctor
"%LOCALAPPDATA%\Programs\PWT\bin\pwt-dashboard.cmd"
"%LOCALAPPDATA%\Programs\PWT\bin\pwt.cmd" scan --root "C:\path\to\project" --file app.yaml
```

- **`pwt.cmd`** forwards its arguments unchanged to PWT (modes `gui`,
  `discover`, `scan`, `mcp`, `repo-add`, `repo-list`, `doctor`, `mcp-status`,
  `journal`, `kubeconform-setup`, `--help`, `--version`). `pwt.cmd gui` runs in
  the current console.
- **`pwt-dashboard.cmd`** starts the dashboard with `javaw.exe` and returns
  immediately. It has **no** single-instance check, so each call opens a
  window. The Start Menu entry runs it too.
- **Scans** are started and cancelled in the dashboard.

### Per-project setup (installed `pwt-setup.cmd`)

```bat
:: Windows Command Prompt
set "PWTB=%LOCALAPPDATA%\Programs\PWT\bin"
"%PWTB%\pwt-setup.cmd" register    -ProjectDir "C:\path\to\project"
"%PWTB%\pwt-setup.cmd" claude      -ProjectDir "C:\path\to\project"
"%PWTB%\pwt-setup.cmd" opencode    -ProjectDir "C:\path\to\project"
"%PWTB%\pwt-setup.cmd" test        -ProjectDir "C:\path\to\project"
"%PWTB%\pwt-setup.cmd" status      -ProjectDir "C:\path\to\project"
"%PWTB%\pwt-setup.cmd" unconfigure -ProjectDir "C:\path\to\project"
```

| Action | Effect |
|---|---|
| `register` | Registers the project in the shared data store |
| `claude` | Writes the project's `.mcp.json` `pwt` entry |
| `opencode` | Writes the project's `opencode.json` `pwt` entry |
| `test` | Real MCP handshake using the configured Claude entry. Expects `start_scan` and `cancel_scan` to be denied. |
| `status` | Shows registration and the configured entries |
| `unconfigure` | Removes PWT's own `pwt` entries from that project's files |

Common options: `-DataDir "<abs>"`, `-InstallDir "<abs>"`, and
`-AllowAnalysis 1|0` (for `claude`/`opencode`; analysis writes are denied
unless `1`).

- **Project path rules:** it must be absolute and existing, and may not be
  the install folder, the data folder, or inside them.
- **OpenCode:** the helper refuses projects that use `opencode.jsonc`.
- **Ownership:** only a `pwt` entry that PWT created is replaced; other
  servers and keys are kept.

### Scanners (Windows) <a id="scanners-windows"></a>

The launchers look scanners up on **PATH only** (`where $PATH:<name>`, never
the current folder) and pass them to PWT. None of this has been tested on
Windows.

| Scanner | Bundled? | Guidance from the package | Windows verification | When missing |
|---|---|---|---|---|
| yamllint | No | Python 3.10+, then `pip install yamllint` | Not verified | Reported unavailable |
| gitleaks | No | Official `windows_x64` zip from the gitleaks releases | Not verified | Reported unavailable |
| checkov | No | Python 3.10+, then `pip install checkov` | Not verified | Reported unavailable |
| actionlint | No | Official `windows_amd64` zip from the actionlint releases | Not verified | Reported unavailable |
| kubeconform | No | Official `windows-amd64` zip from the kubeconform releases; local schemas via `pwt.cmd kubeconform-setup` | Not verified | Reported unavailable |
| local-schema | Built in | Needs schema bindings | Not verified | Skipped without bindings |

A missing or unavailable check does **not** mean the project passed.

---

## Claude Code and OpenCode

Keep these steps separate. Each one is explicit.

| Step | Linux (rc2) | Windows (rc2 working directory) |
|---|---|---|
| User-wide commands and subagent | `make install` (if Claude Code is present) or `make install-claude`; the OpenCode plugin, agent and `/pwt-ask` via `make install` or `make install-opencode` | `install.cmd` (Claude Code only, if present). **No** OpenCode plugin, agent or `/pwt-ask` on Windows. |
| Register the project | `make repo-add ROOT=…` or `make enable ROOT=…` | `pwt-setup.cmd register -ProjectDir …` |
| Project MCP entry | `make setup-claude ROOT=…` (`.mcp.json`); `make enable ROOT=…` or `make setup-opencode ROOT=…` (`opencode.json` + native plugin) | `pwt-setup.cmd claude -ProjectDir …`; `pwt-setup.cmd opencode -ProjectDir …` (MCP entry only) |
| Optional global entry | `make setup-claude-global` (needs the `claude` CLI; bound per session to the start directory); `make setup-opencode-global` (unscoped; see the note below) | **None** on Windows |
| Check | `make claude-status ROOT=… [TEST=1]` | `pwt-setup.cmd status` / `test` |
| Client approval | **Your action:** Claude Code asks you to approve a project's `.mcp.json` server on first use | same |
| Chat use | `/pwt`, `/pwt help\|repos\|status\|report`, aliases, `/pwt-ask <question>` | same commands. `/pwt` and `/pwt-dashboard` only **explain how to open** the dashboard. |

Notes:

- **`make setup-opencode-global` (Linux)** refuses unless
  `~/.config/opencode/opencode.json` already contains an `"mcp"` object
  (behaviour of `integrations/setup_opencode.py`). The Linux `INSTALL.md` does
  not mention this.
- **Windows: no `.pwt\context.md` pointer.** This build cannot write the
  per-project pointer. The `pwt` subagent falls back to `get_project_status`
  for the journal path, so `/pwt-ask` on Windows depends on the project MCP
  entry being configured and approved.
- **Windows:** the OpenCode native plugin, agent and `/pwt-ask` are **not
  included**, and there are no global entries. The integration is **not**
  equivalent to Linux.

For both platforms:

- **Scanning:** starting and cancelling scans happens only in the
  dashboard. MCP `start_scan` and `cancel_scan` are denied.
- **Analysis writes** are denied unless explicitly enabled (`ALLOW_ANALYSIS=1`
  on Linux, `-AllowAnalysis 1` on Windows).
- **Approval:** client approval is a user action. Don't bypass trust
  prompts.
- **No sandbox:** the subagent's project boundary is an instruction, not an
  operating-system filesystem sandbox.

**Verification coverage of these integrations:**

- **Linux package:** the setup scripts, a direct MCP handshake (12 tools; scan
  and cancel denied), stub clients, and OpenCode plugin launch tests were run.
- **Windows:** setup logic and the handshake were run only under PowerShell 7
  **on Linux**.
- **Real Claude Code or OpenCode chat, `/pwt-ask` and subagent workflows are
  not verified from either package.**

---

## Updating

### Linux

1. Extract the new package to a **new** folder, then run `make build` and
   `make install` there.
2. Rerun `make enable ROOT=…` and/or `make setup-claude ROOT=…` for each
   project, so the entries point to the new folder.
3. Remove the old folder only once nothing points to it.

The data directory is reused unchanged. These keep installation-time paths
into the extracted folder:

- `~/.local/bin/pwt`;
- the desktop entry;
- the Claude Code commands' dashboard command;
- project and global MCP entries.

Moving the folder breaks them until you rerun those commands. There is no
automatic migration.

### Windows

1. Run the new package's `install.cmd` (with the same `-InstallDir` if you
   used one). It replaces the PWT-managed install folder and keeps your data.
2. Rerun `pwt-setup.cmd claude|opencode` for each project if the install
   location changed.

MCP entries and the Start Menu entry point into the install folder. There is
no automatic migration.

---

## Uninstalling

Uninstalling never deletes PWT data. Deleting data is a separate, explicit
choice (Linux: `make purge-data`, which asks you to type `YES`).

### Linux

```sh
# Linux shell, inside pwt-version1.1.0_release
make uninstall
make uninstall-claude ROOT=/absolute/path/to/project   # optional: one project's .mcp.json pwt entry
```

| Removed | Kept |
|---|---|
| `~/.local/bin/pwt` and the desktop entry (only if PWT-owned) | the data directory (repositories, runs, reports, journals) |
| the global OpenCode plugin, agent and `/pwt-ask` (only if PWT-managed) | project `.mcp.json`/`opencode.json` entries and `.pwt/context.md` pointers |
| the PWT-managed Claude Code commands and subagent, and the PWT-created **user-scope** Claude MCP entry | a global OpenCode `pwt` entry from `setup-opencode-global` |
| | the extracted release folder |

### Windows

```bat
:: Windows Command Prompt: from the package folder or %LOCALAPPDATA%\Programs\PWT
uninstall.cmd
:: options: -InstallDir "<absolute path>"  -SkipClaude
```

| Removed | Kept |
|---|---|
| the install folder (only if it carries the PWT marker) | the data folder (`%LOCALAPPDATA%\PWT` or your `-DataDir`) |
| PWT-managed Claude Code commands and subagent | project `.mcp.json`/`opencode.json` entries; remove them first with `pwt-setup.cmd unconfigure` if wanted |
| the PWT Start Menu entry, and the user-PATH entry if PWT added it | unrelated settings and files |

`uninstall.cmd` runs a temporary copy of its script from `%TEMP%`, so it can
also remove the install folder it was started from. (That behaviour was
exercised on Linux only.)

Do not delete `.jar.inuse` files or other files you didn't create as an
uninstall step. They are not part of the binary packages.

---

## Troubleshooting

### Linux

| Symptom | Likely cause | What to do |
|---|---|---|
| `make build` reports an unsupported platform | not Linux x86-64 | Use a supported system |
| `make build` reports no Java runtime, or Java below 21 | Java missing or too old | Install Java 21+ or set `JAVA_HOME`/`PWT_JAVA` |
| "Permission denied" after unzipping | ZIP tool dropped executable bits | `chmod 755` the three launchers listed above |
| A check shows UNAVAILABLE | scanner not on `PATH` | Install it; `pwt doctor` shows availability |
| `pwt-dashboard.sh`/`/pwt` reports "no graphical session" | no X11/Wayland display | Use a desktop session, or the CLI |
| `/pwt` or `make dashboard` reports "already running" | any PWT dashboard is open, even for another data directory | Close it, or use `make start` |
| Scanner table stays at "probing…" | known display issue | Read availability in the Activity tab or `pwt doctor` |
| A dashboard opened by `/pwt` closes at once | the detached helper discards output | Run `make start` in a terminal to see the error |

During earlier test sessions, two dashboard windows exited without an
explicit close. **No cause was established.** They are neither a confirmed
crash nor a confirmed user close.

### Windows

| Topic | Information |
|---|---|
| Native testing | **Pending.** Report results using `release-artifacts/WINDOWS-NATIVE-TEST-HANDOFF-rc2.md`. |
| Path characters | rc2's launchers avoid re-parsing paths, so folders with spaces, `&`, `^`, `%` or `!` are **expected** to work. This is **unverified**. Non-ASCII path failures are **hypotheses**, not confirmed limitations; section C of the handoff locates the failing boundary. A workaround, if needed: `-InstallDir "C:\PWT"`. |
| Launcher diagnostics | `set PWT_LAUNCH_DEBUG=1` before running `pwt.cmd` or `pwt-dashboard.cmd` prints the resolved runtime, classpath, data folder and scanner paths to stderr. **Review this output before sharing it**: it contains local paths and user names. |
| Missing context pointer | `repo-add` warns that `.pwt\context.md` was not written. This is expected in this build. |
| Integration differences | No dashboard launch from Claude `/pwt`; no OpenCode native plugin, agent or `/pwt-ask`; no global entries |
| A check shows unavailable | The scanner executable is not on PATH, or its prerequisite (Python 3.10+) is missing |

When sharing diagnostics on either platform:

- remove secrets from findings and reports;
- replace private paths and names if they are sensitive;
- do not send `pwt.db` or the data folder.

---

## Verification and Known Limitations

### Linux rc2: what was verified (Kali Linux x86-64 only)

All of this ran from fresh extractions with GNU tar, Info-ZIP unzip and
Python `zipfile`, using an isolated HOME, data folder and a synthetic project:

- archive checksums;
- `make build`, including fail-closed cases;
- install, idempotent reinstall, registration guards and the data-folder
  rules;
- uninstall preservation;
- a direct MCP handshake with scans denied;
- a headless scan;
- a short GUI session and a detached-helper session, both closed
  intentionally.

**Not verified:**

- other distributions;
- long-duration GUI stability;
- real Claude Code or OpenCode chat workflows from the package.

### Windows rc2 working directory: what was checked (not on Windows)

These ran on Linux:

- input checksums and signatures;
- application jar equivalence with the published jar;
- the classpath composition, using the Linux JavaFX jars;
- static lint of the `.cmd` launchers;
- PowerShell 7 runs of the install, setup and uninstall logic with a fake
  profile;
- a real MCP handshake with Linux Java standing in for the bundled runtime.

**Not verified:**

- every `.cmd` file;
- Windows PowerShell 5.1;
- the bundled runtime;
- the dashboard;
- the scanners;
- Windows path handling and the Windows-only installer steps;
- real client use.

### Known limitations

- **Linux:** x86-64 glibc only.
- **Linux:** the installed launchers depend on the extracted folder staying
  in place.
- **Both:** running `java -jar` (or the jars directly) bypasses the
  launchers' data policy.
- **Windows:** no `.pwt\context.md` pointer; reduced client integration (see
  above); Windows x86-64 only.
- **Both:** the scanners and, on Linux, Java are not bundled.

### Pending before any distribution

- **PWT beta terms:** not provided.
- **Third-party notices:** review pending (Linux), not reviewed (Windows).
- **OpenJFX corresponding source:** the companion archive is prepared but not
  approved.
- **Temurin/OpenJDK source (Windows):** not prepared.
- **Microsoft runtime DLL redistribution terms (Windows):** review pending.
- **Windows:** native verification pending.
