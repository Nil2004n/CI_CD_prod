# P04-T01 request — prepared for owner review (2026-11-03)

This is a proposal, not an implemented or adopted contract. Current authorization covers T01 only; T02 onward, product changes, installs, new dependencies, network, commits and pushes are not authorized by this packet. Historical phase approvals do not override this limit. Base `c1832672f12aa41129d882f4d58dc06331df490e`. The lead owns all shared interfaces, integration and verification. Procedures and credentials are specified in the companion `planning/P04-T01-git-safety.md`; where this proposal and that document conflict, stop for lead resolution. Lead local Python probes recorded 3 pass / 0 fail / 0 inconclusive in `evidence/P04/T01/local-probes.json`: cooperative timeout group/control and SSH passwd-home expansion only. The standalone Rust process-group probe also passed both cases under Rust 1.99.1 (`evidence/P04/T01/rust-process-groups.json`). The synthetic SSH policy configuration dump passed seven checks (`ssh-policy.json`); full SSH isolation/authentication and Rust 1.97.1 remain unverified. No real network was tested. Real HTTPS/SSH remains separately approved and manual-pending.

## 1. Adoption gates

1. `Repository` requires `default_ref_kind` and `default_ref_value` at registration; `AddRepository` carries only a URL. No default-branch discovery is authorized before a grant, and a guessed `main` is wrong. `GitRefName::parse("HEAD")` succeeds, but `RefKind::Branch` denotes a branch, not a symbolic remote default: do not use HEAD as a fabricated branch sentinel. Select an owner decision below before T04. `Repository` has ref, OID and time, but no Git version; existing persisted `Event` contains only fixed message/code and IDs, not structured provenance. No existing log field is an honest durable Git version record.
2. `FixedText` has only four variants, none for Git purposes or repository reasons. `RedactedText` is not constructible from arbitrary strings; `redact` requires a `Redactor`. `sanitise` alone does not redact. `display_name` is also `RedactedText`. No identity redactor or repurposed developer-check purpose is acceptable. Lead T04 adds only fixed application-authored variants `PurposeGitAcquire`, `PurposeGitUpdate`, `RepositoryDisplay`, `RepositoryDirty`, `RepositoryInProgress`, `RepositoryDiverged`, `RepositoryUnsafe`, and `RepositoryFailed` to `FixedText` and its `ALL`, `as_str`, and `redacted` mappings. Their literal messages contain no user text; use `RepositoryDisplay` ("Repository") for every display name until a real redactor exists. This changes no serialized schema. No identity redactor is permitted. Do not leak URL, child output, helper output, or repository text into these fields. Existing `RemoteUrl::parse`, `CanonicalRemote::parse`, `Slug::parse`, `GitRefName::parse`, `GitOid::parse`, `AppRelPath::parse`, `RawInput::as_str`, `sanitise`, and `redact` exist; proposed methods below do not yet exist.
3. P03 `NormalisedDestination` rejects SSH userinfo and has a different store form; canonical destination and full remote fingerprint must be constructed separately. The proposed `host/path` key omits scheme, user and port (`CanonicalRemote::parse` validates that form but does not derive it), so duplicates across those forms collide by design; lead accepts this existing schema behavior for P04; changing it requires an amendment. Hostname rejection cannot guarantee DNS resolution stays public or prevent redirects to private addresses; E13 is lexical only unless a separately designed enforcement mechanism is adopted. Record this limitation, do not claim SSRF protection.
4. `persistence::lock::acquire` is specific to `.app.lock`; a per-repository lock is a proposed primitive, not an existing reusable public function. Path-based validation is not race-free against same-user adversaries. The lead must resolve safe deletion and mount detection using available std/FsOps without following links; fail closed if proof cannot be established. Cross-device mount checks alone do not detect same-device bind mounts.
5. E5 feature isolation needs proof: Cargo features unify across dependency graphs; a self dev-dependency does not by itself prove release unreachability. A release-path compile/static check and negative controls are required; if not provable, block real fixture transport or revise the design with owner approval.

## 2. Proposed modules and interfaces for lead adoption

