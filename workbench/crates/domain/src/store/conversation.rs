//! ChatContext, Message, Conversation, ConversationFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W2, wave 1b, T06.
//! `rag/conversations.json` holds chat conversations with their redacted message text.

use serde::{Deserialize, Serialize};

use super::evidence::SourceRef;
use super::values::{ModelId, ProviderId};
use super::{ConversationsStoreType, SchemaVersion};
use crate::ids::{ApprovalId, ConversationId, FileId, FindingId, MessageId, RepoId, RunId};
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

closed_enum!(
    /// Who a chat message is from (`Message.role`).
    MessageRole, 3, {
        /// The user.
        User => "user",
        /// The model.
        Assistant => "assistant",
        /// The application, for example a notice that a response was cut off.
        SystemNotice => "system_notice",
    }
);

closed_enum!(
    /// How a chat message ended (`Message.status`).
    MessageStatus, 3, {
        /// Complete.
        Complete => "complete",
        /// Cancelled by the user.
        Cancelled => "cancelled",
        /// Failed.
        Failed => "failed",
    }
);

/// What a conversation may draw on (`$defs/ChatContext`).
///
/// Not enforced by the typed layer: that the referenced records exist (Data Schema §11, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatContext {
    /// Repositories in scope.
    pub repo_ids: Vec<RepoId>,
    /// Runs in scope.
    pub run_ids: Vec<RunId>,
    /// Findings in scope.
    pub finding_ids: Vec<FindingId>,
    /// Files in scope.
    pub file_ids: Vec<FileId>,
    /// Whether earlier runs of the repositories are in scope.
    pub include_history: bool,
}

/// One message of a conversation (`$defs/Message`).
///
/// Not enforced by the typed layer: cross-field semantic rules, for example which roles carry
/// `provider_id`, `model_id` and token counts (Data Schema §11, T09). Token counts carry no upper bound in
/// the schema and are `u64`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// Identifier of the message.
    pub message_id: MessageId,
    /// Who the message is from.
    pub role: MessageRole,
    /// The message text (class R: user input or model output).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub text_redacted: RedactedText,
    /// Evidence the message cites.
    pub source_refs: Vec<SourceRef>,
    /// Provider that produced the message, if a model did.
    pub provider_id: Option<ProviderId>,
    /// Model that produced the message, if a model did.
    pub model_id: Option<ModelId>,
    /// Approvals the transmission ran under.
    pub approval_ids: Vec<ApprovalId>,
    /// When the message was created.
    pub created_at: UtcTimestamp,
    /// How the message ended.
    pub status: MessageStatus,
    /// Prompt tokens, if the provider reported them.
    pub tokens_in: Option<u64>,
    /// Completion tokens, if the provider reported them.
    pub tokens_out: Option<u64>,
}

/// A chat conversation (`$defs/Conversation`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    /// Identifier of the conversation.
    pub conversation_id: ConversationId,
    /// Title (class R: derived from user input).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub title: RedactedText,
    /// What the conversation may draw on.
    pub context: ChatContext,
    /// When the conversation was created.
    pub created_at: UtcTimestamp,
    /// When the conversation last changed.
    pub updated_at: UtcTimestamp,
    /// The messages, oldest first.
    pub messages: Vec<Message>,
}

/// Root of `rag/conversations.json` (`$defs/ConversationFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFile {
    /// Schema contract version, always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Root kind, always `"conversations"`.
    pub store_type: ConversationsStoreType,
    /// When the file was last written.
    pub updated_at: UtcTimestamp,
    /// The conversations.
    pub conversations: Vec<Conversation>,
}
