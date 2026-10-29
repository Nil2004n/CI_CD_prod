//! Semantic rule bodies for the model provider, AI enrichment and chat (Data Schema §11 rules 23, 24 and §18
//! rules 9, 11, 12).
//!
//! * `D11-23` the provider credential reference is a locator and OS mode has one.
//! * `D11-24` chat persistence is off in the pilot and conversations stay inside their context.
//! * `D18-09` session-only credentials have no stored reference.
//! * `D18-11` the privacy policy is recorded consistently.
//! * `D18-12` AI enrichment status agrees with its content.
//!
//! Every function is pure over one [`AnyStore`] and ignores the roots its rule does not concern.

use std::collections::HashSet;

use domain::store::AnyStore;
use domain::store::conversation::MessageRole;
use domain::store::log::EventType;
use domain::store::provider::{ProviderCredentialMode, ProviderPolicy};
use domain::store::report::EnrichmentStatus;

use super::rules::{duplicates, flag, source_refs};
use crate::validate::Problems;

/// Rule `D11-23`: the provider's credential reference is an approved locator, never a key. The type
/// (`CredentialRef`) rejects key-like values; additionally a provider in OS credential-store mode has a
/// reference to look up (the opaque OS locator), because there is nothing else to resolve the secret from.
pub(crate) fn d11_23(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-23";
    let AnyStore::Config(f) = doc else { return };
    if let Some(p) = &f.config.provider
        && p.credential_mode == ProviderCredentialMode::OsCredentialStore
        && p.credential_reference.is_none()
    {
        flag(out, R, "/config/provider/credential_reference");
    }
}

/// Whether `event` is one of the chat events, which are never persisted to the run log (T01-D9).
fn is_chat_event(event: EventType) -> bool {
    matches!(
        event,
        EventType::ChatRetrievalCompleted
            | EventType::ChatResponseStarted
            | EventType::ChatResponseFinished
            | EventType::ChatResponseChunk
            | EventType::ChatFailed
    )
}