Signatures below are proposed seams for lead adoption, not present Rust APIs. Names are illustrative only where marked; all errors are fieldless or contain only safe IDs/status, and never retain raw strings, paths, argv, stderr or helper output. Avoid deriving `Debug` on process/remote structures: mask values and paths explicitly, including secret length.

```rust
// repositories::remote (domain is an existing dependency only after T02)
pub enum RemoteScheme { Https, Ssh }
pub struct DestinationParts<'a> { pub scheme: RemoteScheme, pub host: &'a str, pub port: u16, pub path: &'a str }
pub struct RemoteSpec { /* private validated remote, canonical, destination components */ }
pub enum RemoteError { Empty, TooLong, Syntax, Scheme, Credentials, Host, InternalHost, Path, Unsupported }
impl RemoteSpec {
    pub fn parse(input: &str) -> Result<Self, RemoteError>;
    pub fn remote_url(&self) -> &RemoteUrl;
    pub fn canonical_remote(&self) -> &CanonicalRemote;
    pub fn slug(&self) -> &Slug;
    pub fn destination_parts(&self) -> DestinationParts<'_>; // scheme, host, port, path; no credentials
}
// application converts destination parts using existing NormalisedDestination::parse.
// No repositories -> application dependency.

// repositories::git::runner
pub const STDIN_CAP: usize = 8 * 1024 * 1024;
pub const STDOUT_CAP: usize = 8 * 1024 * 1024;
pub const STDERR_CAP: usize = 64 * 1024;
pub struct GitStdin { /* private variant and bytes */ }
impl GitStdin {
    pub fn null() -> Self;
    pub fn bytes(bytes: Vec<u8>) -> Result<Self, GitError>; // rejects over STDIN_CAP
}
pub const MAX_PATHS: usize = 100_000;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_HELPERS: usize = 64;
pub const MAX_HELPER_BYTES: usize = 4096;
pub const OP_TIMEOUT_SECS: u64 = 120;
pub const KILL_GRACE_SECS: u64 = 2;
pub const VERSION_TIMEOUT_SECS: u64 = 5;
pub struct GitEnv { /* private validated OsString pairs, masked Debug */ }
impl GitEnv { pub fn from_approved_pairs(pairs: Vec<(OsString, OsString)>) -> Result<Self, GitError>; }
pub struct GitInvocation { /* private argv, cwd, approved environment, GitStdin, timeout */ }
pub struct GitOutput { /* private status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8> */ }
impl GitOutput {
    pub fn status(&self) -> &std::process::ExitStatus;
    pub fn stdout(&self) -> &[u8];
    pub fn stderr(&self) -> &[u8]; // internal classification only; never displayed or persisted
}
pub enum GitError { InvalidInvocation, UnsupportedVersion, Spawn, Timeout, OutputLimit, Exit, Io }
pub trait GitRunner { fn run(&mut self, call: GitInvocation) -> Result<GitOutput, GitError>; }
pub struct ProcessGitRunner { /* pinned paths, budgets, policy */ }
impl ProcessGitRunner { pub fn new(git: PathBuf, timeout: PathBuf, env: GitEnv) -> Result<Self, GitError>; }
// Invocation constructors are crate-visible only to HardenedGit and validate argv/cwd/stdin.
// A feature-gated test-support constructor of GitOutput validates stdout/stderr caps and accepts
// synthetic status; it is unavailable in production builds. All Debug output is masked.

// repositories::git::ops
pub enum TargetRef { Branch(GitRefName), Tag(GitRefName), Commit(GitOid) }
pub struct Provenance { pub oid: GitOid, pub target: TargetRef, pub git_version: String, pub observed_at: UtcTimestamp }
pub enum GitRefusal { Dirty, UntrackedWouldOverwrite, InProgress, Diverged, MissingRef, InvalidRef, UnsafeConfig, UnsafeAttributes, UnsafePath, Unsupported }
pub enum GitOutcome { Completed(Provenance), Skipped(GitRefusal) }
pub trait HardenedGit {
    fn clone_verified(&mut self, remote: &RemoteSpec, target: &TargetRef, checkout: &Path) -> Result<GitOutcome, GitError>;
    fn update_verified(&mut self, remote: &RemoteSpec, target: &TargetRef, prior_oid: &GitOid, checkout: &Path) -> Result<GitOutcome, GitError>;
}
// repositories::lock
pub struct CheckoutLock { /* private held OS handle; releases on drop */ }
pub enum LockError { Locked, UnsafePath, Io }
pub fn acquire_checkout_lock(data_dir: &Path, repo_id: RepoId) -> Result<CheckoutLock, LockError>;
// standard File::try_lock on checked checkouts/.locks/<id>.lock; held File in CheckoutLock
```

