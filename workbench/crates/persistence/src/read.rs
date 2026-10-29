//! decode and SchemaValidator: the only place store documents are deserialised (request §5.6, §6.1).
//!
//! Lead-owned pipeline glue (P02 wave 2 preparation, request §5.12): size bound, JSON parse, object check,
//! `store_type` and `schema_version` gates, the [`DocumentValidator`] call and the typed decode into
//! [`AnyStore`]. The schema layer itself ([`SchemaValidator`]) is in [`crate::schema`] (W1, T08).
//!
//! **Read boundary (FR-31, E-3, request §5.11):** every failure is reported as fixed-text [`Problem`]s.
//! serde and validator messages are never stored, returned or logged, because they can quote document
//! values. Pointers are built from schema property names only, never from document keys or values.

use domain::store::approval::ApprovalFile;
use domain::store::config::ConfigFile;
use domain::store::conversation::ConversationFile;
use domain::store::log::LogFile;
use domain::store::rag::RagIndexFile;
use domain::store::report::ReportFile;
use domain::store::repository::RepositoryFile;
use domain::store::suppression::SuppressionFile;
use domain::store::tools::ToolManifestFile;
use domain::store::workflow::WorkflowFile;
use domain::store::{AnyStore, StoreKind};
use domain::versions::STORE_SCHEMA_VERSION;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::StoreError;
use crate::validate::{
    DocumentValidator, Layer, Problem, ProblemCode, Problems, Verdict, VersionClass,
};

pub use crate::schema::SchemaValidator;

/// Upper bound on a store file or report read from disk (development default, request §6.5 / T01-D5;
/// production limits follow the FR-11 owner decision).
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;

/// `schema_version` values of earlier store contracts (Data Schema §19). They are recognised only to
/// classify the rejection; no reader for them exists.
const OLDER_VERSIONS: [&str; 4] = ["1.0", "1.1", "1.2", "1.3"];

/// Decodes the bytes of a `kind` store file: size bound → JSON parse → object → `store_type` →
/// `schema_version` → `v.validate` (schema, typed and semantic layers) → typed decode.
///
/// Errors: [`StoreError::Invalid`] with fixed-text problems for every document fault,
/// [`StoreError::SchemaUnavailable`] when the validator cannot validate (never treated as valid).
pub fn decode(
    kind: StoreKind,
    bytes: &[u8],
    v: &dyn DocumentValidator,
) -> Result<AnyStore, StoreError> {
    let invalid = |p: Problems| StoreError::Invalid(kind, p);
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_DOCUMENT_BYTES) {
        return Err(invalid(Problems::one(Problem::new(
            Layer::Json,
            ProblemCode::TooLarge,
            "",
            None,
        ))));
    }
    let doc: Value = serde_json::from_slice(bytes).map_err(|_| {
        invalid(Problems::one(Problem::new(
            Layer::Json,
            ProblemCode::Syntax,
            "",
            None,
        )))
    })?;
    precheck(kind, &doc).map_err(invalid)?;
    match v.validate(kind, &doc) {
        Verdict::Valid => typed_decode(kind, doc).map_err(invalid),
        Verdict::Invalid(p) => Err(invalid(p)),
        Verdict::SchemaUnavailable => Err(StoreError::SchemaUnavailable),
    }
}

/// The object, `store_type` and `schema_version` gates of request §6.1, in that order. Shared by
/// [`decode`] and by every [`DocumentValidator`] (the writer validates without decoding bytes).
pub fn precheck(kind: StoreKind, doc: &Value) -> Result<(), Problems> {
    let Some(obj) = doc.as_object() else {
        return Err(Problems::one(Problem::new(
            Layer::Json,
            ProblemCode::NotObject,
            "",
            None,
        )));
    };
    match obj
        .get("store_type")
        .and_then(Value::as_str)
        .map(StoreKind::from_store_type)
    {
        Some(Some(found)) if found == kind => {}
        Some(Some(_)) => {
            return Err(Problems::one(Problem::new(
                Layer::Version,
                ProblemCode::StoreTypeMismatch,
                "/store_type",
                None,
            )));
        }
        _ => {
            return Err(Problems::one(Problem::new(
                Layer::Version,
                ProblemCode::UnknownStoreType,
                "/store_type",
                None,
            )));
        }
    }
    match version_class(obj.get("schema_version")) {
        None => Ok(()),
        Some(class) => Err(Problems::one(Problem::new(
            Layer::Version,
            ProblemCode::UnsupportedVersion(class),
            "/schema_version",
            None,
        ))),
    }
}

/// `None` for the supported `"1.4"`; otherwise the class of the rejection (request §6.1). A
/// non-string value is `NewerOrUnknown`; the value is never kept.
pub fn version_class(value: Option<&Value>) -> Option<VersionClass> {
    match value {
        None => Some(VersionClass::Missing),
        Some(Value::String(s)) if s == STORE_SCHEMA_VERSION => None,
        Some(Value::String(s)) if OLDER_VERSIONS.contains(&s.as_str()) => Some(VersionClass::Older),
        Some(_) => Some(VersionClass::NewerOrUnknown),
    }
}

/// Decodes a document that has passed the gates into the typed root of `kind`. A failure is a single
/// [`ProblemCode::TypedDecode`] problem at the root pointer (serde_json reports no instance path, and its
/// message can quote values, so it is dropped).
pub fn typed_decode(kind: StoreKind, doc: Value) -> Result<AnyStore, Problems> {
    fn root<T: DeserializeOwned>(doc: Value) -> Result<T, Problems> {
        serde_json::from_value(doc).map_err(|_| {
            Problems::one(Problem::new(
                Layer::Typed,
                ProblemCode::TypedDecode,
                "",
                None,
            ))
        })
    }
    Ok(match kind {
        StoreKind::Config => AnyStore::Config(root::<ConfigFile>(doc)?),
        StoreKind::Repositories => AnyStore::Repositories(root::<RepositoryFile>(doc)?),
        StoreKind::Workflows => AnyStore::Workflows(root::<WorkflowFile>(doc)?),
        StoreKind::Approvals => AnyStore::Approvals(root::<ApprovalFile>(doc)?),
        StoreKind::Log => AnyStore::Log(root::<LogFile>(doc)?),
        StoreKind::Suppressions => AnyStore::Suppressions(root::<SuppressionFile>(doc)?),
        StoreKind::RagIndex => AnyStore::RagIndex(root::<RagIndexFile>(doc)?),
        StoreKind::Conversations => AnyStore::Conversations(root::<ConversationFile>(doc)?),
        StoreKind::Report => AnyStore::Report(Box::new(root::<ReportFile>(doc)?)),
        StoreKind::ToolManifest => AnyStore::ToolManifest(root::<ToolManifestFile>(doc)?),
    })
}
