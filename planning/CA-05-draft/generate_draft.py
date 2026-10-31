"""Generates the CA-05 draft files in this directory (documentation-owned; not production schema).

Writes store-1.5-analysis-defs.draft.schema.json and examples.illustrative.json. Draft revision 2 (2026-10-31):
logical analysis vs attempt records, AI evidence coverage, pilot exclusion of AI-only findings, uncertainties,
O-AN-2 proposed limit fields. Usage (cwd = repo root): python3 -I -B planning/CA-05-draft/generate_draft.py
"""
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent
ID = {"$ref": "#/$defs/Id"}
TS = {"$ref": "#/$defs/Timestamp"}
DG = {"$ref": "#/$defs/Digest"}
TXT = {"type": "string", "minLength": 1, "maxLength": 4000}
IDS = {"type": "array", "items": ID, "uniqueItems": True}
CITS = {"type": "array", "items": {"$ref": "#/$defs/AnalysisCitation"}}
CITS1 = {"type": "array", "items": {"$ref": "#/$defs/AnalysisCitation"}, "minItems": 1}


def nullable(r):
    return {"anyOf": [r, {"type": "null"}]}


def obj(props, req=None, **extra):
    return {"type": "object", "additionalProperties": False, "properties": props,
            "required": list(props) if req is None else req, **extra}


def count():
    return {"type": "integer", "minimum": 0}


TERMINAL = ["completed", "blocked", "failed", "cancelled", "interrupted", "not_performed"]
REASONS = ["no_provider_configured", "no_credential", "approval_missing", "approval_denied", "evidence_unavailable",
           "provider_auth_failed", "provider_credit_exhausted", "provider_rate_limited", "provider_unavailable",
           "no_compliant_route", "uncertain_generation", "truncated_output", "invalid_output", "unsupported_citations",
           "context_budget_exceeded", "request_limit_reached", "cost_limit_reached", "cancelled_by_user",
           "application_interrupted", "persistence_failed", "insufficient_evidence"]