The concrete service is in `application::repository_service`; it owns the approval and persistence boundary, not Git. Proposed public operations are `add(&mut self, input: RawInput) -> Result<RepoId, RepositoryError>`, `list(&mut self) -> Result<Vec<Repository>, RepositoryError>`, `status(&mut self, id: RepoId) -> Result<Repository, RepositoryError>`, `refresh(&mut self, id: RepoId) -> Result<RefreshOutcome, RepositoryError>`, and `remove(&mut self, id: RepoId, options: RemoveOptions) -> Result<(), RepositoryError>`. `RefreshOutcome` is `Completed`, `NeedsApproval(ApprovalId)`, or `Skipped(RefreshReason)`; `RepositoryError` distinguishes locked, storage, missing, duplicate, invalid, unsafe and operation failure without input. The following are proposed lead-owned T04 seams (types are new, not existing APIs); no worker freeze is claimed while owner choices remain open:

```rust
pub struct RemoveOptions { pub delete_checkout: bool, pub purge_derived_index: bool, pub keep_reports: bool }
pub struct RepositoryService<'a, C: Clock, I: IdSource, G: HardenedGit, L: CheckoutAccess> {
    /* private writer, emitter, clock, ids, git and checkout access references */
}
impl<'a, C: Clock, I: IdSource, G: HardenedGit, L: CheckoutAccess> RepositoryService<'a, C, I, G, L> {
    pub fn new(writer: &'a mut StoreWriter, emitter: &'a mut Emitter<C, I>,
        clock: &'a dyn Clock, ids: &'a mut dyn IdSource, git: &'a mut G,
        checkouts: &'a mut L) -> Self;
}
pub trait CheckoutAccess {
    type Guard; // owns File lock for full operation; no borrowed handle escape
    fn lock(&mut self, id: RepoId) -> Result<Self::Guard, LockError>;
    fn checkout_path(&self, id: RepoId, guard: &Self::Guard) -> Result<PathBuf, RepositoryError>;
    fn delete(&mut self, id: RepoId, guard: &Self::Guard) -> Result<(), RepositoryError>;
}
```

The checkout adapter is constructed by the application with its validated `DataDir`; `checkout_path` returns only the confined app checkout, after verifying that the guard binds the exact requested `RepoId`; `delete` performs the same binding check. A guard for another repository is rejected. Application builds `GitEnv` from P03 `ChildEnv::vars()` plus the separately approved E3 exception after validation; `repositories` sees only neutral OS-string pairs and has no dependency on `application`. Construction must reject duplicate, NUL-containing, unauthorized names/values, and disallowed SSH exceptions; masked Debug and no inherited environment. The test-only `GitOutput` constructor must remain available to integration-test fakes only, with feature unification/release isolation independently proven. If the conditional initial-ref owner decision selects explicit refs, `add` gains a validated `TargetRef` parameter and CLI requires the flags; if optional stored refs are approved, use the amended type and schema instead. Durable Git version storage is likewise conditional on the owner decision below.

