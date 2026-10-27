# P02-T01 — P02 request: module layout, frozen interfaces, policies, test plan, packets

Status: **prepared by Opus 2026-10-27; awaiting owner approval before cargo-audit stage C-A and P02-T02.** This is a documentation deliverable. No crate was resolved, fetched or built, no worker was spawned and nothing was committed. Base: `phase/P02-store` at `f454f92` (= `origin/phase/P02-store`), clean tree, no worktrees.

Authority: the owner approved P02 on 2026-10-26 with A1–A10 = recommended options (`plan.md` §5B.6). On 2026-10-27 the owner asked to start T01 and repeated the P01 working instructions: Sonnet 5.5 workers in their own branches/worktrees; lead-run debug cycles after worker tasks; **stop after T01 for approval**; recommended options approved. T01 is lead-only because it defines shared contracts (CLAUDE.md "Opus owns shared contracts"). The decisions in §11 are new, so they are presented for approval rather than treated as pre-approved.

Appendix A: `planning/P02-T01-fr31-classification.md` (FR-31 draft classification, generated from the schema).

## 1. Facts checked read-only in this session (2026-10-27)

| Fact | Basis |
|---|---|
| Canonical schema sha256 `909e2a55c315ca07096f9123e6986b637a83b1bcdfc4eb6ff88d9d06daa80e2e`; Draft 2020-12; no `$id`; no external `$ref`; formats used: `uuid`, `date-time` only | `schemas/workbench-store-v1.4.schema.json` |
| 10 roots in `oneOf`, each with `schema_version` const `"1.4"` and a distinct `store_type` const | same |
| **Only 57 of the 63 `$defs` are reachable from the store roots.** WrapperRequest, WrapperControl, TerminationRecord, StreamRecord, DescendantCleanup and TerminationPolicy belong to the wrapper protocol only. `Check` does not reference any of them | reachability walk over `$ref`. `plan.md` §5B.1/T06 wording ("63 `$defs`", "WrapperControl subset used by Check") is therefore inaccurate → T01-D2 |
| Of the 57: 12 are value/vocabulary defs (Id, Digest, GitOid, RelativePath, Timestamp, Assessment, Capability, Category, Coverage, Severity, Execution, Outcome), 45 are object defs | same |
| 104 plain-string properties in the reachable defs (113 including the wrapper defs) | Appendix A script |
| Domain crate already provides IDs (17 types, no generation), `UtcTimestamp` (incl. `now()`), `SanitisedText`/`RedactedText` (`RedactedText` has no `Deserialize`), vocab and state enums (with `Deserialize`), and the contract enums (`AdapterId`, `ToolAvailability`, `ExportStatus`, … **`Serialize` only**) | `workbench/crates/domain/src/*.rs` |
| `domain::text::redact(raw, &dyn Redactor)` is public, so any caller can obtain a `RedactedText` by passing an identity redactor. FR-31's read-back seal therefore protects the *deserialisation* path only, not `RedactedText` construction in general → disclosed in §6, revisited with the real redactor in P06 | `text.rs:354` |
| `persistence` and `application` are empty crates; no crate depends on another yet | manifests |
| Host `/usr/bin/python3` 3.14.7 with `jsonschema` 4.26.0 is still present (P00 J16 comparator) | read-only version query |
| Workspace lints deny `print_stdout`, `print_stderr`, `exit`, `unwrap_used`, `expect_used`, `panic`; the CLI must therefore write through `std::io::Write` handles and return `std::process::ExitCode` | `workbench/Cargo.toml` |

## 2. Scope as refined by these facts
Unchanged from `plan.md` §5B.1, except:
- Models cover the **45 object defs reachable from the store roots** plus the 12 value/vocabulary defs, which map to existing domain types or the new `store::values` newtypes. The six wrapper-protocol defs are modelled with the wrapper protocol in P06 (T01-D2).
- FR-31 classification covers the **104** plain-string properties of those defs (Appendix A). The wrapper defs' 9 properties are classified in P06.

## 3. Layout (paths relative to `workbench/`)

```text
crates/
  domain/src/store/          # NEW: typed store records (Tech §4: domain = "records, invariants")
    mod.rs                   # lead: module list, StoreKind, version/store_type markers, AnyStore
    values.rs                # validated newtypes (§5.2)                      W1  wave 1a
    plan.rs                  # Selector, RepoTarget, SchemaBinding, RendererSetting, ScanSettings,
                             # FileRecord, TargetSnapshot, EngineSnapshot, PlanSnapshot   W1 1a
    provider.rs              # ProviderPolicy, Provider, InferenceScope       W1  1a
    workflow.rs              # Schedule, Workflow, WorkflowFile               W1  1b
    repository.rs            # Repository, RepositoryFile                     W1  1b
    tools.rs                 # ExitMeaning, LicenseRecord, ToolManifest, ToolManifestFile   W1 1b
    approval.rs              # Approval, ApprovalFile                         W1  1b
    config.rs                # Config, ConfigFile                             W1  1b
    evidence.rs              # SourceRef, Engine, Artifact                    W2  1b
    report.rs                # Check, Finding, CoverageSummary, AIEnrichment, ReportFile   W2 1b
    log.rs                   # Event, EventLevel, EventType, RunSummary, ReportManifest, LogFile   W2 1b
    suppression.rs           # Suppression, SuppressionFile (read-only root) W2  1b
    rag.rs                   # Chunk, RagIndexFile                            W2  1b
    conversation.rs          # ChatContext, Message, Conversation, ConversationFile   W2 1b
    classes.rs               # FR-31 classification table                     W3  wave 3
  persistence/src/
    lib.rs, error.rs, validate.rs          # lead (frozen types §5.4–§5.5)
    paths.rs, fs.rs, fault.rs, lock.rs, atomic.rs   # W3 wave 1a
    read.rs, schema.rs                     # W1 wave 2 (only place store models are deserialised)
    semantic/{mod.rs (lead), rules.rs, ids.rs, workflow.rs, approval.rs, evidence.rs, ai.rs}   # W2 wave 2
    writer.rs, recovery.rs, report_name.rs # W3 wave 2
  persistence/tests/fixtures/store/        # synthetic fixtures + expected-verdict manifest (§9)
  application/src/
    lib.rs                                 # lead
    emit.rs                                # envelope, Clock, IdSource, Emitter, store-event mapping   W1 wave 3
    dev_store.rs                           # `dev` store entry points used by the CLI   W2 wave 3
  cli/                                     # NEW crate `cli`, binary `pwt-workbench-cli`
    src/main.rs, src/lib.rs                # lead (thin main → `cli::run`)
    src/{args.rs, commands.rs, output.rs, features.rs}   # W2 wave 3
    tests/p02_*.rs                         # scripted scenarios   W2 wave 3
dev/
  p02_check.py, p02_baseline.json          # lead (T04); p01 runner stays for history
  p02_differential.py                      # lead (T14): host-Python comparator, FR-16/J16 style
```

