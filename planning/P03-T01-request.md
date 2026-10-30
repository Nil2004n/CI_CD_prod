# P03-T01 — P03 request: module layout, frozen interfaces, policies, test plan, packets

Status: **prepared by Opus 2026-10-30 under the owner's approval of P03-T01–T04** ("I approve p03 t01 to t04 … start the work"). Base: `phase/P03-permissions-credentials` at `5775437` (= origin), clean apart from the local `.commandcode/`. Decisions C1–C10 = recommended options (`plan.md` §5C.6). The lead decisions in §11 stay inside C1–C10. The owner may override any of them before wave 1, which is **not** approved yet (T05+ need a separate go).

T01 is lead-only because it defines shared contracts (CLAUDE.md "Opus owns shared contracts"). The owner asked for Sonnet workers and an orchestrating main session. Inside T01–T04 the only worker-suitable item is the P03 check runner (T04b, a dev-tool file with no shared-contract content), so exactly one `sonnet-implementer` worker is used, in its own worktree and branch (§10).

## 1. Facts checked read-only in this session (2026-10-30)

| Fact | Consequence | Basis |
|---|---|---|
| `domain::text::redact_static` is `pub(crate)`; no production `Redactor` impl may exist before P06 (static rule) | `application` cannot make any `RedactedText` itself. Approval `actor`/`purpose` and the `ApprovalRequested.destinations` event field need domain-provided constructors → §5.1 (lead, T04) | `domain/src/text.rs:397`, `dev/p02_check.py` |
| Contract `CredentialStatus` = `absent, session_only, stored, invalid, store_unavailable`; plan §5C.1 used FR-21 names `absent/present/invalid_format/unavailable` | No contract change. `present` → `session_only`. A badly formatted key is refused at input with a typed error and never stored, so it has no status. `invalid` is reserved for provider rejection (P16), and `stored`/`store_unavailable` for OS-store mode (not adopted, C2) → T03 | `domain/src/contract.rs:691` |
| `ApprovalRequested.workflow_revision` is `Option<u32>`; store `Approval.workflow_revision` is `Option<u64>` | P03 requests carry the revision as `u32` | `contract.rs:933`, `store/approval.rs` |
| `RawInput`'s `Debug` prints the byte length, pinned by `domain/tests/contract.rs:1088`. `RawInput` has no `Drop` | KEY-01 hardening changes both (lead, T04). The test assertion becomes stricter: no length | `contract.rs:265` |
| `CredentialSecret` already clears on drop (best effort) and its `Debug` shows no length | Kept. T07 adds tests only | `contract.rs:147–161` |
| The process euid is not available without `unsafe` (`libc::geteuid` is an unsafe fn; `unsafe_code = "forbid"`) | FR-33 reads `/proc/self/status` (`Uid:` line, second field) through a provided `FsOps` method → §5.6 | `Cargo.toml` lints |
| Cargo tests run inside `unshare -rn`, so the euid is a mapped 0 and test temp directories are root-owned in the namespace | FR-33 does not change the existing tests. Directories created outside the namespace (uid 1000 → `nobody`) would be refused under the runner, as intended | runner `run_cargo` |
| The checkov rev 2 contract fixes the child environment *exactly* (`PATH`, `LANG`, `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `TMPDIR`, `CKV_SKIP_PACKAGE_UPDATE_CHECK=True`) | Scanner profiles inherit nothing from the host (§7) | `evidence/P00/P00-T09/contract-checkov-rev2.md` |
| The P00-T04 Git calls used `env -i` with `HOME` scratch, `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`; `GIT_CONFIG_COUNT` injection fired when it was inherited | The `git` profile sets these and inherits nothing. P04 owns the rest of Git safety | `evidence/P00/P00-T04/record.md` |
| Edition 2024: `std::env::set_var` is `unsafe` | Tests cannot seed the real process environment. The host environment is an explicit input (`HostEnv`), read once in `cli/src/main.rs` | edition |
| `std::io::IsTerminal` is stable | TTY refusal for `credential set` (C5) needs no `unsafe` | std |
| CLI exit codes 0/2/3/4/5/6 are frozen; 6 "approval required" is reserved | P03 uses exit 6 for `NeedsApproval` | `cli/src/commands.rs` |
| `persistence::recovery` counts pending approvals and never grants them | The lifecycle keeps that guarantee | `recovery.rs:180` |

## 2. Scope
As `plan.md` §5C.1, refined by these decisions:
- The fingerprint covers *what* is done (capability, operation, arguments, repos, destinations, constraints, data categories). The *context* (workflow binding, run, lifetime, inference scope, status, expiry, consumption) is matched as separate dimensions (§6).
- `dev permission-check` (new `dev` command) is the CLI path for "a scope or revision mismatch prompts again" (plan §4.1 P03 row). It reads one synthetic request as JSON on stdin and exits 6 with a pending approval when approval is needed.
- No host-environment value reaches a scanner profile. Only the `internal` profile inherits anything, and only `TZ` (§7).

## 3. Layout (paths relative to `workbench/crates/`)

```text
domain/src/contract.rs            # lead T04: FixedText (actor/purpose constants), RawInput Drop + Debug without length
domain/src/store/values.rs        # lead T04: Destination::to_redacted
domain/src/text.rs                # lead T04: pub(crate) redact_validated (sanitise + wrap), used only by values.rs
application/src/
  lib.rs                          # lead: module list
  permission/mod.rs               # lead (frozen types §5.2): PermissionRequest, RequestedData, OperationName, OperationArg,
                                  #   WorkflowBinding, Evaluation, NeedReason, RefusalReason, ScopeDimension, EvaluationContext
  permission/destination.rs       # W1 T05: NormalisedDestination::parse (C6)
  permission/fingerprint.rs       # W1 T05: canonical encoding v1 + SHA-256 (§5.3)
  permission/evaluate.rs          # W1 T05: evaluate() (§6)
  approvals.rs                    # W1 T08: ApprovalService, Authorisation, ApprovalError, CLI entry points
  child_env.rs                    # W2 T06: ToolProfile, ToolDirs, HostEnv, ChildEnv, EnvError, build(), preview()
  credential.rs                   # W3 T07: CredentialStore, SessionCredentialStore, read_key(), KeyFormatError
  dev_permission.rs               # W2 T10: `dev permission-check` (JSON request DTO, decode, evaluate, request)
  dev_store.rs                    # lead T04: DevError::RootForeignDataDir mapping only
persistence/src/
  error.rs                        # lead T04: StoreError::RootForeignDataDir
  fs.rs                           # lead T04 signature / W3 T09 body: FsOps::effective_uid (provided method), parse_status_euid
  paths.rs                        # W3 T09: FR-33 check in DataDir::new
cli/src/
  main.rs, lib.rs                 # lead T04: run_session(), Session (credential store + host env), stdin TTY flag
  args.rs, commands.rs, output.rs, features.rs   # W2 T10
cli/tests/p03_*.rs                # W2 T10
dev/p03_check.py                  # worker T04b (from dev/p02_check.py); dev/p03_baseline.json lead
```

Dependency direction is unchanged: `cli → application → persistence → domain`. New edge `application → sha2` (T02).

## 4. Dependencies (C3 = 3a)
- `application/Cargo.toml` adds `sha2 = { workspace = true }`. No other manifest change and no new crate.
- T02 resolves the lock offline (`cargo update --workspace --offline` in `unshare -rn`, in a scratch copy, as P02-T02). Accept only if the registry set (name, version, source, checksum) is unchanged and the only lock difference is `application`'s dependency list.
- FR-24 re-screen: **not applicable** while C2 = session-only. `keyring` stays spike-only.

## 5. Frozen interfaces
These are the lead-authored B0 stubs. Names, signatures, derives and visibility are frozen. Workers implement bodies and tests, and a change needs the lead's approval. Stub bodies use `todo!()` under `#[expect(clippy::todo, reason = "B0 stub")]`. `static --final` fails if any remain.

### 5.1 Domain additions (lead, T04, fully implemented at B0; no app/store contract change)
```rust
// contract.rs
/// Fixed approval texts; never user text. The only way the application obtains these RedactedTexts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FixedText { ActorRequester, ActorLocalUserCli, ActorLocalUserTui, PurposeDeveloperCheck }
impl FixedText { pub fn as_str(self) -> &'static str; pub fn redacted(self) -> RedactedText; }
// "requester", "local-user/cli", "local-user/tui", "developer permission check"

impl Drop for RawInput { /* best-effort clearing, same as CredentialSecret */ }
impl fmt::Debug for RawInput { /* "RawInput(<redacted>)", no length */ }

// store/values.rs
impl Destination { pub fn to_redacted(&self) -> RedactedText; }  // validated value; re-sanitised
```

### 5.2 `application::permission` (`mod.rs`, lead)
```rust
pub struct OperationName(String);          // parse: [a-z0-9._-], 1..=64 bytes; as_str
pub struct ArgName(String);                // parse: same charset, 1..=64
pub struct OperationArg { pub name: ArgName, pub value: String }   // value ≤ 65 536 bytes, never persisted
pub struct WorkflowBinding { pub workflow_id: WorkflowId, pub revision: u32 }   // revision ≥ 1
pub enum RequestedData { Category(DataCategory), RawSecretValues }
pub struct PermissionRequest {
    pub capability: Capability, pub operation: OperationName, pub arguments: Vec<OperationArg>,
    pub workflow: Option<WorkflowBinding>, pub run_id: Option<RunId>, pub repo_ids: Vec<RepoId>,
    pub destinations: Vec<NormalisedDestination>, pub resource_constraints: Vec<SanitisedText>,
    pub data: Vec<RequestedData>, pub purpose: RedactedText, pub lifetime: ApprovalLifetime,
    pub validity_seconds: Option<u32>, pub inference_scope: Option<InferenceScope>,
}
pub struct EvaluationContext { pub now: UtcTimestamp, pub current_workflow_revision: Option<u32> }
pub enum Evaluation { Permitted(Grant), NeedsApproval(NeedReason), Refused(RefusalReason) }
pub struct Grant { /* private: approval_id, lifetime */ }  // accessors only; constructible in `permission` only
pub enum NeedReason { NoGrant, AlreadyPending(ApprovalId), Mismatch { approval_id: ApprovalId, dimension: ScopeDimension } }
pub enum RefusalReason { RawSecretRequested, MissingDestination, UnexpectedDestination, LifetimeBinding,
    UnknownWorkflow, StaleWorkflowRevision, InferenceScope, PreviouslyDenied(ApprovalId), InvalidRequest }
pub enum ScopeDimension { Status, Capability, Fingerprint, Workflow, WorkflowRevision, Run, Repos, Destinations,
    ResourceConstraints, DataCategories, InferenceScope, Lifetime, Expiry, Consumption }
pub enum ArgError { Empty, TooLong, Character, Duplicate }
pub fn evaluate(request: &PermissionRequest, approvals: &[Approval], ctx: &EvaluationContext) -> Evaluation; // evaluate.rs
pub fn fingerprint(request: &PermissionRequest) -> Result<Digest, ArgError>;                                   // fingerprint.rs
```
`NormalisedDestination` (`destination.rs`): `parse(&str) -> Result<Self, DestinationError>`, `scheme()`, `host()`, `port()`, `resource()`, `canonical() -> String`, `to_store() -> Destination`. `DestinationError` = `{ Syntax, Scheme, Userinfo, NonAscii, Punycode, Wildcard, IpLiteral, Port, Query, TooLong }`, fieldless. All derives: `Clone, PartialEq, Eq, Debug` (+ `Hash, Copy` where possible). No serde on any of these types (the dev DTO lives in `dev_permission.rs`).

### 5.3 Canonical fingerprint encoding v1 (T05 implements; golden vectors pinned in tests; the lead re-computes them independently in Python at T11)
```text
digest = SHA-256( "pwt-operation-fingerprint" 0x00 "v1" 0x00 || F1 || … || F7 )
str(s)     = u32 big-endian byte length || UTF-8 bytes
list(xs)   = u32 big-endian count || item…   (items sorted by their encoded bytes, duplicates removed)
F1 capability            str(snake_case serde name)
F2 operation             str(OperationName)
F3 arguments             list(str(name) || str(value))   — duplicate names are an ArgError::Duplicate
F4 repo_ids              list(str(lowercase hyphenated UUID))
F5 destinations          list(str(NormalisedDestination::canonical()))
F6 resource_constraints  list(str(SanitisedText))
F7 data                  list(str(category snake_case)) — RawSecretValues encodes as str("raw_secret_values")
```
Excluded on purpose: workflow binding, run, lifetime, validity, purpose, inference scope, actor and timestamps. These are matched as dimensions. The fingerprint is never authorisation by itself (rule 15): no code path may return `Permitted` from a fingerprint match alone.

### 5.4 `application::approvals` (T08)
```rust
pub enum Actor { LocalUserCli, LocalUserTui }          // the requester text is FixedText::ActorRequester
pub struct Authorisation { /* private: approval_id, lifetime */ }  // only proof P04/P16 accept; not Clone
pub enum ApprovalError { Store(DevError), NotFound, NotPending, NotRevocable, WorkflowScopeUnavailable,
    Refused(RefusalReason), Fingerprint(ArgError), Emit }
pub struct ApprovalService<'a> { /* writer: &'a mut StoreWriter, emitter: &'a mut Emitter<…>, clock: &'a dyn Clock, ids */ }
impl ApprovalService<'_> {
    pub fn request(&mut self, req: &PermissionRequest, ctx_rev: Option<u32>) -> Result<RequestOutcome, ApprovalError>;
    pub fn resolve(&mut self, id: ApprovalId, decision: ApprovalDecision, actor: Actor, current_rev: Option<u32>) -> Result<ApprovalState, ApprovalError>;
    pub fn revoke(&mut self, id: ApprovalId, actor: Actor) -> Result<(), ApprovalError>;
    pub fn expire_due(&mut self) -> Result<u32, ApprovalError>;
    pub fn authorise(&mut self, req: &PermissionRequest, current_rev: Option<u32>) -> Result<AuthOutcome, ApprovalError>;
}
pub enum RequestOutcome { Permitted, Pending(ApprovalId), Refused(RefusalReason) }
pub enum AuthOutcome { Authorised(Authorisation), NeedsApproval(NeedReason, ApprovalId), Refused(RefusalReason) }
// CLI entry points (data dir explicit, like dev_store): list, resolve, revoke → DevOutcome-style result
```
The exact generic parameters of `ApprovalService` are fixed by W1 at T08 and reviewed by the lead. The method set and semantics are frozen here. Decision table (C7):

| Decision | Request lifetime | Result |
|---|---|---|
| `approve_once` | `once` or `workflow_revision` | approved, lifetime `once` |
| `approve_once` | `run` (needs `run_id`) | approved, lifetime `run` |
| `approve_workflow_scope` | any, with workflow binding whose revision = current saved revision | approved, lifetime `workflow_revision` |
| `approve_workflow_scope` | no binding, or stale revision | `WorkflowScopeUnavailable`, nothing written |
| `deny` | any | denied (no payload beyond the record's own fields) |

- `decided_at` = clock; `expires_at` = `decided_at + validity_seconds` if the request has one, else `null`.
- `actor` is set from `Actor` at resolution and stays `FixedText::ActorRequester` while pending.
- Only `resolve`/`revoke` with an `Actor` change status. There is no other path (rule 12, UI §10).
- `authorise` on a `once` grant writes `consumed = true` through `StoreWriter::replace` **before** returning `Authorised`. A failed write returns `Err` and the operation must not run. After an interruption the grant is consumed (never re-granted) or unconsumed (the caller never received `Authorised`), never both.
- `run` and `workflow_revision` grants are not consumed.
- Events: `ApprovalRequested` (new pending only), `ApprovalResolved`, `ApprovalRevoked`. They go through the P02 `Emitter` and are appended to `log.json` after the approvals write. A log-write failure halts the writer and is reported, and the approval state already written stays.

### 5.5 `application::child_env` (T06)
```rust
pub enum ToolProfile { Git, Yamllint, Gitleaks, Checkov, Actionlint, Kubeconform, Internal }   // ALL, as_str, parse
pub struct ToolDirs { /* private: home, config, cache, tmp: PathBuf */ }   // new(): absolute, no `..`, valid UTF-8 not required
pub struct HostEnv { /* private Vec<(OsString, OsString)> */ }           // from_pairs(iter); empty()
pub struct ChildEnv { /* private: Vec<(OsString, OsString, Origin)> */ }  // no constructor outside child_env
pub enum Origin { Fixed, AppDir, Inherited }
pub enum EnvError { DeniedName, SecretValue, InvalidDir }
pub trait SecretProbe { fn appears_in(&self, value: &[u8]) -> bool; }
pub fn build(profile: ToolProfile, dirs: &ToolDirs, host: &HostEnv, probe: &dyn SecretProbe) -> Result<ChildEnv, EnvError>;
impl ChildEnv { pub fn vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)>; pub fn preview(&self) -> Vec<PreviewLine>; }
pub struct PreviewLine { pub name: SanitisedText, pub origin: Origin }   // never a value
```

### 5.6 `application::credential` (T07) and FR-33 (T09)
```rust
pub trait CredentialStore: SecretProbe + fmt::Debug {
    fn status(&self) -> CredentialStatus;
    fn set(&mut self, secret: CredentialSecret) -> CredentialStatus;   // replaces any current key
    fn delete(&mut self) -> CredentialStatus;
}
pub struct SessionCredentialStore { /* Option<CredentialSecret> */ }    // new(); Default; Debug shows status only
pub enum KeyFormatError { Empty, TooShort, TooLong, Character, TerminalInput, Read }
pub fn read_key(input: &mut dyn Read, is_terminal: bool) -> Result<CredentialSecret, KeyFormatError>;
// persistence
pub trait FsOps { … fn effective_uid(&self) -> io::Result<u32> { process_effective_uid() } }   // provided method
pub fn parse_status_euid(status: &[u8]) -> Option<u32>;    // fs.rs, pure, tested
StoreError::RootForeignDataDir;  DevError::RootForeignDataDir (class Operation → exit 5)
```
- **Key format (C3/C5).** Read at most 4 099 bytes (`MAX_KEY_BYTES` + 3, enough to see a line break and one byte too many), then remove one trailing `\n` or `\r\n`. The key must be 16–4 096 bytes of ASCII graphic characters (0x21–0x7E). Any other input is rejected without keeping it: the read buffer is cleared on every path. TTY stdin → `TerminalInput`, and nothing is read.
- **Event.** `CredentialStatusChanged { status }` is emitted on `set`/`delete`. It carries status only.
- **Hardening (KEY-01).** No key in argv, environment, logs, stdout/stderr, store files or `Debug`. Clearing is best effort and is never described as secure erase.
- **FR-33.** `DataDir::new` calls `fs.effective_uid()`. If that fails → `PathRejected` (fail closed). If euid == 0 and the directory's uid ≠ 0 → `RootForeignDataDir`. In all other cases, behaviour is unchanged.

### 5.7 CLI (T10; lead fixes the entry seam in T04)
```rust
pub struct Session { /* credentials: SessionCredentialStore, host_env: HostEnv */ }  // new(host_env)
pub fn run_session<I, R, W, E>(session: &mut Session, args: I, stdin: R, stdin_is_terminal: bool, stdout: W, stderr: E) -> ExitCode;
pub fn run<…>(args, stdin, stdout, stderr) -> ExitCode   // unchanged signature: run_session(&mut Session::new(HostEnv::empty()), …, false, …)
```
`main.rs` collects `std::env::vars_os()` into `HostEnv` (the only environment read in `src`) and passes `stdin().is_terminal()`.

| Command | Exit |
|---|---|
| `approvals list --data-dir D` | 0; 3/4/5 on store errors |
| `approvals approve <id> --data-dir D [--scope once\|workflow]` (default `once`) | 0; 5 not found / not pending / scope unavailable |
| `approvals deny <id> --data-dir D`, `approvals revoke <id> --data-dir D` | 0; 5 |
| `credential set --session` (stdin pipe only) | 0; 5 format/TTY refusal; prints "held for this invocation only" |
| `credential status`, `credential delete` | 0 (`absent` in a new process) |
| `dev env-preview <tool> --data-dir D` | 0; names and origins only, values masked; dirs `D/tools/<tool>/{home,config,cache,tmp}` (not created) |
| `dev permission-check --data-dir D` (JSON request on stdin) | 0 permitted (a `once` grant is consumed), 6 needs approval (pending request recorded, id printed), 5 refused/invalid |

### 5.8 Static rules added by `dev/p03_check.py` (T04b)
| Rule | Allowlist |
|---|---|
| no `std::process::Command` / `process::Command` / `Command::new` in `src` (until P06) | none |
| environment reads (`ENV_RE` + `vars_os`/`vars`) only in `cli/src/main.rs` | `cli/src/main.rs` |
| `expose` only in `domain/src/contract.rs`, `application/src/credential.rs` | as listed |
| `CredentialSecret::new` only in `domain/src/contract.rs`, `application/src/credential.rs` | as listed |
| `serde_json::from_*` also allowed in `application/src/dev_permission.rs` | + that file |
| `redact_validated` called only in `domain/src/store/values.rs` (definition in `text.rs`) | as listed |
| `Authorisation` constructed (`Authorisation {`) only in `application/src/approvals.rs`; `Grant {` only in `application/src/permission/` | as listed |
| `#[allow]`/`#[expect]` equality with the approved list, ignoring `clippy::todo` unless `--final` | P02 list |
| registry set == P02 set (unchanged), lock sha == T02 value, manifest dependency allowlist unchanged | — |

Each new rule has negative and positive self-test cases.

## 6. Scope-match table (T05; every row needs ≥ 1 positive and ≥ 1 negative test)
Checked first on the request alone (Refused). Then a grant matches only if every dimension matches; table order is the reporting order of `NeedReason::Mismatch`.

| # | Request-level check | Outcome |
|---|---|---|
| R1 | any `RequestedData::RawSecretValues` | Refused `RawSecretRequested` (rule 16, under any grant) |
| R2 | network capability (all except `local_render`, `app_storage`) with no destination / local capability with destinations | Refused `MissingDestination` / `UnexpectedDestination` |
| R3 | `workflow_revision` lifetime without workflow binding; `run` lifetime without `run_id` | Refused `LifetimeBinding` (rule 12) |
| R4 | workflow binding but `current_workflow_revision` is `None` / differs from the binding | Refused `UnknownWorkflow` / `StaleWorkflowRevision` |
| R5 | `model_transmission` without `inference_scope`, or another capability with one | Refused `InferenceScope` (D18-10) |
| R6 | argument errors (duplicate names, bad names) | Refused `InvalidRequest` |
| R7 | a `denied` record with equal capability, fingerprint, workflow binding and run | Refused `PreviouslyDenied(id)` |
| R8 | a `pending` record with equal capability, fingerprint, workflow binding, run and lifetime | NeedsApproval `AlreadyPending(id)` |

| # | Grant dimension | Matches when |
|---|---|---|
| G1 Status | `approved` (pending, denied, cancelled, revoked, expired never authorise — rule 13) |
| G2 Capability | equal |
| G3 Fingerprint | equal to `fingerprint(request)` |
| G4 Workflow | `workflow_id` equal (both `null` allowed only for `once`/`run`) |
| G5 WorkflowRevision | grant revision = request revision = current saved revision (rule 14: a stale grant never matches even while listed) |
| G6 Run | `run` grant: `run_id` equal. `once`: equal when the grant has one |
| G7 Repos | equal as sets |
| G8 Destinations | equal as sets of canonical forms (no wildcard can exist: C6, A-08) |
| G9 ResourceConstraints | equal as sets |
| G10 DataCategories | equal as sets |
| G11 InferenceScope | equal (`PartialEq`) |
| G12 Lifetime | grant lifetime compatible: `once`/`run`/`workflow_revision` per G4–G6 |
| G13 Expiry | `expires_at` is `null` or later than `now` |
| G14 Consumption | `once` grant not `consumed` |

G7–G10 are also covered by G3. They are checked explicitly as defence in depth and to give a precise reason. Several matching grants → prefer `workflow_revision`, then `run`, then `once` (non-consuming first), then the earliest `requested_at`, then `approval_id`. If nothing matches, the reason is the first failing dimension of the approved same-capability record that fails latest in the table order (ties: store order), else `NoGrant`.

**Destination normalisation (C6):**
- Schemes `https` (default port 443) and `ssh` (22) only; the scheme is required.
- Host: ASCII only, lower-cased, exactly one trailing dot removed.
- Rejected: empty labels, labels over 63 bytes, a host over 253 bytes, `*`, `xn--` labels, any non-ASCII, userinfo (`@`), query/fragment.
- IP literals: only canonical dotted-quad IPv4 (no leading zeros, exactly four parts). IPv6 and the shortcut forms (`127.1`, `0x7f.0.0.1`, `2130706433`) are rejected.
- Port: 1–65 535; the default port becomes explicit.
- Resource path: optional, `/`-prefixed, the P02 `url_path` rules.
- Store form: `https://host:port[/path]`, or `host:port` for ssh. The store `Destination` grammar has no `ssh://`, so the fingerprint uses the full canonical form.

## 7. Child-environment policy (T06)
Every profile is built from an empty base. `PATH=/usr/bin:/bin` and `LANG=C.UTF-8` are set for all profiles. App dirs come from `ToolDirs`.

| Profile | App dirs | Fixed | Inherited |
|---|---|---|---|
| git | `HOME`, `XDG_CONFIG_HOME` | `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`, `GIT_ATTR_NOSYSTEM=1` | none |
| yamllint | `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `TMPDIR` | — | none |
| gitleaks | `HOME`, `TMPDIR` | — | none |
| checkov | `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `TMPDIR` | `CKV_SKIP_PACKAGE_UPDATE_CHECK=True` | none (rev 2 "exactly") |
| actionlint | `HOME`, `XDG_CACHE_HOME`, `TMPDIR` | — | none |
| kubeconform | `HOME`, `XDG_CACHE_HOME`, `TMPDIR` | — | none |
| internal | `HOME`, `TMPDIR` | — | `TZ` |

Deny rules apply to the final environment whatever the profile says. If a profile would produce a denied variable, `build` fails closed (`EnvError`) rather than spawning with it.

**Denied names** (ASCII case-insensitive):
- containing `KEY`, `TOKEN`, `SECRET`, `PASSW`, `CREDENTIAL`, `AUTH`, `COOKIE` or `SESSION`;
- prefixed `OPENROUTER_`, `AWS_`, `AZURE_`, `GOOGLE_`, `GCP_`, `GITHUB_`, `GH_`, `GITLAB_`, `CI_JOB_`, `BC_`, `PRISMA_`, `SSH_`, `GPG_`, `DOCKER_`, `NPM_`, `PYPI_`;
- any `*_PROXY`/`*_proxy` name;
- `GIT_ASKPASS`, `SSH_ASKPASS`, `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_*`/`GIT_CONFIG_VALUE_*`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `PYTHONPATH`, `PYTHONSTARTUP`.

**Inherited entries** are additionally dropped if:
- the name is not valid UTF-8 or contains `=`;
- the name repeats (first occurrence wins, and only if it passes);
- `probe.appears_in(value)` is true (the session key anywhere in the value);
- the value contains NUL.

Dropped entries are not reported by value. **Preview:** name and origin only, never a value.

## 8. Test plan (minimum per task; every new assertion must be able to fail, and the report names what would make it fail)
- **T05.** Every R and G row positive and negative. Destination normalisation table (≥ 30 cases: case, trailing dot, default ports, punycode, IDN, userinfo, IPv4 shortcuts, IPv6, wildcard, ports 0/65536). Fingerprint: ≥ 6 golden vectors (hex pinned). Equal requests with permuted lists → equal digest. Each F-field change → different digest. Excluded fields do not change it.
- **T06.** Each profile's exact variable set. A seeded host env (≥ 20 key-like names, multi-line and `=` values, duplicates, non-UTF-8 names and values, the session key under an innocent name and embedded in a longer value) gives no seeded value in any profile. `TZ` holding the key is dropped. A `ToolDirs` path containing the key → `SecretValue`. The preview contains no value. A unit-only test profile proves deny beats allow.
- **T07.** Statuses, set/replace/delete, `read_key` table (empty, 15/16/4096/4097 bytes, CRLF, inner space, non-ASCII, TTY), clearing on every rejection path, `Debug` of the store and secret without key or length, an event without key material, compile_fail guards (store not `Clone`/`Serialize`), and `RawInput` `Debug` without length.
- **T08.** Every lifecycle transition and refusal. A once-grant is consumed exactly once. Fault injection at every `WriteStep` during consumption is followed by reopen and re-authorise, and authorisation is never received twice. A pending request is never a grant. A stale revision asks again. Events are schema-valid. Semantic registry `rest_in` texts for D11-12/13/16/17 are narrowed with explicit count arithmetic.
- **T09.** `parse_status_euid` table. With a fake euid 0: a foreign-owned dir is refused (`RootForeignDataDir`), a root-owned dir is accepted. With non-zero euid the behaviour is unchanged. A euid read failure → `PathRejected`. CLI exit 5 with the new message.
- **T10.** Feature-index rows and a staleness test. Scenarios cover every §5.7 command. The key never appears in stdout, stderr, argv or any store/log file (byte sweep). `credential status` in a fresh `Session` reports `absent`. `dev permission-check` gives 6 → approve → 0 → 6 again for a once-grant, and 6 after a revision change.

## 9. Debug bounds (C10 = 10a)
Pre-authorised debug cycles, two attempts per defect, then the owner decides. Cycles are recorded in `evidence/debug/P03-Dnn/`.

## 10. Packets and workers
- **T04b** (this approval): worker W1 writes `dev/p03_check.py` → `planning/P03-packets/T04b-W1.md`.
- **Waves 1–3:** packets are written when the owner approves each wave, with the ownership in §3. Wave 1 = T05 (W1) ∥ T06 (W2) ∥ T07 (W3).

## 11. Lead decisions (inside C1–C10; owner may override)
| ID | Decision |
|---|---|
| T01-D1 | Fingerprint = operation content only; context matched as dimensions (§5.3, §6) |
| T01-D2 | No CA-05: credential statuses map onto the existing contract enum; domain gains constructors only (`FixedText`, `Destination::to_redacted`), no serialised contract change (T03) |
| T01-D3 | FR-33 via `/proc/self/status` through a provided `FsOps` method; fail closed |
| T01-D4 | Scanner profiles inherit nothing; `internal` inherits `TZ` only; deny rules fail closed on profile output |
| T01-D5 | Host environment read once, in `cli/src/main.rs`; `HostEnv` is an explicit input |
| T01-D6 | `dev permission-check` (stdin JSON) is the CLI path for "asks again"; exit 6 used |
| T01-D7 | CLI `Session` seam so tests can run set/status/delete in one process (C4) |
| T01-D8 | `approve_once` on a `run`-lifetime request grants `run`; otherwise `once` |
| T01-D9 | One worker in T01–T04 (runner only); everything else in T01–T04 is lead work |
