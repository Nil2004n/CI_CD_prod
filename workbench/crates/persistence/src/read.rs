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

pub(crate) fn migration_manifest(bytes: &[u8]) -> Option<crate::migration15::Manifest> {
    if bytes.len() > 8192 {
        return None;
    }
    serde_json::from_slice(bytes).ok()
}

pub(crate) fn claims_version15(bytes: &[u8]) -> bool {
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_DOCUMENT_BYTES) {
        return false;
    }
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .get("schema_version")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .as_deref()
        == Some("1.5")
}

/// Decode a 1.4 historical or 1.5 operational root using its own validator.
/// This is a read-only building block; it does not enable versioned store writes.
pub fn decode_versioned(
    kind: StoreKind,
    bytes: &[u8],
    legacy: &dyn DocumentValidator,
) -> Result<domain::store::VersionedStore, StoreError> {
    use domain::store::{OperationalStore15, VersionedStore};

    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_DOCUMENT_BYTES) {
        return Err(StoreError::Invalid(
            kind,
            Problems::one(Problem::new(Layer::Json, ProblemCode::TooLarge, "", None)),
        ));
    }
    let doc: Value = serde_json::from_slice(bytes).map_err(|_| {
        StoreError::Invalid(
            kind,
            Problems::one(Problem::new(Layer::Json, ProblemCode::Syntax, "", None)),
        )
    })?;
    if doc.get("schema_version").and_then(Value::as_str) != Some("1.5") {
        return decode(kind, bytes, legacy).map(VersionedStore::V14);
    }
    let value = match kind {
        StoreKind::Repositories => OperationalStore15::Repositories(decode_repository15(bytes)?),
        StoreKind::Approvals => {
            match crate::schema::validate_approval15(&doc) {
                Verdict::Valid => {}
                Verdict::Invalid(problems) => return Err(StoreError::Invalid(kind, problems)),
                Verdict::SchemaUnavailable => return Err(StoreError::SchemaUnavailable),
            }
            let file = serde_json::from_value(doc).map_err(|_| {
                StoreError::Invalid(
                    kind,
                    Problems::one(Problem::new(
                        Layer::Typed,
                        ProblemCode::TypedDecode,
                        "",
                        None,
                    )),
                )
            })?;
            OperationalStore15::Approvals(file)
        }
        StoreKind::Report | StoreKind::Suppressions => {
            return Err(StoreError::Invalid(
                kind,
                Problems::one(Problem::new(
                    Layer::Version,
                    ProblemCode::UnsupportedVersion(VersionClass::NewerOrUnknown),
                    "/schema_version",
                    None,
                )),
            ));
        }
        _ => decode_unchanged15(kind, bytes)?,
    };
    Ok(VersionedStore::V15(value))
}

/// Prepare and fully validate one 1.4 operational root as a 1.5 candidate.
///
/// This function performs no I/O and is not a migration: the caller must
/// preflight the complete store and use crash-safe recovery before publishing.
/// Existing fields are preserved; incompatible legacy values are rejected.
pub fn prepare_operational15(
    kind: StoreKind,
    bytes: &[u8],
    legacy: &dyn DocumentValidator,
) -> Result<Vec<u8>, StoreError> {
    if !kind.writable() || matches!(kind, StoreKind::Report | StoreKind::Suppressions) {
        return Err(StoreError::PathRejected);
    }
    // Do not use the caller-supplied validator as a bypass for the canonical
    // legacy semantic and schema checks.
    let _ = decode(kind, bytes, legacy)?;
    let _ = decode(kind, bytes, &SchemaValidator::new())?;
    let invalid = || {
        StoreError::Invalid(
            kind,
            Problems::one(Problem::new(
                Layer::Typed,
                ProblemCode::TypedDecode,
                "",
                None,
            )),
        )
    };
    let mut doc: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let root = doc.as_object_mut().ok_or_else(invalid)?;
    root.insert("schema_version".to_owned(), Value::String("1.5".to_owned()));
    let (collection, property) = match kind {
        StoreKind::Repositories => (Some("repositories"), Some("last_git_version")),
        StoreKind::Approvals => (Some("approvals"), Some("git_approval_context")),
        _ => (None, None),
    };
    if let (Some(collection), Some(property)) = (collection, property) {
        let entries = root
            .get_mut(collection)
            .and_then(Value::as_array_mut)
            .ok_or_else(invalid)?;
        for entry in entries {
            entry
                .as_object_mut()
                .ok_or_else(invalid)?
                .insert(property.to_owned(), Value::Null);
        }
    }
    let candidate = serde_json::to_vec(&doc).map_err(|_| invalid())?;
    if !matches!(
        decode_versioned(kind, &candidate, legacy)?,
        domain::store::VersionedStore::V15(_)
    ) {
        return Err(invalid());
    }
    Ok(candidate)
}

/// Decode a schema- and semantics-validated 1.5 repository document.
/// This standalone reader does not enable migration or store writes.
pub fn decode_repository15(
    bytes: &[u8],
) -> Result<domain::store::repository15::RepositoryFile, StoreError> {
    let kind = StoreKind::Repositories;
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
    match crate::schema::validate_repository15(&doc) {
        Verdict::Valid => {}
        Verdict::Invalid(problems) => return Err(invalid(problems)),
        Verdict::SchemaUnavailable => return Err(StoreError::SchemaUnavailable),
    }
    serde_json::from_value(doc).map_err(|_| {
        invalid(Problems::one(Problem::new(
            Layer::Typed,
            ProblemCode::TypedDecode,
            "",
            None,
        )))
    })
}

/// Decode an unchanged operational root using the 1.5 schema and 1.4 semantics.
///
/// This does not replace [`decode`] or enable store writes. Repository and
/// approval roots need their own version-specific semantic checks.
pub fn decode_unchanged15(
    kind: StoreKind,
    bytes: &[u8],
) -> Result<domain::store::OperationalStore15, StoreError> {
    use domain::store::{OperationalStore15, roots15};
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
    match crate::schema::validate_unchanged15(kind, &doc) {
        Verdict::Valid => {}
        Verdict::Invalid(problems) => return Err(invalid(problems)),
        Verdict::SchemaUnavailable => return Err(StoreError::SchemaUnavailable),
    }
    fn parse<T: DeserializeOwned>(doc: Value, kind: StoreKind) -> Result<T, StoreError> {
        serde_json::from_value(doc).map_err(|_| {
            StoreError::Invalid(
                kind,
                Problems::one(Problem::new(
                    Layer::Typed,
                    ProblemCode::TypedDecode,
                    "",
                    None,
                )),
            )
        })
    }
    match kind {
        StoreKind::Config => Ok(OperationalStore15::Config(parse::<roots15::ConfigFile>(
            doc, kind,
        )?)),
        StoreKind::Workflows => Ok(OperationalStore15::Workflows(
            parse::<roots15::WorkflowFile>(doc, kind)?,
        )),
        StoreKind::Log => Ok(OperationalStore15::Log(parse::<roots15::LogFile>(
            doc, kind,
        )?)),
        StoreKind::RagIndex => Ok(OperationalStore15::RagIndex(
            parse::<roots15::RagIndexFile>(doc, kind)?,
        )),
        StoreKind::Conversations => Ok(OperationalStore15::Conversations(parse::<
            roots15::ConversationFile,
        >(doc, kind)?)),
        StoreKind::ToolManifest => Ok(OperationalStore15::ToolManifest(parse::<
            roots15::ToolManifestFile,
        >(doc, kind)?)),
        _ => Err(StoreError::PathRejected),
    }
}

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
