//! Version 1.5 operational roots whose payload shapes are unchanged from 1.4.
//!
//! Separate root types retain the version when serializing; they do not convert
//! or validate an existing store. Schema and semantic validation precede use.

use serde::{Deserialize, Serialize};

use super::{
    ConfigStoreType, ConversationsStoreType, LogStoreType, RagIndexStoreType, SchemaVersion15,
    ToolManifestStoreType, WorkflowsStoreType,
};
use crate::time::UtcTimestamp;

macro_rules! root {
    ($name:ident, $marker:ty, { $($field:ident: $field_type:ty),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub schema_version: SchemaVersion15,
            pub store_type: $marker,
            pub updated_at: UtcTimestamp,
            $(pub $field: $field_type),+
        }
    };
}

root!(ConfigFile, ConfigStoreType, { config: super::config::Config });
root!(WorkflowFile, WorkflowsStoreType, { workflows: Vec<super::workflow::Workflow> });
root!(LogFile, LogStoreType, {
    runs: Vec<super::log::RunSummary>,
    events: Vec<super::log::Event>,
    reports: Vec<super::log::ReportManifest>,
});
root!(RagIndexFile, RagIndexStoreType, { chunks: Vec<super::rag::Chunk> });
root!(ConversationFile, ConversationsStoreType, {
    conversations: Vec<super::conversation::Conversation>,
});
root!(ToolManifestFile, ToolManifestStoreType, { tools: Vec<super::tools::ToolManifest> });
