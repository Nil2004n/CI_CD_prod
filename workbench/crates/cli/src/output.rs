//! CLI output (request §5.9): human text and JSON Lines.
//!
//! Every byte written to the terminal streams passes through [`SanitisedText`] (the escaping of control,
//! bidirectional, zero-width and separator code points as `<U+XXXX>`), whether it is a human line or a JSON
//! line. In JSON mode the escaping is applied to the already-encoded line: the escaped code points can only
//! occur inside JSON strings, and the replacement text is plain ASCII, so the line stays valid JSON.
//!
//! Write failures (for example a closed pipe) never panic. The first failure on standard output is
//! remembered, later writes to it are skipped, and the caller decides the exit code
//! ([`Output::stdout_failed`]).

use std::io::Write;

use application::dev_store::{DevProblem, DevRecord, DevVerdict, root_name};
use application::permission::NormalisedDestination;
use domain::contract::{ApprovalLifetime, DataCategory};
use domain::ids::{ApprovalId, RepoId, RunId, WorkflowId};
use domain::states::ApprovalState;
use domain::store::StoreKind;
use domain::store::approval::Approval;
use domain::store::provider::{InferenceScope, ProviderPolicy};
use domain::text::{SanitisedText, sanitise};
use domain::time::UtcTimestamp;
use domain::vocab::Capability;
use serde::Serialize;

/// The two output streams of one invocation and the output mode.
#[derive(Debug)]
pub struct Output<W: Write, E: Write> {
    json: bool,
    stdout: W,
    stderr: E,
    stdout_failed: bool,
}

impl<W: Write, E: Write> Output<W, E> {
    /// Output over the given streams; `json` selects JSON Lines on standard output.
    pub fn new(json: bool, stdout: W, stderr: E) -> Self {
        Self {
            json,
            stdout,
            stderr,
            stdout_failed: false,
        }
    }

    /// Whether standard output is JSON Lines.
    pub fn is_json(&self) -> bool {
        self.json
    }

    /// Whether a write to standard output (or the final flush) failed.
    pub fn stdout_failed(&self) -> bool {
        self.stdout_failed
    }

    /// Writes `text` and a newline to standard output. After a failure nothing more is written.
    pub fn line(&mut self, text: &SanitisedText) {
        if self.stdout_failed {
            return;
        }
        let ok = self
            .stdout
            .write_all(text.as_str().as_bytes())
            .and_then(|()| self.stdout.write_all(b"\n"))
            .is_ok();
        if !ok {
            self.stdout_failed = true;
        }
    }

    /// Writes `text` and a newline to standard error. A failure is ignored: there is nowhere left to
    /// report it.
    pub fn error_line(&mut self, text: &SanitisedText) {
        let _ = self
            .stderr
            .write_all(text.as_str().as_bytes())
            .and_then(|()| self.stderr.write_all(b"\n"));
    }

    /// Writes `value` as one JSON line (escaped as described in the module documentation). A value that
    /// cannot be encoded is reported on standard error and counts as a failed write.
    pub fn json_line<T: Serialize>(&mut self, value: &T) {
        match serde_json::to_string(value) {
            Ok(encoded) => self.line(&sanitise(&encoded)),
            Err(_) => {
                self.stdout_failed = true;
                self.error_line(&sanitise("error: a record could not be encoded"));
            }
        }
    }

    /// Writes one `dev` record in the current mode.
    pub fn dev_record(&mut self, record: &DevRecord) {
        if self.json {
            self.json_line(record);
        } else {
            self.line(&render_record(record));
        }
    }

    /// Writes one approval in the current mode (`approvals list`).
    pub fn approval(&mut self, approval: &Approval) {
        if self.json {
            self.json_line(&ApprovalRecord::of(approval));
        } else {
            for line in render_approval(approval) {
                self.line(&line);
            }
        }
    }

    /// Flushes standard output; a failure is remembered like a write failure.
    pub fn finish(&mut self) {
        if !self.stdout_failed && self.stdout.flush().is_err() {
            self.stdout_failed = true;
        }
        let _ = self.stderr.flush();
    }
}

/// The inference scope of an approval as listed: provider, model, routing policy, how many sources and the
/// redaction version. Source identifiers and any content are not shown (P03-D02 A3).
#[derive(Serialize)]
struct ScopeView<'a> {
    provider_id: String,
    model_id: String,
    routing_policy: &'a ProviderPolicy,
    source_count: usize,
    redaction_version: String,
}