Dependency direction (wired in T04): `cli → application → persistence → domain`. The CLI does not depend on `persistence` directly (D-17 "same path as the TUI"). `terminal-ui`, `repositories`, `scanning` and `retrieval` remain empty.

## 4. Dependencies (A2 = 2a, A3 = 3a; executed in T02)

| Crate | Entry |
|---|---|
| workspace | add `jsonschema = { version = "=0.60.1", default-features = false }`, `sha2 = { version = "=0.11.1", default-features = false }` (features fixed in T02 to the minimum that compiles `Sha256::digest`) |
| `persistence` | `domain`, `serde`, `serde_json`, `jsonschema`, `sha2` |
| `application` | `domain`, `persistence`, `serde`, `serde_json` |
| `cli` | `domain`, `application`, `serde`, `serde_json` |
| `domain` | unchanged (`serde`; `serde_json` dev only) |

T02 procedure (as P01-T02): `cargo update --workspace --offline` (lock-preserving; not `generate-lockfile`) in `unshare -rn`. Every new lock entry must equal a `spikes/rust/Cargo.lock` entry by (name, version, source, checksum). Build scripts and proc-macros must have been built in an accepted P00 graph. **Known risk for that rule:** `sha2` 0.11.1 is in the P00 lock but no spike built it; if its graph brings a build script/proc-macro absent from the T05–T08 built graphs (for example a `typenum` or `cpufeatures` build script), T02 stops and reports (A3). No UUID or random crate is added (A2 = 2a): IDs are generated from `/dev/urandom` through std (§5.6).

cargo-audit: stage **C-A** (approved, A8) runs after this approval and before T02, exactly as `planning/P01-T03-cargo-audit-request.md` §2 describes: fetch one `.crate` into an isolated staging `CARGO_HOME`, verify its checksum, read its packaged lockfile, and build nothing. C-A2…C-E still need separate approval; until they are done, criterion 11 records "not run".

## 5. Frozen interfaces
Lead-authored B0 stubs. Signatures, type names, derives, serde representation and visibility below are frozen; workers implement bodies and tests. Changes need lead approval, and owner approval where contract meaning changes. Stub bodies use `todo!()` under `#[expect(clippy::todo, reason = "B0 stub")]`, and `static --final` fails if any remain at T14.

### 5.1 Model conventions (`domain::store`, all workers)
- Every object def becomes a `pub struct` with the schema property names as `pub` fields, in schema order. There are no `#[serde(rename)]` overrides except for Rust keywords (`r#type`) and no `#[serde(default)]`.
- Derives: `Debug, Clone, PartialEq, Serialize, Deserialize`, plus `#[serde(deny_unknown_fields)]` (schema `additionalProperties: false`). `Eq` is derived where all fields allow it.
- Field types:
  - `format: uuid` → the matching `domain::ids` type. If no specific ID type exists, a new ID type is added by the lead (§5.7).
  - `Timestamp` → `UtcTimestamp`.
  - closed enums → domain vocab/state/contract enums, or new `#[serde(rename_all = "snake_case")]` enums in the owning module with exactly the schema values.
  - `Digest`, `GitOid`, `RelativePath` and plain strings → per Appendix A.
  - integers → `u32`/`u64` per the schema bound; booleans → `bool`.
  - `const false` fields → `ConstFalse` (§5.3).
  - nullable (`anyOf [.., null]`) → `Option<T>`, always serialised (as `null`). Required-but-nullable keys are never skipped.
- Bare `String`, `&str`, `Cow<str>`, `Box<str>` and `serde_json::Value` are **forbidden** as field types (FR-31 (3); enforced by the `static` check, §8).
- `RedactedText` fields deserialise only through `#[serde(deserialize_with = "crate::text::store_read::redacted")]` (or `::redacted_opt` / `::redacted_vec`). These are `pub(crate)` functions in `domain::text` that **re-escape on read** (`sanitise`) and wrap the result. `RedactedText` itself keeps no `Deserialize` (the P01 compile_fail guard stays).
- Each root struct `XxxFile` has `schema_version: SchemaVersion` and `store_type: XxxStoreType` (§5.3) as its first two fields.

### 5.2 `domain::store::values` (W1, wave 1a)
- Each newtype is `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]` over a private `String`, with:
  - `pub fn parse(s: &str) -> Result<Self, ValueError>`
  - `pub fn as_str(&self) -> &str`
  - `Display`, `Serialize`, and `Deserialize` through `parse`.
- `ValueError { kind: ValueKind, reason: ValueReason }`, where `ValueReason` is a fieldless enum: `Empty, TooLong, Character, Syntax, Forbidden`. **It never contains input text.**
- **Common rule for every newtype:** reject NUL, C0/C1 controls, U+007F and every code point in the `domain::text` escape set; reject empty input unless listed otherwise; enforce the per-type byte bound below.

