//! Rule registry: one [`super::Rule`] per rule ID of request §7, plus the helpers the rule bodies share.
//!
//! The registry lists the 44 rule IDs of Data Schema §11 (rules 1–25) and §18 (rules 1–18 and 6a) exactly once,
//! in the order `D11-01`…`D11-25`, `D18-01`…`D18-18`, `D18-06a`. A rule has a check function exactly when its
//! status is [`RuleStatus::Implemented`] or [`RuleStatus::Partial`]; producer-only and policy rules carry the
//! phase that honours them and no function (`planning/P02-T01-request.md` §7, §5.12).
//!
//! Every rule body is a pure function over one decoded [`AnyStore`]. Pointers are built from schema property
//! names and array indices only, never from document values or map keys (request §5.11 E-3).

use std::collections::HashSet;
use std::hash::Hash;

use domain::store::AnyStore;
use domain::store::evidence::SourceRef;
use domain::store::plan::PlanSnapshot;

use super::{Rule, RuleInfo, RuleStatus, ai, approval, evidence, ids, workflow};
use crate::validate::{Layer, Problem, ProblemCode, Problems, RuleId};

/// Builds one registry entry.
macro_rules! rule {
    ($id:literal, $status:expr, $title:literal, $check:expr) => {
        Rule {
            info: RuleInfo {
                id: RuleId($id),
                status: $status,
                title: $title,
            },
            check: $check,
        }
    };
}

/// Shorthand for [`RuleStatus::Implemented`] inside the registry.
const I: RuleStatus = RuleStatus::Implemented;

