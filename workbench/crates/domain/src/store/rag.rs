//! Chunk, RagIndexFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W2, wave 1b, T06.
//! `rag/index.json` holds the redacted text chunks that chat retrieval draws on.

use serde::{Deserialize, Serialize};

use super::evidence::SourceRef;
use super::values::{Digest, Identifier, RedactionVersion};
use super::{RagIndexStoreType, SchemaVersion};
use crate::ids::ChunkId;
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

/// One indexed piece of redacted evidence text (`$defs/Chunk`).
///
/// Not enforced by the typed layer: cross-field and cross-record semantic rules (Data Schema §11, T09),
/// for example that `content_digest` matches `text_redacted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    /// Identifier of the chunk.
    pub chunk_id: ChunkId,
    /// Evidence the text was taken from.
    pub source: SourceRef,
    /// The indexed text (class R: derived from repository and tool content).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub text_redacted: RedactedText,
    /// Digest of the indexed content.
    pub content_digest: Digest,
    /// Redaction version applied to the text.
    pub redaction_version: RedactionVersion,
    /// Key of the stored embedding, if one exists.
    pub embedding_key: Option<Identifier>,
    /// When the chunk was indexed.
    pub indexed_at: UtcTimestamp,
}

/// Root of `rag/index.json` (`$defs/RagIndexFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RagIndexFile {
    /// Schema contract version, always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Root kind, always `"rag_index"`.
    pub store_type: RagIndexStoreType,
    /// When the file was last written.
    pub updated_at: UtcTimestamp,
    /// The indexed chunks.
    pub chunks: Vec<Chunk>,
}