| Type | Rule (beyond the common rule) | Max bytes |
|---|---|---|
| `Digest` | `^[a-f0-9]{64}$` | 64 |
| `GitOid` | `^(?:[a-f0-9]{40}\|[a-f0-9]{64})$` | 64 |
| `RelPath` | Data §4 / schema RelativePath. Forward-slash form; reject leading `/`, drive prefix (`X:`), `\`, empty/`.`/`..` segments, trailing `/` | 4096 |
| `AppRelPath` | `RelPath` rules; relative to the app-data root (checkout dir, report files, retained artifacts) | 4096 |
| `LocalPath` | Absolute (`/…`) or `RelPath`-valid; no `..` segment. Used only in Config | 4096 |
| `PathPattern` | Glob subset: segments of `[A-Za-z0-9._ -]`, `*`, `**`, `?` and `[...]` classes; no `{}`, `$`, backtick, `;`, `\|`, `&`, `<`, `>`, `\`; not absolute; no `..` segment | 1024 |
| `GitRefName` | `git check-ref-format` rules for a branch/tag/ref name (no `..`, `@{`, `~^:?*[\`, space, leading `-`, trailing `.`/`/`/`.lock`) | 255 |
| `RemoteUrl` | `https://host[:port]/path` or `ssh://[user@]host[:port]/path` or scp-like `user@host:path`; **no password, query, fragment or `%`-encoded userinfo**; host is DNS name or IP literal | 2048 |
| `CanonicalRemote` | `host/path` lowercase host; no scheme, userinfo, port, query or fragment | 2048 |
| `Slug` | `^[a-z0-9]+(?:-[a-z0-9]+)*$` | 48 |
| `LocalTime` | `HH:MM`, 00–23 and 00–59 | 5 |
| `IanaTimezone` | syntax only: `UTC` or `Area/Location[/Sub]` with segments `[A-Za-z0-9_+-]`; tzdb membership checked in P14 | 64 |
| `Destination` | `host[:port]` or an `https://` URL without userinfo, query or fragment | 2048 |
| Token family (one macro, distinct types): `RedactionVersion`, `VersionLabel`, `ToolName`, `FrameworkId`, `RendererId`, `SchemaResourceId`, `ModelId`, `ProviderId`, `CredentialRef`, `PolicyId`, `SpdxExpression`, `EventCode`, `Identifier` | ASCII `[A-Za-z0-9._:/@+-]`. `SpdxExpression` additionally allows space and `()`. `ToolName`, `FrameworkId`, `EventCode` are restricted to lowercase `[a-z0-9._-]`. `CredentialRef` must not look like a key (rejects any 32+ character run of `[A-Za-z0-9_-]` and the `sk-` prefix; Data §11.23) | 128 (ModelId, SpdxExpression, CredentialRef: 256) |

### 5.3 `domain::store` (lead, `mod.rs`)
- `pub enum StoreKind { Config, Repositories, Workflows, Approvals, Log, Suppressions, RagIndex, Conversations, Report, ToolManifest }`, with:
  - `ALL`
  - `fn store_type(self) -> &'static str`: the schema const;
  - `fn file_name(self) -> Option<&'static str>`: `config.json`, `repo.json`, `workflows.json`, `approvals.json`, `log.json`, `suppressions.json`, `rag/index.json`, `rag/conversations.json`, `tools.json`. `Report` returns `None` because report names are generated (§6.4);
  - `fn writable(self) -> bool`: false only for `Suppressions`;
  - `fn from_store_type(&str) -> Option<Self>`.
- `pub struct SchemaVersion;` serialises as `"1.4"`, and deserialises only from `"1.4"` (`= versions::STORE_SCHEMA_VERSION`).
- Ten marker types `ConfigStoreType` … `ToolManifestStoreType` (macro), each (de)serialising only its own const.
- `pub struct ConstFalse;` serialises as `false` and accepts only `false`.
- `pub enum AnyStore { Config(ConfigFile), …, ToolManifest(ToolManifestFile) }` with `fn kind(&self) -> StoreKind`. It is completed by the lead at B1 once the root types exist, and has no `Deserialize` (decoding is `persistence::read`, keyed by `StoreKind`).

### 5.4 `persistence::validate` and `persistence::error` (lead)
```rust
pub enum Layer { Json, Version, Schema, Typed, Semantic }
pub enum Verdict { Valid, Invalid(Problems), SchemaUnavailable }
pub struct Problems { pub items: Vec<Problem>, pub truncated: bool }      // at most 100 items
pub struct Problem { pub layer: Layer, pub code: ProblemCode, pub pointer: SanitisedText, pub rule: Option<RuleId> }
pub enum ProblemCode { Syntax, TooLarge, NotObject, UnknownStoreType, UnsupportedVersion(VersionClass),
                       StoreTypeMismatch, Keyword(SchemaKeyword), TypedDecode, Rule }
pub enum VersionClass { Older, NewerOrUnknown, Missing }
pub struct RuleId(&'static str);   // e.g. "D11-04", "D18-06a"
pub trait DocumentValidator: Send + Sync + std::fmt::Debug {
    fn validate(&self, kind: StoreKind, doc: &serde_json::Value) -> Verdict;
}
```
- `pointer` is the JSON pointer of the failing instance location, sanitised and cut to 256 bytes. It never includes an instance value.
- `SchemaKeyword` is a fieldless enum mapped from the validator's error kind (`Type`, `Required`, `AdditionalProperties`, `Const`, `Enum`, `Format`, `Pattern`, `Minimum`, `MinLength`, `OneOf`, `AnyOf`, `Other`). Validator error messages are **never** propagated, because they quote instance values.
- `pub enum StoreError { Locked, NotInitialised(StoreKind), Io { step: WriteStep, kind: std::io::ErrorKind }, Invalid(StoreKind, Problems), SchemaUnavailable, Halted, ReportNameCollision, PathRejected, Malformed(StoreKind) }`.
  - `Display` gives fixed English text and never echoes paths or content.
  - The mapping to `AppEvent::StorageFailure`/`OperationFailed` is in `application`.

### 5.5 Files, lock, atomic write (W3, wave 1a)
```rust
pub enum WriteStep { CreateTemp, WriteTemp, SyncTemp, LinkPrevious, ReplacePrevious, Replace, SyncDir, Read, Lock, CreateDir }
pub trait FsOps: Send + Sync + std::fmt::Debug {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>>;          // fails with FileTooLarge past bound
    fn create_new(&self, path: &Path, mode: u32) -> io::Result<()>;              // O_CREAT|O_EXCL
    fn write_all(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn sync_file(&self, path: &Path) -> io::Result<()>;
    fn hard_link(&self, existing: &Path, new: &Path) -> io::Result<()>;          // fails if `new` exists
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>>;
    fn symlink_metadata(&self, path: &Path) -> io::Result<std::fs::Metadata>;
    fn create_dir(&self, path: &Path, mode: u32) -> io::Result<()>;
}
pub struct RealFs;                                       // std only
pub struct FaultFs { inner: RealFs, plan: FaultPlan }    // persistence::fault, public test seam
pub struct FaultPlan { pub step: WriteStep, pub nth: u32, pub error: io::ErrorKind }   // incl. StorageFull, PermissionDenied, Interrupted
pub struct DataDir(PathBuf);                             // absolute, canonicalised, a directory, not a symlink
pub struct StoreLock { /* holds the locked File */ }     // `.app.lock`, File::try_lock (exclusive, non-blocking)
pub fn acquire(dir: &DataDir, fs: &dyn FsOps) -> Result<StoreLock, StoreError>;
pub fn write_replace(fs: &dyn FsOps, dir: &DataDir, rel: &AppRelPath, bytes: &[u8], keep_previous: bool) -> Result<(), StoreError>;
pub fn write_new(fs: &dyn FsOps, dir: &DataDir, rel: &AppRelPath, bytes: &[u8]) -> Result<(), StoreError>;  // never overwrites
```
`RealFs` is the only production code that touches `std::fs` (static check §8). A separate `RealFs` method `open_lock_file` serves `StoreLock`, because `File::try_lock` needs a real `File`.

### 5.6 Writer, read path, report names (wave 2)
```rust
// persistence::read (W1)
pub fn decode(kind: StoreKind, bytes: &[u8], v: &dyn DocumentValidator) -> Result<AnyStore, StoreError>;
pub struct SchemaValidator;          // impl DocumentValidator: embedded schema + version gate + typed decode + semantic
// persistence::writer (W3)
pub struct StoreWriter { /* DataDir, StoreLock, Arc<dyn FsOps>, Arc<dyn DocumentValidator>, halted: bool */ }
impl StoreWriter {
    pub fn open(dir: DataDir, fs: Arc<dyn FsOps>, v: Arc<dyn DocumentValidator>) -> Result<(Self, ReconcileReport), StoreError>;
    pub fn init(dir: DataDir, fs: Arc<dyn FsOps>, v: Arc<dyn DocumentValidator>) -> Result<(Self, InitReport), StoreError>;
    pub fn read(&self, kind: StoreKind) -> Result<AnyStore, StoreError>;
    pub fn replace(&mut self, doc: &AnyStore) -> Result<(), StoreError>;            // validate → write_replace
    pub fn write_report(&mut self, report: &ReportFile, slug: &Slug) -> Result<ReportManifest, StoreError>;
    pub fn restore_previous(&mut self, kind: StoreKind) -> Result<RestoreReport, StoreError>;
    pub fn is_halted(&self) -> bool;
}
// persistence::report_name (W3)
pub fn report_stem(at: UtcTimestamp, slug: &Slug, run: RunId) -> ReportStem;   // YYYYMMDDTHHMMSSZ_<slug>_<run-id>
```
`&mut self` on every mutating method, with the writer owned by the coordinator, gives a single serialised writer without an internal mutex.

### 5.7 Additive domain changes (lead, T04; T01-D4)
- `ids`: every ID type gets `pub fn from_random_bytes(bytes: [u8; 16]) -> Self`, which sets the RFC 9562 version-4 and variant bits; the result can never be nil. Parsing still does not enforce the version.
- `text`: `pub(crate) mod store_read { redacted, redacted_opt, redacted_vec }` (§5.1).
- `contract`: add `Deserialize` to the fieldless enums used by store models (`AdapterId`, `ToolAvailability`, `ExportStatus`, `DataCategory`, `ApprovalLifetime`, and any other enum the models reuse); add `AppEvent::event_type(&self) -> &'static str`. Wire format unchanged.
- `ids`: new ID types only if the models need a UUID-typed field without one (the P01 list was derived from the schema, so none are expected).

### 5.8 Envelope and emission (W1, wave 3; contract effect in CA-04)
```rust
pub trait Clock: Send + Sync + std::fmt::Debug { fn now(&self) -> UtcTimestamp; }
pub trait IdSource: Send + std::fmt::Debug { fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError>; }
pub struct SystemClock; pub struct UrandomIds;           // UrandomIds reads /dev/urandom via std; no fallback
#[derive(Serialize)] pub struct EventEnvelope { pub event_id: EventId, pub at: UtcTimestamp, pub level: EventLevel,
                                               #[serde(flatten)] pub event: AppEvent }
pub struct Emitter<C: Clock, I: IdSource> { .. }
impl<C: Clock, I: IdSource> Emitter<C, I> { pub fn emit(&mut self, ev: AppEvent) -> Result<EventEnvelope, EmitError>; }
pub fn to_store_event(env: &EventEnvelope) -> Option<domain::store::log::Event>;   // None = not persisted (T01-D9)
```
- Emission-time validation (`EmitError::InvalidEvent`):
  - `RunFinished.state` must be terminal;
  - `CheckCompleted.outcome` must be terminal;
  - `RunStateChanged` must satisfy `from.can_transition(to)`;
  - reserved events are rejected.
- Level table: `StorageFailure`, `OperationFailed`, `ChatFailed` → `error`; `TriggerSkipped`, and `RunFinished` with `failed`/`interrupted`/`partial` → `warning`; everything else → `info`; `debug` is unused in P02.

### 5.9 CLI (W2, wave 3; D-17 §4.1)
| Command | Data dir | Kind | Output events / records |
|---|---|---|---|
| `--version` | no | real | app contract, store schema, wrapper protocol, UI schema and crate versions |
| `help [command]` | no | real | command table |
| `features` | no | real | feature-index rows |
| `store init --data-dir D` | required | dev | `StoreInitialised{created: [StoreKind]}` |
| `store validate --data-dir D [--root K]` | required | dev | one `StoreValidated{kind, verdict, problems}` per root |
| `store show --data-dir D <root>` | required | dev | the validated, typed, re-serialised document (never raw bytes); problems only if invalid |
| `store recover --data-dir D [--restore-previous <root>]` | required | dev | `StoreReconciled{…}` / `StoreRestored{…}` |

- Global flag `--json`: JSON Lines. App events are serialised `EventEnvelope`s; dev records are `{"mode":"dev","dev_event":"…",…}` (a separate `DevRecord` enum in `application::dev_store`, never mixed into `AppEvent`).
- Human output goes through `SanitisedText`/`RedactedText` only.
- **Exit codes:** 0 = command executed (results may include invalid stores; never "clean"); 2 = usage error; 3 = store locked by another instance; 4 = storage failure or halted writer; 5 = operation failed (typed); 6 = approval required (reserved, unused in P02).
- `--data-dir` is mandatory for store commands. There is no default path, and the CLI never reads `XDG_*`/`HOME` to find one.
- Arguments are parsed with std only.
- `main` is `fn main() -> ExitCode { cli::run(std::env::args_os(), stdin, stdout, stderr) }`, so `run` is testable in-process. The scenario tests run the built binary through `env!("CARGO_BIN_EXE_pwt-workbench-cli")` with temporary data dirs.

### 5.10 P02-D03 amendments (owner 2026-10-27; record `evidence/debug/P02-D03/record.md`)
These rows override §5.2/§5.5 where they differ. Packets: `planning/P02-packets/D03-W1.md`, `D03-W3.md`.
- **Common rule (O8):** the `domain::text` escape set adds U+00AD, U+034F, U+115F, U+1160, U+180E, U+2061–2064, U+3164, U+FFA0, U+FFF9–FFFB, U+E0000–E007F (amends `planning/P01-T01-request.md` §5.3). `values` uses the same function.
- **`CredentialRef` (O1):** additionally `^[a-z0-9][a-z0-9._-]{0,63}(:[a-z0-9._-]{1,64})*$`; the key-likeness check is a heuristic.
- **Leading `-` (O4):** rejected for `RemoteUrl` (value, user, path) and every token-family type; tokens `.`/`..` rejected. Paths and patterns keep `-`; **acceptance check for P04/P05:** every path/pattern/URL passed to a child process follows `--` in an argv array.
- **Hosts (O5):** lowercase only; a host whose last label is all digits or `0x`-prefixed must be a canonical `Ipv4Addr`. Loopback/link-local policy: P03/P16.
- **ssh/scp user (O6):** ≤ 32 bytes, O1 run check, no leading `-`.
- **URL paths (O7):** `%00`–`%1F`, `%7F`, `%2F`, `%5C`, `%3F`, `%23`, `%40` → `Forbidden`.
- **`SpdxExpression` (O9):** ≥ 1 alphanumeric; no leading, trailing or double spaces.
- **`PathPattern` (O10):** ≤ 16 wildcards (`*`-run, `?`, class each count 1); **acceptance check for P05:** the matcher is linear-time.
- **Data directory (O2):** `DataDir::new` rejects `mode & 0o077 != 0`; `acquire` rejects a data directory, `rag/` or `backup/` not owned by the lock file's uid, or `rag/`/`backup/` that is not a private real directory → `PathRejected`. Added: `FileInfo.{uid, nlink}`, `PRIVATE_DIR_MODE`, `check_private_dir`, `StoreLock::owner_uid`.
- **Opens (O3):** `RealFs` file opens use `O_NOFOLLOW|O_NONBLOCK` + fstat regular-file check (`InvalidInput`); `sync_dir` uses `O_NOFOLLOW|O_DIRECTORY`; the lock file must have `nlink == 1`. Direct dependency `libc =0.2.191` (constants only, no `unsafe`).
- **App contract 1.1 (Q-T04-1):** `AppEvent::RunStarted { run_id, started_at }`.

## 6. Policies

### 6.1 Version (A4 = 4a)
- Gate order on read: size bound → JSON parse → object check → `store_type` → `schema_version` → schema → typed decode → semantic.
- `schema_version` `"1.0"`–`"1.3"` → `UnsupportedVersion(Older)`; any other value except `"1.4"` → `NewerOrUnknown`; absent → `Missing`. No migration and no read-only legacy reader (no such stores exist, Data §19).
- A file whose `store_type` disagrees with the file it was read from → `StoreTypeMismatch`.

### 6.2 Schema validation (FR-15, FR-16)
- Embedded copy via `include_str!` of the canonical file. At first use its SHA-256 is compared with the pinned constant `909e2a55…0e2e`; a mismatch or compile failure gives `SchemaUnavailable`, and that is **never** `Valid`. A writer that cannot validate refuses to write.
- One `jsonschema` validator per root, compiled once (`OnceLock`) from the embedded document with `{"$ref": "#/$defs/<Root>File"}` as the entry, Draft 2020-12 forced, `should_validate_formats(true)` and a refusing custom retriever. Remote/file resolution is already compiled out by default-features-off; the refusing retriever is a second barrier, and its call counter is asserted to be zero in tests.
- The per-root verdict is cross-checked in tests against validation through the full `oneOf` root for every fixture (differential A). FR-16 J16-style comparison with host Python `jsonschema` 4.26.0 is lead-run in T14 through `dev/p02_differential.py` (differential B; comparator only, no install).

### 6.3 Durability (A5 = 5a)
- Directories `0700` and files `0600`, created by the app.
- **Replace protocol:**
  1. `create_new` temp `.<name>.tmp-<16 hex random>` in the same directory;
  2. write;
  3. `sync_file`;
  4. if the target exists, hard-link it to `.<name>.prev.tmp-…` and rename that over `<name>.prev` (keeps one previous validated generation without copying);
  5. rename temp over target;
  6. `sync_dir`.
- **No-overwrite protocol (reports):** temp + sync, then `hard_link(temp, final)`, which fails if `final` exists (`ReportNameCollision`); then remove the temp and `sync_dir`.
- Any I/O error → `StoreError::Io{step}`, the writer becomes **halted** (Data §11.25) and refuses further writes until reopened. The error surfaces as `StorageFailure`.
- Claims are limited to: rename atomicity on this Linux host and filesystem; the last validated copy stays readable after an injected failure at every step. No crash-proof or multi-file ACID claim (Data §12).

### 6.4 Report naming (Data §13)
- Stem: `YYYYMMDDTHHMMSSZ_<slug>_<run-id>`, from `UtcTimestamp` seconds (UTC), the repo `Slug` and the full 36-character run ID. Files go under `backup/`: `.json` is written in P02; `.md` and `.pdf` paths are reserved in the manifest with `md_written=false` and `pdf_written=false`.
- Same-second, same-repo reports differ by run ID. An exact repeat → `ReportNameCollision`, never an overwrite.
- `json_digest` = SHA-256 of the exact bytes written.

### 6.5 Startup reconciliation (Data §12.6–§12.7, T01-D8)
- Done under the lock:
  - remove only leftover temp files matching the app's exact temp pattern (count reported);
  - runs in `queued`, `awaiting_approval`, `running` or `cancelling` → `interrupted`, with `finished_at` set to reconciliation time and a fixed reason;
  - pending approvals stay `pending`;
  - manifests whose report file is missing, and report files with no manifest, are **reported, not mutated or deleted**;
  - malformed or foreign root files are reported, and writes to that root are refused until `store recover --restore-previous <root>` moves the bad file to `quarantine/<name>.<UTC stamp>` and restores `<name>.prev` (validated first) through the replace protocol.
- Size bounds: 64 MiB per store file and per report (`TooLarge`). These are development defaults; production limits follow the FR-11 owner decision.

### 6.6 FR-31 (Appendix A; T13)
- Classes: `E` closed enum, `N` validated newtype (§5.2), `R` `RedactedText`, `S` `SanitisedText`.
- `classes.rs` holds the table as data `(def, property, Class)`. `tests/fr31.rs`:
  - walks the embedded schema and fails if any plain-string property of a reachable def lacks an entry, or an entry has no property; there is a negative control with a synthetic extra property;
  - checks that each model field's Rust type matches its class, using a const type-name table per struct.
- **Trust assumption (FR-31 (2)):** the store is application-owned (Architecture trust boundary 3). `RedactedText` read back from it is re-escaped but not re-redacted.
- **Disclosed limitation:**
  - Rust has no crate-restricted visibility, so the read-back is sealed at the type level only inside `domain` (`pub(crate)` helpers): no crate can deserialise a bare `RedactedText`.
  - Store models are publicly `Deserialize`, so the rule "store documents are decoded only in `persistence::read`" is enforced by the `static` check (§8), not by the compiler.
  - Independently, `text::redact` accepts any `Redactor` (§1).

## 7. Semantic rules (T09; draft status per rule, the worker refines and the lead reviews)
Rule IDs: `D11-01`…`D11-25` (Data §11) and `D18-01`…`D18-18` plus `D18-06a` (Data §18). Statuses:
- **I**: implemented on store records in P02;
- **P**: store-level part implemented, rest in the named phase;
- **F**: producer-only, enforced in the named phase;
- **X**: policy statement, not checkable on records.

| Rule | Draft | Rule | Draft | Rule | Draft |
|---|---|---|---|---|---|
| D11-01 IDs unique / embedded IDs consistent | I | D11-10 discovery joins dynamic selectors | F P05 | D11-19 chunk digest matches text | I |
| D11-02 targets reference registered repos | F P07/P14 | D11-11 schema/renderer allowlists | P P08/P09 | D11-20 line ranges ordered | I |
| D11-03 file IDs within repo scope | I | D11-12 grant binding (workflow rev / run) | P P03 | D11-21 coverage counters agree | P P07 |
| D11-04 finding_ids resolve, IDs match | I | D11-13 inactive grants never authorise | P P03 | D11-22 source or explicit limitation | I |
| D11-05 eligible source refs | P P10/P17 | D11-14 settings change invalidates grants | F P03/P14 | D11-23 credential ref is a locator | I |
| D11-06 confined paths, credential-free remotes | I (newtypes) | D11-15 fingerprint is not authorisation | X | D11-24 chat persistence opt-in | P P17 |
| D11-07 selector mode ⇔ paths | I | D11-16 raw secrets prohibited category | P P03 | D11-25 storage errors not swallowed | I (writer) |
| D11-08 runnable workflow requirements | P P14 | D11-17 validated destinations | P P03/P16 | D18-01 `ready` tool requirements | P P06/P13 |
| D11-09 schedule field rules | I (tzdb P14) | D11-18 timestamps ordered | I | D18-02 adapter_ids policy | F P07 |
| D18-03 network policy enforced | F P06/P13 | D18-06a deferred values rejected | I | D18-12 AI non-completion | I |
| D18-04 exit code is evidence | P P06 | D18-07 file/context boundaries | P P05/P07 | D18-13 narrative not a finding | F P17 |
| D18-05 control-channel IDs | F P06 | D18-08 no raw secrets in evidence | P P06/P07 | D18-14 `pdf_written` requirements | I |
| D18-06 WrapperRequest rules | F P06 | D18-09 session-only null reference | I | D18-15 licence provisional | X P20 |
| D18-10 transmission grant ⇔ inference scope | I | D18-11 privacy constants | I | D18-16 category distinctions | F P08/P09 |
| D18-17 overlap keeps origins | F P10 | D18-18 terminal checks not pending | I | | |

## 8. Runner `dev/p02_check.py` and checks (lead, T04)
- It is a copy of `dev/p01_check.py` with the same environment, version gate, namespace, `--frozen`/`--offline` behaviour, protected-file hashing and `--repo` support. Slots `p02-lead`, `p02-w1…w3` keep separate target directories.
- **New `cli` mode:** `cargo test -p cli --frozen --tests` (all `tests/p0*_*.rs` scenarios, headless). It is included in `all`.
- Floors in `dev/p02_baseline.json`: they start at the P01 floors (416 passed, 32 compile_fail) and are raised at B1/B2/T14. They are never lowered without owner approval.
- **`static` additions** (each with a negative control in the self-test):
  1. store-model field types allowlisted (no bare `String`/`&str`/`Cow`/`Box<str>`/`serde_json::Value` in `domain/src/store/**` structs);
  2. `store_read::` used only in `domain/src/store/**`;
  3. `serde_json::from_*` / `Deserialize::deserialize` naming a store type only in `persistence/src/read.rs` and test code;
  4. `std::fs`/`File::`/`OpenOptions` only in `persistence/src/fs.rs` and `application/src/emit.rs` (`/dev/urandom`);
  5. no `std::env::var*`/`home_dir` in `cli`, `application` or `persistence`;
  6. no `println!`/`eprintln!`/`print!`/`dbg!` anywhere outside tests;
  7. `Cargo.lock` equals the T02-accepted set;
  8. the P01 checks are kept unchanged.

## 9. Test plan (minimum cases; workers may add, never remove)
| Task | Tests | Required cases |
|---|---|---|
| T05 (values + plan + provider; config-side roots) | `domain/tests/store_values.rs`, `store_models_a.rs` | every newtype: valid samples, each rejection rule, max+1 length, controls/bidi/NUL, error has no input text; Data §8/§9 examples round-trip byte-stable after one canonicalisation; minimal and maximal synthetic record per root; unknown field rejected; required-null kept as `null` |
| T06 (evidence-side roots) | `domain/tests/store_models_b.rs` | Data §10 log example; synthetic minimal/maximal Report, Log (runs, events, manifests), Suppression (read), RagIndex, Conversation; `RedactedText` read-back re-escapes an embedded ESC; `ConstFalse`/version/store_type markers reject wrong values |
| T07 (fs, lock, atomic) | `persistence/tests/fs_atomic.rs` | second `acquire` → `Locked`; lock released on drop; an unlocked stale `.app.lock` is reusable; fault at **every** `WriteStep` × {Interrupted, StorageFull, PermissionDenied} leaves the previous file byte-identical and readable; temp names match the pattern; `.prev` holds the previous generation; `write_new` never overwrites; modes 0600/0700; DataDir rejects relative, missing and symlinked paths; hostile/Unicode file names stay inside the dir |
| T08 (schema/read) | `persistence/tests/schema.rs` + fixtures | J11 (every valid example per root), J12 (examples labelled `1.3`, `1.2`, `9.9`, missing → correct `VersionClass`), J13 (store-type mismatch); format negatives (non-UUID string in a `uuid` field, invalid `date-time`) at the Schema layer; uppercase UUID / non-`Z` offset pass `format` but fail at the Typed layer (domain is stricter), asserted as such; external `$ref` schema → `SchemaUnavailable`, retriever call count 0; pinned-hash mismatch → `SchemaUnavailable`; differential A agrees on every fixture; problem pointer never contains a planted instance value; > 100 problems → `truncated` |
| T09 (semantic) | `persistence/tests/semantic.rs` | the rule table contains every rule ID exactly once; every **I** rule has ≥ 1 positive and ≥ 1 negative fixture; every **P** rule tests its store part; **F/X** rules name their phase |
| T10 (writer/recovery/naming) | `persistence/tests/writer.rs`, `recovery.rs` | validate-before-write (invalid → nothing written); halted after injected ENOSPC/EACCES, and later writes refused; Suppressions never writable; interrupted-run reconciliation for all four non-terminal states; pending approvals untouched; leftover temp removed, foreign files untouched; missing/orphan reports reported only; malformed root quarantined only through `restore_previous`; two reports in the same second → distinct names; exact repeat → `ReportNameCollision`; long or Unicode slug input bounded; manifest digest = file SHA-256 |
| T11 (emit) | `application/tests/emit.rs` | deterministic clock/ID source; envelope JSON golden for 3 events; `event_id` is v4 and never nil; every pilot `AppEvent` → `to_store_event` result is schema-valid (via `SchemaValidator`) or `None` per T01-D9; non-terminal `RunFinished`/`CheckCompleted`, illegal `RunStateChanged` and reserved events → `InvalidEvent`; `/dev/urandom` failure → typed error, no fallback |
| T12 (CLI) | `cli/tests/p02_*.rs` | `--version`, `help`, `features`; init a temp store; validate all roots; malformed-JSON root; interrupted write (pre-planted temp + `.prev`) → recover; schema 1.3-labelled file rejected; lock held by a second process → exit 3; missing `--data-dir` → exit 2; `--json` lines parse and carry `mode:"dev"` for store commands; a seeded marker string in a store file never reaches stdout or stderr unescaped; feature-index staleness test (every row names an existing command and test function) |
| T13 (FR-31) | `domain/tests/fr31.rs` + doctests | completeness test + negative control; per-field class/type agreement; compile_fail guards (each with a positive control): `serde_json::from_str::<RedactedText>`, constructing a model with `String` where a newtype is required, calling `store_read::redacted` from outside `domain` |

## 10. Parallel execution and packets (A6 = 6a)
| Wave | Worker | Task | Owns (exclusive; relative to `workbench/crates/`) | Branch |
|---|---|---|---|---|
| 1a | W1 | T05a values + plan + provider | `domain/src/store/{values,plan,provider}.rs`, `domain/tests/store_values.rs` | `p02/w1-values` |
| 1a | W3 | T07 fs/lock/atomic | `persistence/src/{paths,fs,fault,lock,atomic}.rs`, `persistence/tests/fs_atomic.rs` | `p02/w3-fs` |
| 1b | W1 | T05b config-side roots | `domain/src/store/{workflow,repository,tools,approval,config}.rs`, `domain/tests/store_models_a.rs` | `p02/w1-models-a` |
| 1b | W2 | T06 evidence-side roots | `domain/src/store/{evidence,report,log,suppression,rag,conversation}.rs`, `domain/tests/store_models_b.rs` | `p02/w2-models-b` |
| 2 | W1 | T08 | `persistence/src/{read,schema}.rs`, `persistence/tests/schema.rs`, `persistence/tests/fixtures/store/**` | `p02/w1-schema` |
| 2 | W2 | T09 | `persistence/src/semantic/*` except `mod.rs`, `persistence/tests/semantic.rs` (+ `fixtures/semantic/**`) | `p02/w2-semantic` |
| 2 | W3 | T10 | `persistence/src/{writer,recovery,report_name}.rs`, `persistence/tests/{writer,recovery}.rs` | `p02/w3-writer` |
| 3 | W1 | T11 | `application/src/emit.rs`, `application/tests/emit.rs` | `p02/w1-emit` |
| 3 | W2 | T12 | `application/src/dev_store.rs`, `cli/src/{args,commands,output,features}.rs`, `cli/tests/p02_*.rs` | `p02/w2-cli` |
| 3 | W3 | T13 | `domain/src/store/classes.rs`, `domain/tests/fr31.rs` | `p02/w3-fr31` |

- **Why wave 1 is split (T01-D3):** both model workers need `values` and the plan/provider defs. Splitting puts those in 1a, so 1b has no cross-worker type dependency. The two 1b partitions reference only 1a types, which were checked against the def graph.
- **Baselines:** B0 (scaffold), B1a (wave 1a integrated), B1 (wave 1b), B2 (wave 2), B3 (wave 3, before T14).
- **Lead-only files:** all `lib.rs`/`mod.rs`, `persistence/src/{error,validate}.rs`, `persistence/src/semantic/mod.rs`, `cli/src/{main,lib}.rs`, `domain/src/{ids,text,contract,versions}.rs` (§5.7 changes), every `Cargo.toml`, `Cargo.lock`, `dev/`, `evidence/`, `planning/`, specs and root control files.
- **Spawning:** `sonnet-implementer` (configured `model: claude-sonnet-5-5`), `isolation: "worktree"`. Packet step 0: `git merge --ff-only <baseline>`, then check `HEAD` (the harness creates worktrees from `main`).
- **Permitted commands:** `python3 -I -B dev/p02_check.py {fmt,clippy,test,build,static,cli} --slot p02-w<N> --repo <worktree>`, `git status/diff/add/commit/log` on the worker's own branch, read-only inspection. No network, no Cargo outside the runner, no edits outside owned files, no push, merge, fetch or install.
- **Stop condition:** owned tests pass under the runner, a blocker is found, or the scope would need to change. Report and stop.
- **Report format:** commit hashes; changed files; each command with exit code and per-suite counts; deviations; interface questions; limitations; anything not run, labelled "not run". Worker reports are not evidence; the lead re-verifies.
- **Integration:**
  - the lead merges each branch (cherry-pick on conflict), runs §8 itself, and runs `reviewer` on every wave;
  - `security-reviewer` runs on T05a (newtypes), T07, T08, T10 and T12;
  - finished workers' worktrees and branches are removed after preservation checks (safe `-d`).

## 11. Commits and debug cycles
- **Commits (A7 = 7a):** lead commits at B0, B1, B2 and the report, each followed by a non-force push of `phase/P02-store`. **B1a and B3 are additional lead commits** (workers fast-forward to a commit); T01-D11 asks whether they are also pushed. This request and Appendix A are committed with B0. Merging to `main` is separate.
- **Debug cycles (A9 = 9a, bounds as P01 §9):**
  - **Trigger:** an anomaly or failure the lead finds after a worker task.
  - **Process:** `P02-Dnn` record (confirmed defect vs hypotheses, evidence, bounded fix); re-spawn a fresh `sonnet-implementer` for the responsible task with the same owned files and named retests; the lead re-verifies; logged in `plan.md` §9 and `evidence/debug/P02-Dnn/`.
  - **Limits:** at most two cycles per defect, then an owner decision. Changes to frozen interfaces, acceptance criteria, dependencies or the lockfile, scope or permissions are not covered. Assertions are never weakened. Lead-owned defects are fixed by the lead under the same logging.

## 12. Decisions for the owner at this stop
| ID | Proposal | Recommended default |
|---|---|---|
| T01-D1 | Store models live in **`domain::store`** (Tech §4: domain owns records), not in `persistence`. `RedactedText` read-back through `pub(crate)` helpers in `domain::text` with re-escaping; store decoding restricted to `persistence::read` by static check; trust assumption and limitations as in §6.6 | Adopt. Alternative: models in `persistence::model` with a `#[doc(hidden)] pub` read-back function in `domain` (weaker: any crate could call it) |
| T01-D2 | Model scope = the 57 `$defs` reachable from store roots; the six wrapper-protocol defs move to P06; correct `plan.md` §5B.1/T06 wording; FR-31 count 104 | Adopt; plan text corrected by the lead in T04 (annotation, as for the P01 `syn` fact) |
| T01-D3 | Split wave 1 into 1a (W1 values/plan/provider ∥ W3 fs) and 1b (W1 config-side ∥ W2 evidence-side); B1a baseline added | Adopt. Alternative: the lead writes `values.rs` in T04 (+1.5–2 h lead time; wave 1 unsplit) |
| T01-D4 | Additive domain changes of §5.7 (v4 generation from bytes, `store_read` helpers, `Deserialize` on fieldless contract enums, `AppEvent::event_type`). P01 frozen-surface additions; no wire-format change | Adopt |
| T01-D5 | Version gate classes; problem cap 100; pointer bound 256 bytes; read bound 64 MiB as a development default | Adopt |
| T01-D6 | Durability protocol of §6.3, incl. hard-link `.prev` generation, halt-on-error and modes 0600/0700 | Adopt |
| T01-D7 | `store init` creates only the five collection roots with empty defaults (`repo`, `workflows`, `approvals`, `log`, `tools`). `config.json` is **not** created in P02, because its required limits/theme/redaction-version values would be invented (written first by P03 setup); `rag/*` in P17; `suppressions.json` never | Adopt |
| T01-D8 | Conservative reconciliation (§6.5): interrupted runs and temp cleanup only; missing/orphan reports and malformed roots reported, never mutated; explicit `--restore-previous` | Adopt |
| T01-D9 | Persist to `log.json` every must-deliver event **except** `Chat*` (chat stays out of the log, Data §11.24); `message_redacted` = the event's own detail text if it has one, else a fixed per-event summary; `code` = failure code where present | Adopt |
| T01-D10 | Envelope serialised flattened (`event_id`, `at`, `level` beside `event_type`). In CA-04 (T03) propose app contract **1.0 → 1.1** (additive; 1.0 was published with the P01 merge); store 1.4 unchanged | Adopt as the CA-04 proposal (CA-04 itself needs owner approval, A10) |
| T01-D11 | Extra lead commits B1a and B3: commit + non-force push of `phase/P02-store` like B0/B1/B2 | Commit and push (keeps origin in step with what workers build on) |
| T01-D12 | CLI exit codes and `dev` record format of §5.9; `store show` prints only validated, re-serialised content | Adopt |

## 13. Estimate
Estimate: 22–34 h total (`plan.md` §5B.3). Two changes: T01-D3 adds one integration round (B1a, +0.5–1 h lead), and §5.7 adds 0.5 h to T04. The resulting 23–35.5 h estimate is low-confidence.

## 14. Next actions after approval
1. **cargo-audit stage C-A** (approved A8; one network fetch into an isolated staging `CARGO_HOME`; no build).
2. **P02-T02:** offline lock-preserving resolution and exposure report (lead; stops on any new build script/proc-macro or entry mismatch).
3. **P02-T03:** CA-04 draft, which can run alongside T02.
4. **P02-T04:** scaffold, B0 commit and push.
5. Wave 1a, then the stages that follow.

Each step reports. Under the owner's standing instruction, the lead proceeds through T02–T04 without a further stop **only if** you say so; otherwise it stops after C-A/T02 as at P01.