impl<'a> ScopeView<'a> {
    fn of(scope: &'a InferenceScope) -> Self {
        Self {
            provider_id: snake(&scope.provider_id),
            model_id: snake(&scope.model_id),
            routing_policy: &scope.routing_policy,
            source_count: scope.source_ids.len(),
            redaction_version: snake(&scope.redaction_version),
        }
    }
}

/// The human text of an inference scope (one line).
fn scope_text(scope: &InferenceScope) -> String {
    let policy = serde_json::to_value(&scope.routing_policy).unwrap_or(serde_json::Value::Null);
    let member = |name: &str| {
        policy
            .get(name)
            .map_or_else(|| String::from("?"), |v| v.to_string().replace('"', ""))
    };
    format!(
        "provider {}; model {}; data_collection={} zdr={} allow_fallbacks={}; sources: {}; redaction {}",
        one_line(&snake(&scope.provider_id)),
        one_line(&snake(&scope.model_id)),
        member("data_collection"),
        member("zdr"),
        member("allow_fallbacks"),
        scope.source_ids.len(),
        one_line(&snake(&scope.redaction_version))
    )
}

/// How a destination of an approval record is shown (D01 L1): the canonical form where the stored text
/// allows it, otherwise the store form, labelled.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct DestinationView {
    /// `canonical` (`https://host:port[/path]`) or `store` (the stored text as it is).
    pub form: &'static str,
    /// The destination text.
    pub value: String,
}

impl DestinationView {
    /// The view of one stored destination.
    ///
    /// An `https` store form is re-normalised and shown by [`NormalisedDestination::canonical`]. A `host:port`
    /// form is what the store keeps of an `ssh` destination and drops the ssh path (D01 L1), so the
    /// canonical form cannot be rebuilt from it: it is shown as the store form, labelled as such. Anything
    /// the normaliser rejects is also shown as the store form (the store validated it, so this is only an
    /// older or hand-written record).
    pub fn of(stored: &str) -> Self {
        if stored.starts_with("https://")
            && let Ok(normal) = NormalisedDestination::parse(stored)
        {
            return Self {
                form: "canonical",
                value: normal.canonical(),
            };
        }
        Self {
            form: "store",
            value: stored.to_owned(),
        }
    }
}

/// The snake_case name of a serialisable fieldless enum value (the name the JSON form uses).
pub fn snake<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::from("unknown"),
    }
}

/// The JSON form of one approval (`approvals list --json`). Holds no key material: an approval record
/// carries a fingerprint digest, never an argument value, and the purpose and actor are fixed texts.
#[derive(Serialize)]
struct ApprovalRecord<'a> {
    mode: &'static str,
    record: &'static str,
    approval_id: ApprovalId,
    status: ApprovalState,
    capability: Capability,
    lifetime: ApprovalLifetime,
    workflow_id: Option<WorkflowId>,
    workflow_revision: Option<u64>,
    run_id: Option<RunId>,
    repo_ids: &'a [RepoId],
    destinations: Vec<DestinationView>,
    resource_constraints: Vec<&'a str>,
    data_categories: &'a [DataCategory],
    inference_scope: Option<ScopeView<'a>>,
    requested_at: UtcTimestamp,
    decided_at: Option<UtcTimestamp>,
    expires_at: Option<UtcTimestamp>,
    actor: &'a str,
    purpose: &'a str,
    consumed: bool,
}

impl<'a> ApprovalRecord<'a> {
    fn of(a: &'a Approval) -> Self {
        Self {
            mode: "real",
            record: "approval",
            approval_id: a.approval_id,
            status: a.status,
            capability: a.capability,
            lifetime: a.lifetime,
            workflow_id: a.workflow_id,
            workflow_revision: a.workflow_revision,
            run_id: a.run_id,
            repo_ids: &a.repo_ids,
            destinations: a
                .destinations
                .iter()
                .map(|d| DestinationView::of(d.as_str()))
                .collect(),
            resource_constraints: a.resource_constraints.iter().map(|c| c.as_str()).collect(),
            data_categories: &a.data_categories,
            inference_scope: a.inference_scope.as_ref().map(ScopeView::of),
            requested_at: a.requested_at,
            decided_at: a.decided_at,
            expires_at: a.expires_at,
            actor: a.actor.as_str(),
            purpose: a.purpose.as_str(),
            consumed: a.consumed,
        }
    }
}