/// Rule `D11-24` (store part): chat persistence is opt-in and, in the pilot, deferred.
///
/// `chat_persistence_enabled` is not true (PRD, deferred chat persistence); no chunk carries an embedding key
/// (embedding transmission is deferred); the run log holds no chat event. Conversations: only an assistant
/// message carries a provider, a model, token counts or approvals, and an assistant message with a provider
/// names its approvals; a message cites sources of the repositories in its conversation's context only
/// (context isolation). Retention and retrieval isolation across runs are P17.
pub(crate) fn d11_24(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-24";
    match doc {
        AnyStore::Config(f) => {
            if f.config.chat_persistence_enabled {
                flag(out, R, "/config/chat_persistence_enabled");
            }
        }
        AnyStore::RagIndex(f) => {
            for (i, c) in f.chunks.iter().enumerate() {
                if c.embedding_key.is_some() {
                    flag(out, R, &format!("/chunks/{i}/embedding_key"));
                }
            }
        }
        AnyStore::Log(f) => {
            for (i, e) in f.events.iter().enumerate() {
                if is_chat_event(e.event_type) {
                    flag(out, R, &format!("/events/{i}/event_type"));
                }
            }
        }
        AnyStore::Conversations(f) => {
            for (i, conv) in f.conversations.iter().enumerate() {
                let context: HashSet<_> = conv.context.repo_ids.iter().copied().collect();
                for (j, m) in conv.messages.iter().enumerate() {
                    let at = format!("/conversations/{i}/messages/{j}");
                    let assistant = m.role == MessageRole::Assistant;
                    if !assistant {
                        for (field, set) in [
                            ("provider_id", m.provider_id.is_some()),
                            ("model_id", m.model_id.is_some()),
                            ("tokens_in", m.tokens_in.is_some()),
                            ("tokens_out", m.tokens_out.is_some()),
                            ("approval_ids", !m.approval_ids.is_empty()),
                        ] {
                            if set {
                                flag(out, R, &format!("{at}/{field}"));
                            }
                        }
                    } else if m.provider_id.is_some() && m.approval_ids.is_empty() {
                        flag(out, R, &format!("{at}/approval_ids"));
                    }
                    for (k, s) in m.source_refs.iter().enumerate() {
                        if s.repo_id.is_some_and(|id| !context.contains(&id)) {
                            flag(out, R, &format!("{at}/source_refs/{k}/repo_id"));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Rule `D18-09`: a provider in session-only mode has a null credential reference, so nothing that could
/// resolve a stored secret is persisted. A supported secure store for OS mode is checked at configuration
/// time, never by falling back to plaintext.
pub(crate) fn d18_09(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-09";
    let AnyStore::Config(f) = doc else { return };
    if let Some(p) = &f.config.provider
        && p.credential_mode == ProviderCredentialMode::SessionOnly
        && p.credential_reference.is_some()
    {
        flag(out, R, "/config/provider/credential_reference");
    }
}

/// `D18-11` for a recorded policy at `at`: the provider allowlist lists each provider once. The privacy
/// constants (deny collection, require ZDR, no fallback) are enforced by the policy's types.
fn policy_consistent(at: &str, policy: &ProviderPolicy, out: &mut Problems) {
    duplicates(out, "D18-11", policy.only.iter().map(|p| p.as_str()), |j| {
        format!("{at}/only/{j}")
    });
}

/// Rule `D18-11`: the privacy policy is recorded consistently wherever it appears: the configured provider,
/// an approval's inference scope and the AI enrichment of a report each list a provider at most once in
/// `only`, and an enrichment records its routing policy exactly when it records a provider (a request that
/// was made); a requested model needs a provider. The constants themselves are typed.
pub(crate) fn d18_11(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-11";
    match doc {
        AnyStore::Config(f) => {
            if let Some(p) = &f.config.provider {
                policy_consistent("/config/provider/routing_policy", &p.routing_policy, out);
            }
        }
        AnyStore::Approvals(f) => {
            for (i, a) in f.approvals.iter().enumerate() {
                if let Some(scope) = &a.inference_scope {
                    policy_consistent(
                        &format!("/approvals/{i}/inference_scope/routing_policy"),
                        &scope.routing_policy,
                        out,
                    );
                }
            }
        }
        AnyStore::Report(r) => {
            let e = &r.ai_enrichment;
            if let Some(policy) = &e.routing_policy {
                policy_consistent("/ai_enrichment/routing_policy", policy, out);
            }
            if e.provider_id.is_some() != e.routing_policy.is_some() {
                flag(out, R, "/ai_enrichment/routing_policy");
            }
            if e.requested_model.is_some() && e.provider_id.is_none() {
                flag(out, R, "/ai_enrichment/provider_id");
            }
        }
        _ => {}
    }
}

/// Rule `D18-12`: the AI enrichment of a report agrees with its status.
///
/// `completed`: a non-empty narrative, a generation time, a provider, a requested model, the routing
/// policy, approvals, at least one cited source and no failure reason. Every other status: an empty
/// narrative, no generation time; `failed`, `unavailable` and `denied` give a non-empty reason;
/// `not_requested`, `unavailable` and `denied` name no answering model or provider (nothing was sent), and
/// `not_requested` records no provider, requested model, policy, approval or source at all. Every cited
/// source ID resolves to a source reference of the report. The actual provider and model may stay null and
/// are never inferred.
pub(crate) fn d18_12(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-12";
    let AnyStore::Report(r) = doc else { return };
    let e = &r.ai_enrichment;
    let at = "/ai_enrichment";
    let mut want = |ok: bool, field: &str| {
        if !ok {
            flag(out, R, &format!("{at}/{field}"));
        }
    };
    let has_reason = e.reason_redacted.as_ref().is_some_and(|t| !t.is_empty());
    match e.status {
        EnrichmentStatus::Completed => {
            want(!e.narrative_redacted.is_empty(), "narrative_redacted");
            want(e.generated_at.is_some(), "generated_at");
            want(e.provider_id.is_some(), "provider_id");
            want(e.requested_model.is_some(), "requested_model");
            want(e.routing_policy.is_some(), "routing_policy");
            want(!e.approval_ids.is_empty(), "approval_ids");
            want(!e.source_ids.is_empty(), "source_ids");
            want(e.reason_redacted.is_none(), "reason_redacted");
        }
        status => {
            want(e.narrative_redacted.is_empty(), "narrative_redacted");
            want(e.generated_at.is_none(), "generated_at");
            if matches!(
                status,
                EnrichmentStatus::Failed | EnrichmentStatus::Unavailable | EnrichmentStatus::Denied
            ) {
                want(has_reason, "reason_redacted");
            }
            if status != EnrichmentStatus::Failed {
                want(e.actual_model.is_none(), "actual_model");
                want(e.actual_provider.is_none(), "actual_provider");
            }
            if status == EnrichmentStatus::NotRequested {
                want(e.provider_id.is_none(), "provider_id");
                want(e.requested_model.is_none(), "requested_model");
                want(e.routing_policy.is_none(), "routing_policy");
                want(e.approval_ids.is_empty(), "approval_ids");
                want(e.source_ids.is_empty(), "source_ids");
            }
        }
    }
    let known: HashSet<_> = source_refs(doc)
        .into_iter()
        .map(|(_, s)| s.source_id)
        .collect();
    for (j, id) in e.source_ids.iter().enumerate() {
        if !known.contains(id) {
            flag(out, R, &format!("{at}/source_ids/{j}"));
        }
    }
}