DEFS = {
    "AnalysisStatus": {"type": "string", "enum": ["pending", "awaiting_approval", "running", "validating"] + TERMINAL},
    "AnalysisTerminalStatus": {"type": "string", "enum": TERMINAL},
    "AnalysisReason": {"type": "string", "enum": REASONS},
    "AnalysisCitation": obj({"kind": {"type": "string", "enum": ["finding", "check", "source"]}, "id": ID}),
    "EvidenceCounts": obj({"findings": count(), "checks": count(), "coverage_gaps": count(),
                           "execution_failures": count(), "excerpts": count()}),
    "EvidenceSnapshot": obj({
        "digest": DG, "packet_format_version": {"type": "string", "minLength": 1},
        "finding_ids": IDS, "check_ids": IDS, "source_ids": IDS,
        "eligible": {"$ref": "#/$defs/EvidenceCounts"},
        "included": {"$ref": "#/$defs/EvidenceCounts"},
        "excerpt_bytes": count(), "packet_bytes": count(),
        "exclusions": {"type": "array", "items": obj({
            "kind": {"type": "string", "enum": ["finding", "check", "excerpt", "document"]},
            "reason": {"type": "string", "enum": ["context_budget", "not_eligible", "redaction_withheld", "outside_scope"]},
            "count": {"type": "integer", "minimum": 1}})}}),
    "ProviderRequest": obj({
        "request_seq": {"type": "integer", "minimum": 1, "maximum": 4},
        "kind": {"type": "string", "enum": ["initial", "transport_retry", "repair"]},
        "started_at": TS, "finished_at": nullable(TS),
        "outcome": {"type": "string", "enum": ["response_received", "rejected_by_provider", "rate_limited",
                                                "provider_unavailable", "failed_before_send", "uncertain_generation",
                                                "truncated", "cancelled", "interrupted"]},
        "http_status": nullable({"type": "integer", "minimum": 100, "maximum": 599}),
        "retry_after_seconds": nullable({"type": "integer", "minimum": 0})}),
    "AnalysisUsage": obj({"source": {"type": "string", "enum": ["provider_reported", "not_reported"]},
                          "input_tokens": nullable(count()), "output_tokens": nullable(count()),
                          "reported_cost": nullable({"type": "number", "minimum": 0}),
                          "reported_cost_unit": nullable({"type": "string", "minLength": 1})}),
    "AnalysisValidation": obj({
        "outcome": {"type": "string", "enum": ["passed", "passed_with_rejections", "failed", "not_run"]},
        "rejected_items": {"type": "array", "items": obj({
            "reason": {"type": "string", "enum": ["unsupported_citation", "schema_violation", "unsupported_assertion",
                                                   "outcome_override_attempt", "ai_only_finding_out_of_scope"]},
            "count": {"type": "integer", "minimum": 1}})}}),
    "AnalysisContent": obj({
        "evidence_sufficiency": {"type": "string", "enum": ["sufficient", "limited", "insufficient"]},
        "summary": obj({"text_redacted": TXT, "citations": CITS1}),
        "interpretations": {"type": "array", "items": obj({
            "interpretation_id": ID, "origin": {"const": "ai_interpretation"}, "text_redacted": TXT,
            "finding_ids": IDS, "check_ids": IDS, "citations": CITS1})},
        "recommendations": {"type": "array", "items": obj({
            "recommendation_id": ID, "origin": {"const": "ai_interpretation"},
            "priority": {"type": "string", "enum": ["high", "medium", "low"]}, "rationale_redacted": TXT,
            "finding_ids": IDS, "check_ids": IDS, "suggested_remediation_redacted": TXT,
            "verification_steps_redacted": {"type": "array", "items": TXT},
            "verification_status": {"const": "not_executed"}, "citations": CITS1})},
        "limitations": {"type": "array", "items": obj({"text_redacted": TXT, "check_ids": IDS, "citations": CITS1})},
        "uncertainties": {"type": "array", "items": obj({
            "kind": {"type": "string", "enum": ["open_question", "ambiguous_evidence", "missing_context"]},
            "text_redacted": TXT, "citations": CITS})},
        "observations": {"type": "array", "maxItems": 0, "items": obj({
            "observation_id": ID, "origin": {"const": "ai_observation"},
            "review_status": {"type": "string", "enum": ["unreviewed", "acknowledged", "dismissed"]},
            "text_redacted": TXT, "citations": CITS1}),
            "description": "Reserved for independent AI-only observations. Out of the pilot (owner, 2026-10-31): must be empty; the validator rejects such items (ai_only_finding_out_of_scope)."}}),
    "AnalysisFile": obj({
        "schema_version": {"const": "1.5"}, "store_type": {"const": "analysis"}, "updated_at": TS,
        "record_id": ID, "analysis_id": ID,
        "analysis_version": {"type": "integer", "minimum": 1},
        "attempt_seq": {"type": "integer", "minimum": 1},
        "supersedes_analysis_id": nullable(ID),
        "trigger": {"type": "string", "enum": ["automatic", "explicit_retry", "explicit_reanalysis"]},
        "run_id": ID, "repo_id": ID, "report_id": ID, "report_json_digest": DG,
        "commit_oid": nullable({"$ref": "#/$defs/GitOid"}),
        "status": {"$ref": "#/$defs/AnalysisTerminalStatus"},
        "reason_code": nullable({"$ref": "#/$defs/AnalysisReason"}), "reason_redacted": nullable({"type": "string"}),
        "idempotency_key": DG,
        "evidence_snapshot": nullable({"$ref": "#/$defs/EvidenceSnapshot"}),
        "ai_evidence_coverage": {"type": "string", "enum": ["complete", "partial", "none"]},
        "data_categories": {"type": "array", "uniqueItems": True, "items": {"type": "string", "enum": [
            "repository_metadata", "redacted_source", "redacted_findings", "schema_documents", "app_metadata"]}},
        "approval_ids": IDS, "inference_scope": nullable({"$ref": "#/$defs/InferenceScope"}),
        "provider_id": nullable({"const": "openrouter"}), "requested_model": nullable({"type": "string"}),
        "actual_model": nullable({"type": "string"}), "actual_provider": nullable({"type": "string"}),
        "routing_policy": nullable({"$ref": "#/$defs/ProviderPolicy"}),
        "prompt_template_version": {"type": "string", "minLength": 1},
        "analysis_schema_version": {"type": "string", "minLength": 1},
        "redaction_version": {"type": "string", "minLength": 1},
        "started_at": nullable(TS), "finished_at": TS,
        "provider_requests": {"type": "array", "items": {"$ref": "#/$defs/ProviderRequest"}, "maxItems": 4},
        "usage": {"$ref": "#/$defs/AnalysisUsage"}, "validation": {"$ref": "#/$defs/AnalysisValidation"},
        "content": nullable({"$ref": "#/$defs/AnalysisContent"})}),
    "AnalysisAttemptRef": obj({"attempt_seq": {"type": "integer", "minimum": 1}, "record_id": ID,
                               "status": {"$ref": "#/$defs/AnalysisTerminalStatus"},
                               "record_path": {"type": "string"}, "record_digest": DG}),
    "AnalysisManifest": obj({
        "analysis_id": ID, "report_id": ID, "repo_id": ID, "analysis_version": {"type": "integer", "minimum": 1},
        "supersedes_analysis_id": nullable(ID), "status": {"$ref": "#/$defs/AnalysisStatus"},
        "reason_code": nullable({"$ref": "#/$defs/AnalysisReason"}),
        "attempts": {"type": "array", "items": {"$ref": "#/$defs/AnalysisAttemptRef"}},
        "completed_record_id": nullable(ID), "updated_at": TS}),
    "AssessmentCompletion": obj({"state": {"type": "string", "enum": ["in_progress", "complete", "incomplete"]},
                                 "incomplete_stages": {"type": "array", "uniqueItems": True, "items": {
                                     "type": "string", "enum": ["scanner", "analysis", "report"]}}}),
    "AnalysisSettings": obj({
        "max_snapshot_bytes": {"type": "integer", "minimum": 4096, "maximum": 262144},
        "max_excerpts": {"type": "integer", "minimum": 0, "maximum": 100},
        "max_excerpt_bytes": {"type": "integer", "minimum": 256, "maximum": 8192},
        "max_output_tokens": {"type": "integer", "minimum": 256, "maximum": 16384},
        "max_response_bytes": {"type": "integer", "minimum": 16384, "maximum": 1048576},
        "connect_timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 60},
        "first_byte_timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 300},
        "total_timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 600},
        "max_transport_retries": {"type": "integer", "minimum": 0, "maximum": 2},
        "max_retry_after_seconds": {"type": "integer", "minimum": 0, "maximum": 120},
        "max_repair_attempts": {"type": "integer", "minimum": 0, "maximum": 1},
        "max_logical_analyses_per_run": {"type": "integer", "minimum": 1, "maximum": 50},
        "max_provider_requests_per_run": {"type": "integer", "minimum": 1, "maximum": 200},
        "max_reported_cost_per_run": nullable({"type": "number", "minimum": 0}),
        "reported_cost_unit": nullable({"type": "string", "minLength": 1})}),
}