/// Makes free text one visible line: LF, CR and TAB, which the frozen escape set of `sanitise` keeps or
/// treats as layout, are shown as `<U+000A>`, `<U+000D>` and `<U+0009>` (P03-D02 A1). Applied to every
/// free-text field of the human approval view before the line is sanitised, so a multi-line constraint
/// cannot forge further lines.
fn one_line(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => shown.push_str("<U+000A>"),
            '\r' => shown.push_str("<U+000D>"),
            '\t' => shown.push_str("<U+0009>"),
            other => shown.push(other),
        }
    }
    shown
}

/// `-` for an absent value, the value otherwise.
fn or_dash<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| String::from("-"), |v| v.to_string())
}

/// The human rendering of one approval: one line per fact, each sanitised.
pub fn render_approval(a: &Approval) -> Vec<SanitisedText> {
    let mut lines = vec![format!(
        "{} {} {} lifetime={}{}",
        a.approval_id,
        snake(&a.status),
        snake(&a.capability),
        snake(&a.lifetime),
        if a.consumed { " (consumed)" } else { "" }
    )];
    lines.push(format!(
        "  workflow: {} revision {}; run: {}; repositories: {}",
        or_dash(a.workflow_id),
        or_dash(a.workflow_revision),
        or_dash(a.run_id),
        a.repo_ids.len()
    ));
    for destination in &a.destinations {
        let view = DestinationView::of(destination.as_str());
        let label = if view.form == "canonical" {
            "canonical"
        } else {
            "store form (an ssh path is not recorded)"
        };
        lines.push(format!("  destination: {} [{label}]", view.value));
    }
    if !a.data_categories.is_empty() {
        let names: Vec<String> = a.data_categories.iter().map(snake).collect();
        lines.push(format!("  data: {}", names.join(", ")));
    }
    for constraint in &a.resource_constraints {
        lines.push(format!("  constraint: {}", one_line(constraint.as_str())));
    }
    lines.push(format!(
        "  requested: {}; decided: {}; expires: {}; actor: {}",
        a.requested_at,
        or_dash(a.decided_at),
        or_dash(a.expires_at),
        one_line(a.actor.as_str())
    ));
    if let Some(scope) = &a.inference_scope {
        lines.push(format!("  inference scope: {}", scope_text(scope)));
    }
    lines.push(format!("  purpose: {}", one_line(a.purpose.as_str())));
    lines.iter().map(|l| sanitise(l)).collect()
}