T07 owns `repositories/src/lock.rs` and `application/src/repository_registry.rs`; the lead defines the registry seam at T04: `read(id)`, `list()`, `insert(repository)`, `replace(repository)`, `remove(id)` each take `&mut StoreWriter` explicitly and return a typed result; writes use `AnyStore::Repositories` and emit through a separately passed emitter only after persistence. T09 owns only `application/src/repository_service.rs`. The service depends on an open `StoreWriter`, `Emitter<C,I>`, clock, ID source, hardened Git and lock/deletion adapters; construct `ApprovalService<'_, C, I>::new(&mut writer, &mut emitter, &dyn Clock, &mut dyn IdSource)` in a limited borrow scope, not a second mutable writer. `StoreWriter::read`/`replace` operate on `AnyStore`, not an invented repository writer. The Rust trait and crate visibility are not an authorisation security boundary. Enforce production call sites and static checks so user operations reach Git only through the authorising repository service and the HardenedGit implementation; no direct runner or Git invocation from other production call sites. Tests prove pending and refusal never run Git. Hold `.app.lock` then checkout lock consistently, including deletion; never acquire in reverse order.


Proposed ownership layout (T04 creates shared stubs; later files are subject to separate authorization):

```text
workbench/crates/
  repositories/src/
    lib.rs                 # lead module exports
    remote.rs              # T05
    git/mod.rs             # lead scaffold
    git/runner.rs          # T06
    git/ops.rs             # T08
    lock.rs                # T07
  application/src/
    repository_registry.rs # T07
    repository_service.rs  # T09
  cli/src/{args,commands,output,features}.rs # T10
  repositories/tests/p04_{remote,runner,ops}.rs # respective workers
  application/tests/p04_registry.rs # T07
  application/tests/p04_service.rs # T09
  cli/tests/p04_*.rs       # T10
```

## 3. Intake and identity

Bound original input to 2048 bytes, ASCII graphic only; reject leading `-`, controls, whitespace, non-ASCII, query/fragment, all percent escapes, backslash, empty path/segments, `.`/`..`, and `@` or `:` within path segments; never echo input. Accept only `https://host[:port]/path`, `ssh://[user@]host[:port]/path`, and `user@host:path`, subject to existing `RemoteUrl::parse`; SSH user is a conservative ASCII account name, no colon/password. Reject HTTPS userinfo, local paths, `file`, `git`, `ext`, unsupported schemes, IPv6, wildcard, malformed/shortcut IPv4, IDN/punycode and option-like ref/path forms. A syntactically plausible secret in an otherwise valid path cannot be reliably identified. The accepted grammar does not guarantee secret detection; do not claim every accepted path is credential-free. Users must not include secrets in repository paths, and no URL is echoed or logged.

Lowercase DNS host, remove one trailing dot, validate labels and ports 1–65535, make default 443/22 explicit in approval destination; retain a normalized safe remote compatible with store grammar. Reject `localhost` and subdomains, loopback 127/8, RFC1918 10/8, 172.16/12, 192.168/16, link-local 169.254/16, unspecified 0/8 and multicast/reserved IPv4; test ranges and boundaries. No DNS lookup in T01. Convert scp form to SSH destination; preserve SSH user separately in full operation identity. Canonical duplicate key is lowercase `host/path` as enforced by `CanonicalRemote`, with path case preserved and no scheme/user/port; don't silently strip `.git` or trailing path bytes. Reject a collision at add before any write. Derive a bounded deterministic lowercase `Slug` from the last path component, parse it and fail if empty; display name uses the fixed application-authored value in §1.

## 4. Approval, state and refs

Acquire lock and re-read registry under lock. Validate remote, target and local safety before authorisation, then form a fresh `PermissionRequest`: capability `GitAcquire`/`GitUpdate`, operation `git.clone`/`git.update` via `OperationName::parse`; arguments with unique `ArgName::parse` names `remote` (full syntactically validated remote including SSH user, port and path), `ref_kind`, `ref_value`; `repo_ids = [id]`, destination as above, `data = [RequestedData::Category(DataCategory::RepositoryMetadata)]`, no workflow/run/inference scope, once lifetime, no validity, no resource constraints unless frozen explicitly. Only values validated under the stated grammar are used; that grammar cannot detect all secrets in paths, and the fingerprint is unsalted. Never intentionally put a secret into it. Include ref and remote in fingerprint so changes ask again; ssh path is bound even though the event/store destination drops it. Purpose uses the corresponding fixed `PurposeGitAcquire` or `PurposeGitUpdate` variant from §1. `ApprovalService::authorise(&request, None)` is the only source of `Authorisation`; pending yields ID and no Git; refusal yields skipped and no Git. A once grant is durably consumed before proof is returned; failure to persist means no Git, and a later failure must not restore it. `request()` returning `Permitted` is not proof. Never accept repository text as a decision.