SCHEMA = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$comment": ("DRAFT store 1.5 definitions proposed by CA-05 (planning/CA-05-automatic-evidence-analysis.md), "
                 "revision 2, 2026-10-31. NOT applied: the canonical store schema remains "
                 "schemas/workbench-store-v1.4.schema.json. Unchanged 1.4 $defs (Id, Timestamp, Digest, GitOid, "
                 "InferenceScope, ProviderPolicy) are referenced, not copied. Changes to existing 1.4 definitions are "
                 "listed in changes_to_existing and are applied only by an owner-approved lead contract task. "
                 "AnalysisSettings bounds are schema guard rails, not the proposed values (planning/CA-05-decision-proposals.md)."),
    "changes_to_existing": {
        "RunSummary": "add required analyses: AnalysisManifest[] (one per logical analysis) and completion: AssessmentCompletion",
        "ReportFile": "schema_version const 1.5; remove ai_enrichment (the scanner evidence report carries no AI content)",
        "ReportManifest": "add required export_revision (integer >= 1), analysis_id (Id|null), analysis_record_id (Id|null), analysis_version (integer >= 1 | null), analysis_status_at_render (AnalysisStatus|null)",
        "SourceRef.kind": "add enum value ai_analysis",
        "Config": "add required analysis: AnalysisSettings",
        "root oneOf": "add AnalysisFile; all store roots schema_version const 1.5",
        "AIEnrichment": "retained only for the 1.4 reader (legacy narrative); not referenced by 1.5 roots",
    },
    "$defs": DEFS,
}


def u(n):
    return "00000000-0000-4000-8000-%012d" % n


POL = {"data_collection": "deny", "zdr": True, "allow_fallbacks": False, "only": []}
F1, F2, C1, C2, C3, S1 = u(101), u(102), u(201), u(202), u(203), u(301)
NO_USAGE = {"source": "not_reported", "input_tokens": None, "output_tokens": None, "reported_cost": None,
            "reported_cost_unit": None}
