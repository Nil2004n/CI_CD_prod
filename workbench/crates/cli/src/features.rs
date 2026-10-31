//! The feature index (plan §4.1, D-17): which requirement each CLI command exercises, which scenario test
//! pins it, and whether it is real, mock, `dev` or manual-pending.
//!
//! Every row names a command listed in [`crate::commands::COMMANDS`] or [`crate::commands::P03_COMMANDS`]
//! and a test function in `crates/cli/tests/p02_*.rs` or `p03_*.rs`;
//! `p02_scenarios::feature_index_rows_name_an_existing_command_and_test` fails when either goes stale (and
//! when a command has no row). A feature that cannot be exercised headless is listed with mode
//! [`FeatureMode::ManualPending`] and a reason, never omitted (none exists in P02 or P03).

use std::io::Write;

use serde::Serialize;

use crate::output::Output;
use domain::text::sanitise;

/// How a feature is delivered.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureMode {
    /// Backed by the production path.
    Real,
    /// Backed by a labelled mock; cannot satisfy real acceptance.
    Mock,
    /// A developer entry point without an Architecture §6 command equivalent.
    Dev,
    /// Cannot be exercised headless; `reason` says why.
    ManualPending,
}

impl FeatureMode {
    /// The mode as shown to the user.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Mock => "mock",
            Self::Dev => "dev",
            Self::ManualPending => "manual-pending",
        }
    }
}

/// One row of the feature index.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub struct FeatureRow {
    /// Requirement identifiers (PRD, Tech, FR, D-17) the row covers.
    pub requirement: &'static str,
    /// The command, as named in [`crate::commands::COMMANDS`] or [`crate::commands::P03_COMMANDS`].
    pub command: &'static str,
    /// The name of the scenario test function that exercises it.
    pub scenario: &'static str,
    /// How the feature is delivered.
    pub mode: FeatureMode,
    /// Why the feature is manual-pending; `None` otherwise.
    pub reason: Option<&'static str>,
}

