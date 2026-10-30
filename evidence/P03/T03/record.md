# P03-T03 — contract check (C8 = 8a)

Date: 2026-10-30. Opus, read-only against `workbench/crates/domain/src/{contract.rs,store/*.rs}`, Architecture §6/§7/§10, Data §11 rules 12–17 and 23, UI §10, Data §25 (CA-04).

**Result: no CA-05 needed. App contract stays 1.1, store 1.4, wrapper protocol 1.1, UI schema 1.1; schema file unchanged.**

| Item needed by P03 | Existing contract | Conclusion |
|---|---|---|
| Approve once / workflow scope / deny | `AppCommand::ResolveApproval { approval_id, decision: ApprovalDecision }` with `ApproveOnce`, `ApproveWorkflowScope`, `Deny` | sufficient; the actor is the presentation adapter (CLI/TUI) and is passed to the service, not carried in the command |
| Revoke | `AppCommand::RevokeApproval` | sufficient |
| Lifetimes once / run / workflow (C7) | `ApprovalLifetime { Once, Run, WorkflowRevision }` | sufficient; no session lifetime (C7 = 7a) |
| Credential statuses | `CredentialStatus { Absent, SessionOnly, Stored, Invalid, StoreUnavailable }` | sufficient: plan names map `absent → Absent`, `present → SessionOnly`. A badly formatted key is refused at input (typed `KeyFormatError`) and never stored, so it has no status. `Invalid` stays reserved for provider rejection (P16); `Stored`/`StoreUnavailable` for OS-store mode (not adopted, C2) |
| Credential commands | `ConfigureCredential { action: CredentialAction::{Set, Replace, Validate, Delete} }`, `CredentialMode::SessionOnly` | sufficient; `Validate` is P16 |
| Events | `ApprovalRequested` (workflow_revision `Option<u32>`), `ApprovalResolved`, `ApprovalRevoked`, `CredentialStatusChanged { status }` | sufficient; P03 requests carry the revision as `u32` (store field is `u64`) |
| Data categories | closed `DataCategory`; no raw-secret value (rule 16) | sufficient; the request-side `RequestedData::RawSecretValues` is application-internal and always refused |
| `dev env-preview`, `dev permission-check` | CA-04: `dev` commands are presentation-only and labelled | no contract change |

Domain API additions (lead, T04), none of which changes a serialised contract:
- `contract::FixedText` (fixed approval actor/purpose texts as `RedactedText`). `application` cannot construct `RedactedText` itself (`redact_static` is `pub(crate)`; no production `Redactor` before P06).
- `store::values::Destination::to_redacted` (a validated destination as event text).
- `RawInput`: `Drop` with best-effort clearing; `Debug` without length (KEY-01 hardening carried from P01).
