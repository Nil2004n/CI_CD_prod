//! Canonical operation fingerprint, encoding v1 (`planning/P03-T01-request.md` §5.3). Owner: W1, T05.

use domain::contract::DataCategory;
use domain::store::values::Digest;
use sha2::{Digest as _, Sha256};

use super::{ArgError, MAX_ARG_VALUE_BYTES, PermissionRequest, RequestedData, check_name};

/// Domain-separation prefix of encoding v1.
pub const FINGERPRINT_PREFIX: &[u8] = b"pwt-operation-fingerprint\x00v1\x00";

/// SHA-256 over the canonical encoding v1 of the request's operation content (fields F1–F7). Workflow
/// binding, run, lifetime, validity, purpose, inference scope, actor and timestamps are excluded; they are
/// matched as separate dimensions.
///
/// Encoding (all integers are `u32` big endian):
///
/// ```text
/// digest = SHA-256(FINGERPRINT_PREFIX || F1 || ... || F7)
/// str(s)   = len(s) || utf8(s)
/// list(xs) = count || item...      items sorted by their encoded bytes, duplicates removed, count after removal
/// F1 str(capability)   F2 str(operation)   F3 list(str(arg name) || str(arg value))   F4 list(str(repo id))
/// F5 list(str(destination canonical form))   F6 list(str(resource constraint))   F7 list(str(data category))
/// ```
///
/// Errors: an invalid operation or argument name (re-validated here), an argument value over
/// [`MAX_ARG_VALUE_BYTES`], or two arguments with the same name ([`ArgError::Duplicate`], whatever their
/// values). When several faults exist, names and value sizes are checked first (in argument order), then
/// duplicate names.
pub fn fingerprint(request: &PermissionRequest) -> Result<Digest, ArgError> {
    check_name(request.operation.as_str())?;

    let mut arguments: Vec<Vec<u8>> = Vec::with_capacity(request.arguments.len());
    for arg in &request.arguments {
        check_name(arg.name.as_str())?;
        if arg.value.len() > MAX_ARG_VALUE_BYTES {
            return Err(ArgError::TooLong);
        }
        let mut item = Vec::new();
        put_str(&mut item, arg.name.as_str())?;
        put_str(&mut item, &arg.value)?;
        arguments.push(item);
    }
    let mut names: Vec<&str> = request.arguments.iter().map(|a| a.name.as_str()).collect();
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ArgError::Duplicate);
    }

    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_PREFIX);

    let mut field = Vec::new();
    put_str(&mut field, request.capability.as_str())?;
    put_str(&mut field, request.operation.as_str())?;
    hasher.update(&field);

    let mut field = Vec::new();
    put_list(&mut field, arguments)?;
    hasher.update(&field);

    let repos = encode_items(request.repo_ids.iter().map(ToString::to_string))?;
    let destinations = encode_items(request.destinations.iter().map(|d| d.canonical()))?;
    let constraints = encode_items(
        request
            .resource_constraints
            .iter()
            .map(|c| c.as_str().to_owned()),
    )?;
    let data = encode_items(request.data.iter().map(|d| data_name(*d).to_owned()))?;
    for list in [repos, destinations, constraints, data] {
        let mut field = Vec::new();
        put_list(&mut field, list)?;
        hasher.update(&field);
    }

    let hex: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    // 64 lowercase hex characters always satisfy `Digest`; the error arm is unreachable.
    Digest::parse(&hex).map_err(|_| ArgError::Character)
}

/// The F7 string of a requested datum: the serde `snake_case` name of the category, or
/// `raw_secret_values`. The match is exhaustive, so a new category fails to compile here, and a test
/// compares every name with the serde name.
fn data_name(data: RequestedData) -> &'static str {
    match data {
        RequestedData::Category(category) => category_name(category),
        RequestedData::RawSecretValues => "raw_secret_values",
    }
}

/// The serde `snake_case` name of a data category.
fn category_name(category: DataCategory) -> &'static str {
    match category {
        DataCategory::RepositoryMetadata => "repository_metadata",
        DataCategory::RedactedSource => "redacted_source",
        DataCategory::RedactedFindings => "redacted_findings",
        DataCategory::SchemaDocuments => "schema_documents",
        DataCategory::PublicArtifactIdentifiers => "public_artifact_identifiers",
        DataCategory::RedactedChatContext => "redacted_chat_context",
        DataCategory::AppMetadata => "app_metadata",
    }
}

/// Appends `u32` big-endian length and the bytes of `s`.
fn put_str(out: &mut Vec<u8>, s: &str) -> Result<(), ArgError> {
    put_u32(out, s.len())?;
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

/// Appends a length or count as `u32` big endian. A value that does not fit is reported as
/// [`ArgError::TooLong`] (not reachable with the input bounds in force, but never truncated).
fn put_u32(out: &mut Vec<u8>, n: usize) -> Result<(), ArgError> {
    let n = u32::try_from(n).map_err(|_| ArgError::TooLong)?;
    out.extend_from_slice(&n.to_be_bytes());
    Ok(())
}

/// Encodes each text as `str(text)`.
fn encode_items(items: impl Iterator<Item = String>) -> Result<Vec<Vec<u8>>, ArgError> {
    items
        .map(|text| {
            let mut item = Vec::new();
            put_str(&mut item, &text)?;
            Ok(item)
        })
        .collect()
}

/// Appends `list(items)`: the deduplicated count, then the items sorted by their encoded bytes.
fn put_list(out: &mut Vec<u8>, mut items: Vec<Vec<u8>>) -> Result<(), ArgError> {
    items.sort_unstable();
    items.dedup();
    put_u32(out, items.len())?;
    for item in &items {
        out.extend_from_slice(item);
    }
    Ok(())
}