After proof, write `updating` and emit started before Git; on success persist ready and OID/time and, subject to the owner decision, durable Git-version provenance, then emit succeeded; on safety refusal persist dirty where appropriate and emit skipped; on failure persist error with only fixed redacted reason, then emit failed. A failed store/event write halts progress; state already persisted is not rolled back, and recovery of interrupted `updating` must recheck rather than assume success. Pending/refusal does not mutate checkout. Events use existing `RepositoryRegistered`, `RepositoryStatusChanged { repo_id, operation, status, reason }`, `RepositoryRemoved`; persist first, append event through existing emitter/log path, report a subsequent failure honestly. `RepositoryFile` inventory starts empty/revision 0. No multi-repository operation should stop other independent entries on one refusal.

Validate branch and tag names more narrowly than `GitRefName::parse` as needed; fully qualify `refs/heads/` and `refs/tags/`, use argument boundaries and resolve to a commit OID, never evaluate revision expressions supplied by a user. Commit targets must parse as full 40- or 64-hex OIDs and resolve to a commit; never let an abbreviated or option-like value through. Managed checkouts are deliberately detached at verified OIDs. Branch update requires prior HEAD equality and fast-forward ancestry, then moves detached HEAD to the verified OID; tag/commit are detached verified targets, never merge a tag name directly. Update takes the prior persisted OID: tag requires it and refuses a retarget, and commit requires target equality with it. An initially verified tag is pinned in `last_commit_oid`; refuse a retargeted tag. Empty trees are refused under the P00 default. Missing/empty remote/default ref is an explicit refusal. See companion for every execution and abort point; no checkout or merge until attributes, paths and target have been verified, then check resulting HEAD exactly. Git version recorded only after verified gate.

## 5. Removal and runner constraints

Deregister alone preserves checkout and reports. `--purge-index` is accepted as a documented no-op until an index exists; `--keep-reports` is the default. With delete selected, hold both locks and verify the exact app-owned `checkouts/<uuid>` path and every component by no-follow inspection; reject symlinks, mount points, unexpected type, escape, or inability to prove confinement. Never recursively follow links. Verify identity at deletion and refuse on races; if the platform cannot meet this requirement with available primitives, refuse deletion rather than claim safety. Delete before registry removal, report partial failures explicitly; no event says removed before successful store change. Reports remain intact.

Process runner uses only a pinned absolute Git and approved timeout executable, `Command` argv arrays, fixed cwd, `env_clear`, approved child environment, null stdin by default or bounded byte input for `check-attr --stdin -z` and other explicitly enumerated machine-readable operations, concurrent stdin feeding and capped output drains, wall-clock timeout, and process group; never shell evaluation, raw output in errors, progress or prompts. Git 2.53.x only until separately qualified. `timeout`/process-group behavior, output limits and credential helper policy must follow companion and local probe evidence; `setsid` descendants remain a known limitation. No repository-local config, hooks, filters, URL rewrite, fsmonitor, submodule recursion or untrusted helper may override hardening; inability to prove controls means refusal. HTTPS helper and SSH agent exceptions must be exact and documented, with no proxy inheritance. No network in this task.

## 6. CLI contract proposal

Use existing CLI `Output` JSON Lines and sanitized human lines; no raw input, URL credentials, Git stderr or helper data. Global `--data-dir D` and `--json` follow existing parser conventions. Each success emits one JSON object per record, no secrets; errors emit safe fixed code/message. Ref selection needs the registration gate in §1 before implementation.

