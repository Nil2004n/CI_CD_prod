//! The feature index (plan §4.1, D-17): which requirement each CLI command exercises, which scenario test
//! pins it, and whether it is real, mock, `dev` or manual-pending.
//!
//! Every row names a command listed in [`crate::commands::COMMANDS`] and a test function in
//! `crates/cli/tests/p02_*.rs`; `p02_scenarios::feature_index_rows_name_an_existing_command_and_test`
//! fails when either goes stale. A feature that cannot be exercised headless is listed with mode
//! [`FeatureMode::ManualPending`] and a reason, never omitted (none exists in P02).

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
    /// The command, as named in [`crate::commands::COMMANDS`].
    pub command: &'static str,
    /// The name of the scenario test function that exercises it.
    pub scenario: &'static str,
    /// How the feature is delivered.
    pub mode: FeatureMode,
    /// Why the feature is manual-pending; `None` otherwise.
    pub reason: Option<&'static str>,
}

/// The feature index as of P02.
pub const FEATURES: [FeatureRow; 13] = [
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