/// The registry: every rule ID of request §7 exactly once, in registry order.
pub(crate) static REGISTRY: &[Rule] = &[
    rule!(
        "D11-01",
        I,
        "IDs are unique and embedded IDs agree",
        Some(ids::d11_01)
    ),
    rule!(
        "D11-02",
        RuleStatus::Producer {
            phase: "P07/P14 (launch-time check that workflow targets name registered repositories)",
        },
        "Workflow targets reference registered repositories at launch",
        None
    ),
    rule!(
        "D11-03",
        I,
        "File IDs and source references stay within the repository scope",
        Some(ids::d11_03)
    ),
    rule!(
        "D11-04",
        I,
        "Check finding IDs resolve and finding check and task IDs match",
        Some(ids::d11_04)
    ),
    rule!(
        "D11-05",
        RuleStatus::Partial {
            rest_in: "P10/P17 (retained or resolvable evidence at retrieval and citation time)",
        },
        "Source references are eligible and resolvable",
        Some(ids::d11_05)
    ),
    rule!(
        "D11-06",
        I,
        "Internal paths are confined and remotes carry no credentials",
        Some(ids::d11_06)
    ),
    rule!(
        "D11-07",
        I,
        "Selector mode agrees with its paths and patterns",
        Some(workflow::d11_07)
    ),
    rule!(
        "D11-08",
        RuleStatus::Partial {
            rest_in: "P14 (installed and acknowledged capabilities)",
        },
        "Runnable workflows and plans have targets, categories and outputs",
        Some(workflow::d11_08)
    ),
    rule!(
        "D11-09",
        RuleStatus::Partial {
            rest_in: "P14 (time-zone database membership of the IANA zone)",
        },
        "Schedule fields match the schedule kind",
        Some(workflow::d11_09)
    ),
    rule!(
        "D11-10",
        RuleStatus::Producer {
            phase: "P05 (file discovery joins new files to dynamic selectors only)",
        },
        "Newly discovered files join dynamic selector modes only",
        None
    ),
    rule!(
        "D11-11",
        RuleStatus::Partial {
            rest_in: "P08/P09 (schema resource and renderer allowlists)",
        },
        "Renderer parameters cannot carry shell commands",
        Some(workflow::d11_11)
    ),
    rule!(
        "D11-12",
        RuleStatus::Partial {
            rest_in: "P07/P14 (callers pass the real run and saved workflow revision to the approval service and end a run's grants with expire_run)",
        },
        "Grants are bound to a workflow revision or a run",
        Some(approval::d11_12)
    ),
    rule!(
        "D11-13",
        RuleStatus::Partial {
            rest_in: "P04/P07/P14/P16 (every operation that needs a grant calls the approval service before it runs)",
        },
        "Pending and inactive grants stay consistent and never authorise",
        Some(approval::d11_13)
    ),
    rule!(
        "D11-14",
        RuleStatus::Producer {
            phase: "P03/P14 (workflow settings changes invalidate grants)",
        },
        "Relevant workflow changes invalidate grants",
        None
    ),
    rule!(
        "D11-15",
        RuleStatus::Policy {
            phase: Some(
                "P03 (the approval engine binds the fingerprint but never treats it as authorisation)"
            ),
        },
        "The operation fingerprint is not authorisation",
        None
    ),
    rule!(
        "D11-16",
        RuleStatus::Partial {
            rest_in: "P06/P16/P17 (redaction of the disclosed categories before transmission)",
        },
        "Model transmission grants name the data categories they disclose",
        Some(approval::d11_16)
    ),
    rule!(
        "D11-17",
        RuleStatus::Partial {
            rest_in: "P04/P16 (redirect handling and private or loopback address policy when connecting)",
        },
        "Grants carry bounded destinations that fit their capability",
        Some(approval::d11_17)
    ),
    rule!(
        "D11-18",
        I,
        "Timestamps are ordered",
        Some(evidence::d11_18)
    ),
    rule!(
        "D11-19",
        I,
        "Chunk digest matches the sanitised chunk text",
        Some(evidence::d11_19)
    ),
    rule!(
        "D11-20",
        I,
        "Line ranges are ordered and commit IDs are present when required",
        Some(ids::d11_20)
    ),
    rule!(
        "D11-21",
        RuleStatus::Partial {
            rest_in: "P07 (counters against the inventory and across reports)",
        },
        "Coverage counters agree with check records and snapshots",
        Some(evidence::d11_21)
    ),
    rule!(
        "D11-22",
        I,
        "Findings have a source or the report states a limitation",
        Some(ids::d11_22)
    ),
    rule!(
        "D11-23",
        I,
        "The provider credential reference is a locator",
        Some(ai::d11_23)
    ),
    rule!(
        "D11-24",
        RuleStatus::Partial {
            rest_in: "P17 (opt-in chat persistence, retention and retrieval isolation)",
        },
        "Chat persistence is off and conversations stay in their context",
        Some(ai::d11_24)
    ),
    rule!(
        "D11-25",
        RuleStatus::Partial {
            rest_in: "persistence::writer (T10: halt when a checkpoint cannot be written)",
        },
        "Storage failures are recorded as errors and resource limits are bounded",
        Some(evidence::d11_25)
    ),
    rule!(
        "D18-01",
        RuleStatus::Partial {
            rest_in: "P06/P13 (installed version, command mapping and binary identity at preflight)",
        },
        "Tool manifests describe a verifiable, bounded tool",
        Some(workflow::d18_01)
    ),
    rule!(
        "D18-02",
        RuleStatus::Producer {
            phase: "P07 (resolution of adapter IDs against the capability policy)",
        },
        "Adapter IDs narrow the approved capability policy",
        None
    ),
    rule!(
        "D18-03",
        RuleStatus::Partial {
            rest_in: "P06/P13 (enforcement by tool configuration and isolation)",
        },
        "Tool network policy agrees with its network resources",
        Some(workflow::d18_03)
    ),
    rule!(
        "D18-04",
        RuleStatus::Partial {
            rest_in: "P06 (parser agreement with the native exit mapping)",
        },
        "Exit codes are evidence and the native outcome is consistent",
        Some(workflow::d18_04)
    ),
    rule!(
        "D18-05",
        RuleStatus::Producer {
            phase: "P06 (core-owned bounded control channels)",
        },
        "Control-channel IDs resolve to core-owned streams",
        None
    ),
    rule!(
        "D18-06",
        RuleStatus::Producer {
            phase: "P06 (wrapper request validation; not a store root)",
        },
        "Wrapper requests carry no key or command string",
        None
    ),
    rule!(
        "D18-07",
        RuleStatus::Partial {
            rest_in: "P05/P07 (context disclosure and working-tree digests)",
        },
        "File selections and generated artifacts keep their identity",
        Some(workflow::d18_07)
    ),
    rule!(
        "D18-08",
        RuleStatus::Partial {
            rest_in: "P06/P07 (redaction of tool output and secret fingerprints)",
        },
        "Retained evidence is redacted content only",
        Some(evidence::d18_08)
    ),
    rule!(
        "D18-09",
        I,
        "Session-only credentials have no stored reference",
        Some(ai::d18_09)
    ),
    rule!(
        "D18-10",
        I,
        "Model transmission grants carry an inference scope, others none",
        Some(approval::d18_10)
    ),
    rule!(
        "D18-11",
        I,
        "The privacy policy is consistent wherever it is recorded",
        Some(ai::d18_11)
    ),
    rule!(
        "D18-12",
        I,
        "AI enrichment status agrees with its content",
        Some(ai::d18_12)
    ),
    rule!(
        "D18-13",
        RuleStatus::Producer {
            phase: "P17 (narrative is never recorded as an engine finding)",
        },
        "AI narrative is not an engine-confirmed finding",
        None
    ),
    rule!(
        "D18-14",
        I,
        "A written PDF has verified output and no unresolved error",
        Some(evidence::d18_14)
    ),
    rule!(
        "D18-15",
        RuleStatus::Policy {
            phase: Some("P20 (exact-version licence review before release)"),
        },
        "The tool and licence list is provisional",
        None
    ),
    rule!(
        "D18-16",
        RuleStatus::Producer {
            phase: "P08/P09 (category distinctions in reports)",
        },
        "Style and validation categories stay distinct from security",
        None
    ),
    rule!(
        "D18-17",
        RuleStatus::Producer {
            phase: "P10 (overlap handling keeps all evidence origins)",
        },
        "Overlapping tools keep all evidence origins",
        None
    ),
    rule!(
        "D18-18",
        I,
        "Terminal reports leave no check pending and explain interrupted checks",
        Some(evidence::d18_18)
    ),
    rule!(
        "D18-06a",
        I,
        "Deferred values are rejected with the deferred rule",
        Some(workflow::d18_06a)
    ),
];