COMMON = dict(schema_version="1.5", store_type="analysis", run_id=u(2), repo_id=u(3), report_id=u(4),
              report_json_digest="a" * 64, commit_oid="b" * 40,
              prompt_template_version="analysis-template-0.1-illustrative",
              analysis_schema_version="analysis-output-0.1-illustrative", redaction_version="redaction-0.1-illustrative")
SNAP = {"digest": "d" * 64, "packet_format_version": "evidence-packet-0.1-illustrative",
        "finding_ids": [F1, F2], "check_ids": [C1, C2, C3], "source_ids": [S1],
        "eligible": {"findings": 2, "checks": 3, "coverage_gaps": 1, "execution_failures": 0, "excerpts": 2},
        "included": {"findings": 2, "checks": 3, "coverage_gaps": 1, "execution_failures": 0, "excerpts": 1},
        "excerpt_bytes": 640, "packet_bytes": 5120,
        "exclusions": [{"kind": "excerpt", "reason": "redaction_withheld", "count": 1}]}
SCOPE = {"provider_id": "openrouter", "model_id": "owner-supplied-model-id", "routing_policy": POL,
         "source_ids": [S1], "redaction_version": "redaction-0.1-illustrative"}
PROVIDER = dict(provider_id="openrouter", requested_model="owner-supplied-model-id", actual_model=None,
                actual_provider=None, routing_policy=POL)

interrupted = dict(COMMON, updated_at="2026-11-02T10:06:00Z", record_id=u(31), analysis_id=u(11), analysis_version=1,
                   attempt_seq=1, supersedes_analysis_id=None, trigger="automatic", status="interrupted",
                   reason_code="application_interrupted",
                   reason_redacted="The application stopped while the request was in flight; it may have been billed.",
                   idempotency_key="c" * 64, evidence_snapshot=SNAP, ai_evidence_coverage="none",
                   data_categories=["redacted_findings", "redacted_source", "app_metadata"], approval_ids=[u(21)],
                   inference_scope=SCOPE, **PROVIDER, started_at="2026-11-02T10:05:00Z",
                   finished_at="2026-11-02T10:06:00Z",
                   provider_requests=[{"request_seq": 1, "kind": "initial", "started_at": "2026-11-02T10:05:00Z",
                                       "finished_at": None, "outcome": "interrupted", "http_status": None,
                                       "retry_after_seconds": None}],
                   usage=NO_USAGE, validation={"outcome": "not_run", "rejected_items": []}, content=None)
completed = dict(COMMON, updated_at="2026-11-02T11:00:40Z", record_id=u(32), analysis_id=u(11), analysis_version=1,
                 attempt_seq=2, supersedes_analysis_id=None, trigger="explicit_retry", status="completed",
                 reason_code=None, reason_redacted=None, idempotency_key="c" * 64, evidence_snapshot=SNAP,
                 ai_evidence_coverage="partial",
                 data_categories=["redacted_findings", "redacted_source", "app_metadata"], approval_ids=[u(21)],
                 inference_scope=SCOPE, **PROVIDER, started_at="2026-11-02T11:00:00Z",
                 finished_at="2026-11-02T11:00:39Z",
                 provider_requests=[{"request_seq": 1, "kind": "initial", "started_at": "2026-11-02T11:00:00Z",
                                     "finished_at": "2026-11-02T11:00:38Z", "outcome": "response_received",
                                     "http_status": 200, "retry_after_seconds": None}],
                 usage=NO_USAGE,
                 validation={"outcome": "passed_with_rejections", "rejected_items": [
                     {"reason": "unsupported_citation", "count": 1},
                     {"reason": "ai_only_finding_out_of_scope", "count": 1}]},
                 content={
                     "evidence_sufficiency": "limited",
                     "summary": {"text_redacted": "Illustrative: two scanner findings in one workflow file; the Kubernetes schema check was unsupported, so scanner coverage is partial; one excerpt was withheld by redaction, so AI evidence coverage is partial.",
                                 "citations": [{"kind": "finding", "id": F1}, {"kind": "finding", "id": F2}, {"kind": "check", "id": C3}]},
                     "interpretations": [{"interpretation_id": u(401), "origin": "ai_interpretation",
                                          "text_redacted": "Illustrative: both findings concern the same job's token permissions.",
                                          "finding_ids": [F1, F2], "check_ids": [C1],
                                          "citations": [{"kind": "finding", "id": F1}, {"kind": "source", "id": S1}]}],
                     "recommendations": [{"recommendation_id": u(501), "origin": "ai_interpretation", "priority": "high",
                                          "rationale_redacted": "Illustrative: the scanner rated the token-permission finding high.",
                                          "finding_ids": [F1], "check_ids": [],
                                          "suggested_remediation_redacted": "Illustrative: restrict the job's permissions to read-only.",
                                          "verification_steps_redacted": ["Illustrative: re-run the assessment and confirm the finding is absent with complete coverage."],
                                          "verification_status": "not_executed", "citations": [{"kind": "finding", "id": F1}]}],
                     "limitations": [{"text_redacted": "Illustrative: schema validation was unsupported for this file.",
                                      "check_ids": [C3], "citations": [{"kind": "check", "id": C3}]}],
                     "uncertainties": [{"kind": "missing_context", "text_redacted": "Illustrative: whether the referenced reusable workflow is in scope is unknown.", "citations": []},
                                       {"kind": "ambiguous_evidence", "text_redacted": "Illustrative: the withheld excerpt may change the priority of the second finding.", "citations": [{"kind": "finding", "id": F2}]}],
                     "observations": []})
