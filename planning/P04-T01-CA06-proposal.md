# P04-T01: proposed CA-06 contract amendment — owner review

Status: design approved by owner 2026-11-03 (“okay i approve”); implementation and canonical schema application remain pending the appropriate phase authorization. This document makes the T01 design concrete; it does not amend the canonical schema, authorize implementation, or select a schema version. CA-05 also proposes store 1.5, so the version and migration sequence must be reconciled before either amendment is applied.

## Recommended behavior

`repo add <url>` registers a repository without network access. Represent its initial reference as unresolved, with both `default_ref_kind` and `default_ref_value` null. After a matching `git_acquire` approval, query the remote symbolic HEAD; accept only a valid branch reference. If HEAD is missing, unborn, or invalid, report a safe failure and leave the reference unresolved. Persist the discovered branch only after it has been validated; an interrupted operation must not imply a successful checkout. Subsequent refresh binds the resolved reference and remote in its authorization fingerprint. No remote-provided text grants permission.

Add nullable `last_git_version` to the persisted repository. Set it only after a successful, verified Git operation; retain the previous successful value on failure. Persist it together with the successful commit OID and update time through the existing store writer. The accepted value must match the allowlisted Git version syntax and contain no raw process output. A missing value means unknown, including for existing records.

## Contract changes for a future approved amendment

- Schema: permit the reference fields to be null only as a pair; require both to be populated together for a resolved reference. Add nullable `last_git_version`; constrain its grammar and length. Define the exact version identifier after reconciling CA-05.
- Typed Rust models: make both reference fields optional and add the optional version field. Validate pair consistency in schema and semantic validation; do not interpret `HEAD` as a branch sentinel.
- Migration: preserve existing reference values and set `last_git_version` to null when upgrading existing records. Specify versioned migration and backward-read behavior in T03; do not claim old binaries can read the new schema.
- Application and CLI: keep `repo add <url>`; represent unresolved references and unknown version explicitly in status output. Ensure approval happens before discovery and that the result cannot broaden an existing grant.
- Tests: existing-record migration; valid and mismatched null pairs; unresolved registration without network; approval-required discovery; missing/unborn/malicious symbolic HEAD; interruption at each write; success and failure version persistence; changed ref/remote requiring authorization; compatibility with the eventual CA-05 migration.

## Review boundaries

The approval fingerprint currently includes the ref value. Discovery must therefore use a distinct, narrowly scoped pre-discovery `git_acquire` request for the validated remote, followed by authorization of the resolved target before checkout. Freeze the precise operation names and arguments in T03/T04 after checking the existing P03 evaluator; do not reuse the pre-discovery grant to authorize an arbitrary target. The owner accepted this two-step design on 2026-11-03; exact operation names and arguments remain to be specified in T03/T04.

For deletion, retain the existing P04 scope: refuse deletion if path, mount or ownership checks cannot establish the stated confinement; document residual same-user races and require hostile-fixture proof before acceptance. No schema changes or product code are applied by this proposal.