/// Records a semantic problem of rule `rule` at `pointer`.
///
/// `pointer` is a JSON pointer made of schema property names and array indices only.
pub(super) fn flag(out: &mut Problems, rule: &'static str, pointer: &str) {
    out.push(Problem::new(
        Layer::Semantic,
        ProblemCode::Rule,
        pointer,
        Some(RuleId(rule)),
    ));
}

/// Flags the pointer `pointer(i)` for every item whose key occurred at an earlier position.
pub(super) fn duplicates<K: Eq + Hash>(
    out: &mut Problems,
    rule: &'static str,
    keys: impl IntoIterator<Item = K>,
    pointer: impl Fn(usize) -> String,
) {
    let mut seen = HashSet::new();
    for (i, key) in keys.into_iter().enumerate() {
        if !seen.insert(key) {
            flag(out, rule, &pointer(i));
        }
    }
}

/// Every plan snapshot of the document with its pointer prefix: the plan of a report, or the plan of each
/// run summary of a log. Other roots hold none.
pub(super) fn plans(doc: &AnyStore) -> Vec<(String, &PlanSnapshot)> {
    match doc {
        AnyStore::Report(r) => vec![(String::from("/plan"), &r.plan)],
        AnyStore::Log(l) => l
            .runs
            .iter()
            .enumerate()
            .map(|(i, run)| (format!("/runs/{i}/plan"), &run.plan))
            .collect(),
        _ => Vec::new(),
    }
}

/// Where a [`SourceRef`] sits in its document (indices only).
#[derive(Clone, Copy, Debug)]
pub(super) enum SrcLoc {
    /// `checks[i].source_refs[j]` of a report.
    Check(usize, usize),
    /// `findings[i].sources[j]` of a report.
    Finding(usize, usize),
    /// `artifacts[i].source_refs[j]` of a report.
    Artifact(usize, usize),
    /// `chunks[i].source` of a RAG index.
    Chunk(usize),
    /// `conversations[i].messages[j].source_refs[k]` of a conversation file.
    Message(usize, usize, usize),
}

impl SrcLoc {
    /// The JSON pointer of the source reference.
    pub(super) fn pointer(self) -> String {
        match self {
            Self::Check(i, j) => format!("/checks/{i}/source_refs/{j}"),
            Self::Finding(i, j) => format!("/findings/{i}/sources/{j}"),
            Self::Artifact(i, j) => format!("/artifacts/{i}/source_refs/{j}"),
            Self::Chunk(i) => format!("/chunks/{i}/source"),
            Self::Message(i, j, k) => format!("/conversations/{i}/messages/{j}/source_refs/{k}"),
        }
    }
}

/// Every source reference of the document in document order: those of report checks, findings and
/// artifacts, of RAG chunks and of conversation messages. Other roots hold none.
pub(super) fn source_refs(doc: &AnyStore) -> Vec<(SrcLoc, &SourceRef)> {
    let mut found = Vec::new();
    match doc {
        AnyStore::Report(r) => {
            for (i, c) in r.checks.iter().enumerate() {
                found.extend(
                    c.source_refs
                        .iter()
                        .enumerate()
                        .map(|(j, s)| (SrcLoc::Check(i, j), s)),
                );
            }
            for (i, f) in r.findings.iter().enumerate() {
                found.extend(
                    f.sources
                        .iter()
                        .enumerate()
                        .map(|(j, s)| (SrcLoc::Finding(i, j), s)),
                );
            }
            for (i, a) in r.artifacts.iter().enumerate() {
                found.extend(
                    a.source_refs
                        .iter()
                        .enumerate()
                        .map(|(j, s)| (SrcLoc::Artifact(i, j), s)),
                );
            }
        }
        AnyStore::RagIndex(f) => {
            found.extend(
                f.chunks
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (SrcLoc::Chunk(i), &c.source)),
            );
        }
        AnyStore::Conversations(f) => {
            for (i, conv) in f.conversations.iter().enumerate() {
                for (j, m) in conv.messages.iter().enumerate() {
                    found.extend(
                        m.source_refs
                            .iter()
                            .enumerate()
                            .map(|(k, s)| (SrcLoc::Message(i, j, k), s)),
                    );
                }
            }
        }
        _ => {}
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `check` is `Some` exactly for implemented and partial rules, so a producer or policy rule can never
    /// run a body and an implemented rule can never be silently unchecked.
    #[test]
    fn a_check_function_exists_exactly_for_implemented_and_partial_rules() {
        for rule in REGISTRY {
            let checked = matches!(
                rule.info.status,
                RuleStatus::Implemented | RuleStatus::Partial { .. }
            );
            assert_eq!(rule.check.is_some(), checked, "{}", rule.info.id.0);
        }
    }
}