| Command | Success JSON fields | Exit |
|---|---|---|
| `repo add <url>` | `type=repository_added`, `repo_id` | 0; 2 usage, 3 lock, 4 storage, 5 invalid/duplicate |
| `repo list` | each `type=repository`, `repo_id`, `display_name`, `state` | 0; 3/4/5 |
| `repo status <id>` | `type=repository`, `repo_id`, `display_name`, `state`, `last_commit_oid`, `last_update_at` | 0; 2/3/4/5 |
| `repo refresh <id>` | `type=repository_refresh`, `repo_id`, `status`, `reason` or `approval_id` | 0 success; 2/3/4/5 refusal/failure; 6 approval needed |
| `repo remove <id> [--delete-checkout] [--purge-index] [--keep-reports]` | `type=repository_removed`, `repo_id`, `checkout_deleted`, `index_purged=false`, `reports_kept=true` | 0; 2/3/4/5 |

JSON optional fields are null when unavailable, IDs and status strings are stable, and no arbitrary repository text appears in reasons. A stdout failure uses existing CLI failure behavior. Add feature-index rows and staleness coverage.

## 7. Future test matrix and execution ownership

These are future tests, not evidence of T01 implementation. T05: ≥50 accepted/rejected URL cases, boundary private ranges, malformed encodings, duplicate canonical collisions, no echo, no invented DNS protection. T06: argv/cwd/env exactness, seed-key sweep, version gate, bounded concurrent stdout/stderr and timeout/grandchild controls, spawn failure and no output leakage. T07: lock contention and release, confinement/symlink and mount refusal, race/fault cases, registry duplicate/state/event ordering and deletion failure. T08: P00 G01–G15 with potency controls; local fixture-only transport, hostile config/includes/filters/hooks/LFS/submodules/fsmonitor/textconv/macros, newline paths and external symlinks, SHA-1/SHA-256, missing refs, injection and HEAD verification; dirty/diverged/untracked/in-progress unchanged before/after byte and HEAD digests. T09: permission 6 → approval → success, once consumption and write-fault injection, changed remote/ref re-prompts, denial never invokes fake Git, all state/event failures and cross-repository independence. T10: every CLI row, JSON parse and escaping, lock/store errors, retention options and canary/secret sweep across output/store/events. Lead runs integrated checks and security review, verifies non-test fixture isolation and records exact revision, commands, environment, exits and counts. Real remote behavior remains manual-pending until separately authorized.

Proposed sequence after further approval: lead T02 dependency/lock and T03 contract checks, T04 shared scaffolding; W1 T05 remote, W2 T06 runner, W3 T07 registry/locks on disjoint files; W1 T08 hardened operations and W2 T09 application service against frozen fake seam; W3 T10 CLI after T09; lead T11 integration/review, T12 report. Workers get explicit worktrees, base and owned files; no self-approval. No new crates: proposed manifest edges `repositories -> domain` and `application -> repositories`, plus E5 dev-only feature only if isolation is demonstrated; offline resolution and registry-set/lock-edge verification at T02. Existing baseline floors 1138 tests/41 compile-fail cases must not be lowered. Debug bounds from E10 are historical planning choices only: current T01 approval does not authorize future debugging; require applicable owner authorization, at most two attempts per defect then stop for owner decision. Never claim implementation or test success from this draft.

### Further integration decisions required

The checkout-lock implementation uses `std::fs::File::try_lock` with checked private lock-file handling through the application-owned port; `repositories` must not acquire an undeclared persistence dependency. The lead must freeze that seam and its testable filesystem checks. Holding the checkout lock spans local checks, fetch, target verification, worktree change and final registry writes. Update fetch itself changes Git metadata even when a later divergence check refuses the worktree change: preserve and compare worktree bytes and HEAD, and explicitly disclose fetch metadata changes; do not promise byte-for-byte preservation of the entire `.git` directory. The lead accepts this P00 worktree-and-HEAD boundary; disclose metadata mutation in the eventual report. Companion policy must cover isolated global helper extraction, ordered empty resets, includes and refusal on ambiguous extraction. `GitOid::parse` already accepts exactly 40 or 64 lowercase hex characters (source `values.rs`); exercise both in T03 and the future tests. `CanonicalRemote::parse` enforces lowercase `host/path` and forbids scheme, userinfo and port; it does not itself derive the key.

