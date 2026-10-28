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
use domain::store::StoreKind;
use domain::text::{SanitisedText, sanitise};
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

    /// Flushes standard output; a failure is remembered like a write failure.
    pub fn finish(&mut self) {
        if !self.stdout_failed && self.stdout.flush().is_err() {
            self.stdout_failed = true;
        }
        let _ = self.stderr.flush();
    }
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
}