/// The feature index as of P03.
pub const FEATURES: [FeatureRow; 57] = [
    FeatureRow {
        requirement: "D-17; Architecture A-11 (contract versions)",
        command: "--version",
        scenario: "version_prints_all_contract_versions",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "D-17",
        command: "help",
        scenario: "help_lists_every_command",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "D-17",
        command: "help",
        scenario: "help_for_one_command_and_unknown_topic",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "D-17 (feature index, plan 4.1)",
        command: "features",
        scenario: "features_prints_the_index",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Tech 4 (store); T01-D7",
        command: "store init",
        scenario: "init_creates_exactly_five_roots_and_is_idempotent",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-15; FR-16",
        command: "store validate",
        scenario: "validate_fresh_store_and_single_root",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-15; FR-16 (version gate)",
        command: "store validate",
        scenario: "schema_1_3_label_is_rejected_with_the_version_problem",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-31 (no value echo); Data Schema 12",
        command: "store validate",
        scenario: "malformed_root_is_invalid_and_exit_is_zero",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Data Schema 12.6 (single instance)",
        command: "store validate",
        scenario: "lock_held_by_another_holder_exits_3",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-31 (validated content only)",
        command: "store show",
        scenario: "show_of_a_malformed_root_prints_problems_only",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-31 (safe text)",
        command: "store show",
        scenario: "seeded_marker_never_reaches_output_unescaped",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Data Schema 12.6-12.7; T01-D8",
        command: "store recover",
        scenario: "recover_removes_leftover_temp_files_and_reports_malformed_roots",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Data Schema 12.7; T01-D8",
        command: "store recover",
        scenario: "restore_previous_restores_and_quarantines",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-31 (no value echo); FR-15; Data Schema 12",
        command: "store validate",
        scenario: "malformed_json_in_every_root_is_invalid_not_a_failure",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-31 (validated content only); FR-15",
        command: "store show",
        scenario: "malformed_json_in_every_root_is_invalid_not_a_failure",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 6.5 (64 MiB bound); FR-15",
        command: "store validate",
        scenario: "oversized_root_is_reported_too_large_without_reading_it",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Data Schema 12.6 (single instance; stale lock reused)",
        command: "store init",
        scenario: "stale_lock_file_does_not_block_and_is_not_echoed",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Data Schema 12.6; Request 6.3 (lock file confinement)",
        command: "store validate",
        scenario: "lock_file_that_is_a_symlink_or_hard_linked_is_rejected_with_exit_5",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Data Schema 12.6 (single instance, second process)",
        command: "store recover",
        scenario: "lock_held_by_a_second_process_exits_3_then_succeeds_after_release",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 6.5; Data Schema 12.6-12.7; T01-D8",
        command: "store recover",
        scenario: "leftover_temp_files_are_removed_and_look_alikes_survive",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 6.4 (same-second reports); Data Schema 13",
        command: "store recover",
        scenario: "same_second_reports_are_recognised_and_not_orphans",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.5 (data-dir confinement); FR-31 (no path echo)",
        command: "store init",
        scenario: "hostile_data_directory_names_work_and_are_never_echoed",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.5 (path bounds)",
        command: "store init",
        scenario: "data_directory_paths_near_and_over_path_max",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.5 (data-dir confinement); Request 5.9 (exit 5)",
        command: "store validate",
        scenario: "symlinked_and_permissive_data_directories_exit_5",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.9 (usage errors, exit 2)",
        command: "store init",
        scenario: "non_utf8_and_flag_like_data_dir_values_are_usage_errors",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 6.3 (storage failure); Request 5.9 (exit 4)",
        command: "store init",
        scenario: "real_storage_failure_exits_4_when_permissions_are_enforced",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 6.3; Request 5.9 (class to exit code)",
        command: "store validate",
        scenario: "application_storage_failures_map_to_exit_4",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Architecture 10; D01 L1 (destinations shown in canonical form)",
        command: "approvals list",
        scenario: "approvals_list_shows_canonical_destinations_and_labels_the_ssh_store_form",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Architecture 10 (read-only listing)",
        command: "approvals list",
        scenario: "approvals_list_on_an_empty_store_prints_nothing",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.7 and 5.9 (store failures keep their exit codes)",
        command: "approvals list",
        scenario: "approval_commands_report_store_failures_with_the_store_exit_codes",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.9 (usage errors, exit 2)",
        command: "approvals list",
        scenario: "approval_and_dev_usage_errors_exit_2",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.4 (decision table: approve once); Data 11 rule 13",
        command: "approvals approve",
        scenario: "permission_check_once_grant_goes_6_approve_0_then_6_again",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.4 (approve workflow scope); Data 11 rule 14",
        command: "approvals approve",
        scenario: "workflow_scope_grant_holds_until_the_workflow_revision_changes",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.4 (workflow scope unavailable, exit 5)",
        command: "approvals approve",
        scenario: "workflow_scope_is_unavailable_without_a_saved_workflow_at_the_revision",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.7 (unknown approval, exit 5)",
        command: "approvals approve",
        scenario: "unknown_approval_ids_exit_5",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.4 (deny; a denied request stays refused)",
        command: "approvals deny",
        scenario: "deny_blocks_the_identical_request_and_a_decided_approval_cannot_be_decided_again",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.7 (unknown approval, exit 5)",
        command: "approvals deny",
        scenario: "unknown_approval_ids_exit_5",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.4 (revoke only an approved grant); Data 11 rule 13",
        command: "approvals revoke",
        scenario: "revoke_ends_a_grant_and_only_an_approved_grant_can_be_revoked",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.7 (unknown approval, exit 5)",
        command: "approvals revoke",
        scenario: "unknown_approval_ids_exit_5",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "FR-33 (root running against a foreign data directory, exit 5)",
        command: "approvals list",
        scenario: "fr33_refusal_maps_to_exit_5_with_the_fixed_message",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01; Request 5.6 (session-only key; status absent in a new process)",
        command: "credential set",
        scenario: "credential_lifecycle_in_one_session_and_absent_in_a_fresh_one",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01; Request 5.6 (C5: no terminal input)",
        command: "credential set",
        scenario: "credential_set_refuses_a_terminal_and_reads_nothing",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01; Request 5.6 (key format 16 to 4096 printable ASCII)",
        command: "credential set",
        scenario: "malformed_keys_are_refused_with_exit_5_and_never_echoed",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01 (the key is held for one process only)",
        command: "credential set",
        scenario: "binary_key_is_held_for_one_process_only",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01 (no key in output, argv or files)",
        command: "credential set",
        scenario: "seeded_key_appears_in_no_output_argv_or_file",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01; Request 5.6 (status never shows key material)",
        command: "credential status",
        scenario: "credential_json_shapes",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01 (status absent in a fresh session)",
        command: "credential status",
        scenario: "binary_key_is_held_for_one_process_only",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "KEY-01 (delete removes the key)",
        command: "credential delete",
        scenario: "credential_lifecycle_in_one_session_and_absent_in_a_fresh_one",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.9 (usage errors, exit 2)",
        command: "credential set",
        scenario: "credential_usage_errors_exit_2",
        mode: FeatureMode::Real,
        reason: None,
    },
    FeatureRow {
        requirement: "Tech 5 and 18 (child environment); Request 7 (profile variable sets)",
        command: "dev env-preview",
        scenario: "env_preview_shows_names_and_origins_never_values",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 7 (TZ holding the session key is dropped)",
        command: "dev env-preview",
        scenario: "env_preview_drops_inherited_tz_holding_the_key",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 7 (deny rules fail closed)",
        command: "dev env-preview",
        scenario: "env_preview_fails_closed_on_bad_directories",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "T01-D5 (host environment read once, in main)",
        command: "dev env-preview",
        scenario: "binary_env_preview_uses_the_process_environment",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "T01-D6; Request 5.4 (asks again after a once grant)",
        command: "dev permission-check",
        scenario: "permission_check_once_grant_goes_6_approve_0_then_6_again",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 6 (refusals R1 to R7); Data 11 rule 16",
        command: "dev permission-check",
        scenario: "refused_requests_exit_5_with_the_reason",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "T01-D6 (request document bounds and validation)",
        command: "dev permission-check",
        scenario: "invalid_request_documents_exit_5_and_record_nothing",
        mode: FeatureMode::Dev,
        reason: None,
    },
    FeatureRow {
        requirement: "Request 5.7 (exit codes 6, 0, 5 of the process)",
        command: "dev permission-check",
        scenario: "binary_permission_check_exit_codes",
        mode: FeatureMode::Dev,
        reason: None,
    },
];

/// Prints the feature index: aligned text lines, or one JSON object per row.
pub fn print<W: Write, E: Write>(out: &mut Output<W, E>) {
    if out.is_json() {
        for row in FEATURES {
            out.json_line(&FeatureLine {
                record: "feature",
                row,
            });
        }
        return;
    }
    out.line(&sanitise("requirement | command | scenario test | mode"));
    for row in FEATURES {
        let mut line = format!(
            "{} | {} | {} | {}",
            row.requirement,
            row.command,
            row.scenario,
            row.mode.as_str()
        );
        if let Some(reason) = row.reason {
            line.push_str(" | ");
            line.push_str(reason);
        }
        out.line(&sanitise(&line));
    }
}

/// The JSON form of one row: the row's own members (its `mode` is the feature's delivery mode) after a
/// `record` discriminator.
#[derive(Serialize)]
struct FeatureLine {
    record: &'static str,
    #[serde(flatten)]
    row: FeatureRow,
}