/// The root names, comma separated, or `none`.
fn kind_list(kinds: &[StoreKind]) -> String {
    if kinds.is_empty() {
        return String::from("none");
    }
    kinds
        .iter()
        .map(|k| root_name(*k))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Indented problem lines, and a note if the list was cut.
fn problem_lines(problems: &[DevProblem], truncated: bool, out: &mut String) {
    for p in problems {
        let pointer = if p.pointer.is_empty() {
            "(document root)"
        } else {
            p.pointer.as_str()
        };
        out.push_str(&format!("  {} {} at {pointer}", p.layer, p.code));
        if let Some(rule) = p.rule {
            out.push_str(&format!(" (rule {rule})"));
        }
        out.push('\n');
    }
    if truncated {
        out.push_str("  (further problems not listed)\n");
    }
}

/// The human rendering of a record, as sanitised text without a trailing newline.
pub fn render_record(record: &DevRecord) -> SanitisedText {
    let mut text = String::new();
    match record {
        DevRecord::StoreInitialised {
            created,
            already_present,
        } => {
            text.push_str(&format!(
                "store initialised\n  created: {}\n",
                kind_list(created)
            ));
            text.push_str(&format!(
                "  already present: {}",
                kind_list(already_present)
            ));
        }
        DevRecord::StoreValidated {
            kind,
            verdict,
            problems,
            truncated,
        } => {
            text.push_str(&format!("{}: {}\n", root_name(*kind), verdict.as_str()));
            problem_lines(problems, *truncated, &mut text);
        }
        DevRecord::StoreShown {
            kind,
            verdict,
            document,
            problems,
            truncated,
        } => match (verdict, document) {
            (DevVerdict::Valid, Some(doc)) => match serde_json::to_string_pretty(doc) {
                Ok(pretty) => text.push_str(&pretty),
                Err(_) => text.push_str(&format!(
                    "{}: valid (the document could not be rendered)",
                    root_name(*kind)
                )),
            },
            _ => {
                text.push_str(&format!("{}: {}\n", root_name(*kind), verdict.as_str()));
                problem_lines(problems, *truncated, &mut text);
            }
        },
        DevRecord::StoreReconciled {
            temp_files_removed,
            runs_interrupted,
            approvals_pending,
            manifests_without_report,
            reports_without_manifest,
            malformed_roots,
        } => {
            text.push_str("store reconciled\n");
            text.push_str(&format!("  temp files removed: {temp_files_removed}\n"));
            text.push_str(&format!(
                "  runs set to interrupted: {}\n",
                runs_interrupted.len()
            ));
            for run in runs_interrupted {
                text.push_str(&format!("    {run}\n"));
            }
            text.push_str(&format!("  approvals still pending: {approvals_pending}\n"));
            text.push_str(&format!(
                "  manifests without a report file: {}\n",
                manifests_without_report.len()
            ));
            for report in manifests_without_report {
                text.push_str(&format!("    {report}\n"));
            }
            text.push_str(&format!(
                "  report files without a manifest: {reports_without_manifest}\n"
            ));
            text.push_str(&format!(
                "  malformed roots: {}",
                kind_list(malformed_roots)
            ));
        }
        DevRecord::StoreRestored { kind, quarantined } => {
            text.push_str(&format!(
                "restored {} from its previous generation",
                root_name(*kind)
            ));
            if *quarantined {
                text.push_str("; the replaced file was moved to quarantine/");
            } else {
                text.push_str("; there was no current file to move");
            }
        }
    }
    let trimmed = text.trim_end_matches('\n');
    sanitise(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that always fails, standing for a closed pipe.
    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }

    /// Fails if a write error panics, is not remembered, or later lines are still attempted.
    #[test]
    fn a_broken_stdout_is_remembered_and_never_panics() {
        let mut out = Output::new(false, Broken, Vec::new());
        out.line(&sanitise("first"));
        assert!(out.stdout_failed());
        out.line(&sanitise("second"));
        out.finish();
        assert!(out.stdout_failed());
    }

    /// Fails if a JSON line is not escaped before it is written, or the escaped line stops being JSON.
    #[test]
    fn json_lines_are_escaped_and_stay_valid_json() {
        let mut bytes = Vec::new();
        {
            let mut out = Output::new(true, &mut bytes, Vec::new());
            out.json_line(&serde_json::json!({ "k": "a\u{202e}b\u{85}c" }));
        }
        let line = String::from_utf8(bytes).unwrap();
        assert!(!line.contains('\u{202e}'));
        assert!(!line.contains('\u{85}'));
        let parsed: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(parsed["k"], "a<U+202E>b<U+0085>c");
    }

    /// Fails if a restore record loses the quarantine information.
    #[test]
    fn restored_record_mentions_quarantine() {
        let text = render_record(&DevRecord::StoreRestored {
            kind: StoreKind::Repositories,
            quarantined: true,
        });
        assert!(text.as_str().contains("quarantine"));
        assert!(text.as_str().contains("repositories"));
    }

    /// Fails if an https store form is not shown canonically, an ssh store form is shown as canonical, or a
    /// text the normaliser rejects is dropped instead of shown as the store form.
    #[test]
    fn destination_views_label_the_form() {
        assert_eq!(
            DestinationView::of("https://github.com:443/a/b"),
            DestinationView {
                form: "canonical",
                value: String::from("https://github.com:443/a/b")
            }
        );
        assert_eq!(
            DestinationView::of("https://Example.COM./x").form,
            "canonical"
        );
        assert_eq!(
            DestinationView::of("https://Example.COM./x").value,
            "https://example.com:443/x"
        );
        assert_eq!(
            DestinationView::of("git.example.org:22"),
            DestinationView {
                form: "store",
                value: String::from("git.example.org:22")
            }
        );
        assert_eq!(DestinationView::of("https://bad host/").form, "store");
    }

    /// Fails if `snake` stops giving the serde name of an enum value or hides a non-string value.
    #[test]
    fn snake_names_enum_values() {
        assert_eq!(snake(&ApprovalState::Pending), "pending");
        assert_eq!(snake(&Capability::GitAcquire), "git_acquire");
        assert_eq!(
            snake(&ApprovalLifetime::WorkflowRevision),
            "workflow_revision"
        );
        assert_eq!(snake(&5u32), "unknown");
    }
}