blocked = dict(COMMON, updated_at="2026-11-02T10:01:00Z", record_id=u(33), analysis_id=u(12), analysis_version=1,
               attempt_seq=1, supersedes_analysis_id=None, trigger="automatic", status="blocked",
               reason_code="approval_denied", reason_redacted="Model transmission was denied for this scope.",
               idempotency_key="e" * 64, evidence_snapshot=dict(SNAP, exclusions=[]), ai_evidence_coverage="none",
               data_categories=[], approval_ids=[u(22)], inference_scope=None, **PROVIDER, started_at=None,
               finished_at="2026-11-02T10:01:00Z", provider_requests=[], usage=NO_USAGE,
               validation={"outcome": "not_run", "rejected_items": []}, content=None)
APATH = "analysis/20261102T%sZ_example-repo_00000000-0000-4000-8000-000000000002_%s.json"
EXAMPLES = {
    "$comment": "ILLUSTRATIVE examples for the CA-05 draft revision 2 (not real records, produced by no implementation). IDs, digests, model and version strings are placeholders. One logical analysis (analysis_id ...011) has an interrupted attempt 1 and a completed attempt 2 after an explicit retry; a second report's analysis (...012) is blocked.",
    "attempt1_interrupted": interrupted,
    "attempt2_completed": completed,
    "blocked_approval_denied": blocked,
    "run_summary_fragment": {
        "analyses": [{"analysis_id": u(11), "report_id": u(4), "repo_id": u(3), "analysis_version": 1,
                      "supersedes_analysis_id": None, "status": "completed", "reason_code": None,
                      "attempts": [{"attempt_seq": 1, "record_id": u(31), "status": "interrupted",
                                    "record_path": APATH % ("100600", "a1-t1"), "record_digest": "1" * 64},
                                   {"attempt_seq": 2, "record_id": u(32), "status": "completed",
                                    "record_path": APATH % ("110040", "a1-t2"), "record_digest": "2" * 64}],
                      "completed_record_id": u(32), "updated_at": "2026-11-02T11:00:40Z"}],
        "completion": {"state": "complete", "incomplete_stages": []}},
    "run_summary_fragment_blocked": {
        "analyses": [{"analysis_id": u(12), "report_id": u(4), "repo_id": u(3), "analysis_version": 1,
                      "supersedes_analysis_id": None, "status": "blocked", "reason_code": "approval_denied",
                      "attempts": [{"attempt_seq": 1, "record_id": u(33), "status": "blocked",
                                    "record_path": APATH % ("100100", "a1-t1"), "record_digest": "9" * 64}],
                      "completed_record_id": None, "updated_at": "2026-11-02T10:01:00Z"}],
        "completion": {"state": "incomplete", "incomplete_stages": ["analysis"]}},
}

(OUT / "store-1.5-analysis-defs.draft.schema.json").write_text(json.dumps(SCHEMA, indent=2) + "\n")
(OUT / "examples.illustrative.json").write_text(json.dumps(EXAMPLES, indent=2) + "\n")
print(f"{len(DEFS)} defs; examples: {', '.join(k for k in EXAMPLES if not k.startswith('$'))}")
