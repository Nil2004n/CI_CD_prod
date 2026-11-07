# T05 startup read fault-injection debug

Owner approved the proposed bounded fix after the prior writer regression failure.
Revision: dirty `f71749c53704afbd87150ac6d31b65f69377000b`, branch `phase/P04-git-acquisition`.

The unjournaled-1.5 scan reads each of the five initialized roots before the
existing reconciliation reads. `READS_AT_OPEN` is now 10; the test additionally
asserts the observed count before injecting the report-write read failure. Its
failure, halted-writer, persistence and recovery assertions remain intact.

Primary verification, cwd `workbench`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` for Cargo test/Clippy commands, offline and locked:

- `cargo test -p persistence --test writer --test recovery --test store_integration`: 49 passed, 0 failed, 0 ignored (exit 0).
- `cargo test -p persistence --test p04_crossroot15 --test p04_upgrade15 --test p04_migration15 --test p04_repository15_semantics --test p04_approval15_semantics --test p04_schema15`: 31 passed, 0 failed, 0 ignored (exit 0).
- `cargo test -p application --test p04_registration15 --test p04_registry --test p04_service --test p04_record15`: 8 passed, 0 failed, 0 ignored (exit 0).
- `cargo clippy -p persistence --tests -- -D warnings` and `cargo clippy -p application --tests -- -D warnings`: exit 0.
- `cargo fmt --all -- --check` and repository-root `git diff --check`: exit 0.

No full phase check or real data migration was performed. Fault injection models
operation failures, not power loss. Existing filesystem parent-component race
limitations remain. Report writing and restore through the 1.5 writer currently
refuse; broader 1.5 compatibility and phase acceptance remain unverified.