## 8. Historical choices (resolved by owner approval in §10)

The following are lead design choices for review: fixed texts above, fixed display label, precise limits above (cap overflow aborts the operation; no truncated content accepted as complete), standard File lock, explicit stdin, full OID parsing, and separate registry/service ownership. A writer thread feeds bounded byte stdin while stdout/stderr are drained concurrently; close stdin after bytes, cap each stream independently, terminate on overflow and do not return truncated success. Bytes are NUL-delimited path lists for `check-attr`, never interactive input. No child output, including accessors above, may escape the internal parser/classifier. Common caps are 8 MiB stdin and stdout, 64 KiB stderr, 100,000 paths, 4,096 bytes per path, 64 helpers of at most 4,096 bytes each; operation timeout 120 seconds, kill grace 2 seconds, version probe 5 seconds. The byte caps also bound aggregate path input. `GitOutput` and invocation use custom masked Debug and are not serializable.

| Decision needing owner | Concrete options | Impact |
|---|---|---|
| Initial reference | A: require `repo add <url> --ref-kind branch|tag|commit --ref <value>` and reject missing flags until a later remote-default feature; B: approve CA-06 making default ref optional with an explicit unresolved state, then discover remote HEAD only after authorisation. | A changes the planned URL-only CLI; B changes the store contract. `HEAD` is not a branch name merely because the parser permits it. |
| Durable Git-version provenance | A: approve CA-06 adding nullable `last_git_version` to Repository and updating schema/version, typed model and compatibility tests; B: revise acceptance to retain version only in operation result and explicitly mark durable version pending. | Existing fixed event message and code cannot encode this as structured provenance. |
| Safe deletion on this platform | Accept a demonstrated no-follow/mount implementation with stated same-user race limits, or defer deletion; refuse where proof is unavailable. | `File::try_lock` and path metadata checks alone are not race-free and `st_dev` misses same-device bind mounts. |

The lock adapter verifies real private parents, lock file regularity/owner/single link, uses `OpenOptions` with platform no-follow flags where safely exposed without unsafe, verifies opened identity against metadata, then nonblocking `File::try_lock`; `Locked` is distinct from I/O and unsafe path. This is path-based and cannot defend all concurrent same-user substitutions. Consult `/proc/self/mountinfo` without following checkout symlinks, decode mount escapes, reject matching mount or descendant mounts, and fail closed on unreadable/ambiguous mountinfo; this still does not remove race limitations. Hold guard through Git and final state/event handling. These are proposed Linux pilot constraints, not completed probes. Worker packet templates are unapproved and do not start implementation. The lead has reviewed and integrated this request; it awaits owner acceptance. The initial-ref and durable-provenance decisions remain conditional, so the affected interfaces are not frozen and no implementation packet may be dispatched yet.

## 9. Resolution proposed before owner approval in §10

The lead recommends the reviewable amendment in `planning/P04-T01-CA06-proposal.md`: preserve URL-only registration using an unresolved reference, authorize remote discovery and then the resolved target separately, and persist the last successful Git version. CA-05 and CA-06 versioning must be reconciled before application. This is a proposed contract, not an owner decision or an applied schema change. The affected interfaces remain conditional until approval.

## 10. Owner approval and interface freeze — 2026-11-03

Owner response to the T01 contract-change approval request: “okay i approve”. T01 adopts the proposed initial-ref and durable Git-version behavior in `planning/P04-T01-CA06-proposal.md`, including separate authorization for discovery and the resolved target, and fail-closed deletion with documented same-user race limits. The interface design in this request, safety annex and CA-06 proposal is frozen as the T01 baseline; where earlier conditional language conflicts, this section governs. Exact schema version and migration ordering must be resolved against CA-05 in T03 before applying the amendment; approval here does not itself modify canonical schemas or application code. Product implementation, T02 onward, commits, and pushes require the applicable separate authorization.
