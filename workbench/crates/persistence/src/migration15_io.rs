//! Lock-held 1.4→1.5 journal operations. The caller must hold the matching StoreWriter lock.
//! No automatic migration is initiated by this module.
use crate::fs::FsOps;
use crate::{
    atomic,
    error::{StoreError, WriteStep},
    migration15::{
        self, Entry, Hash, JournalSnapshot, Manifest, Marker, Observation, ROOTS, RootObservation,
    },
    paths::{DataDir, StorePath, check_private_dir},
    read,
};
use domain::store::{StoreKind, VersionedStore};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
};

const CONTROL: &str = ".migration-15";
const LIMIT: u64 = read::MAX_DOCUMENT_BYTES;
const MANIFEST_LIMIT: u64 = 8192;
type RootPair = (Option<Vec<u8>>, Option<Vec<u8>>);
type Preflight<'a> = dyn Fn(&[RootPair]) -> Result<(), StoreError> + 'a;
const KINDS: [StoreKind; 8] = [
    StoreKind::Config,
    StoreKind::Repositories,
    StoreKind::Workflows,
    StoreKind::Approvals,
    StoreKind::Log,
    StoreKind::RagIndex,
    StoreKind::Conversations,
    StoreKind::ToolManifest,
];

/// Verify available retained reports against the old log while the store lock is held.
/// Missing historical records remain unresolved, not invented or deleted.
pub(crate) fn verify_crossroot(
    fs: &dyn FsOps,
    dir: &DataDir,
    roots: &[RootPair],
) -> Result<(), StoreError> {
    use domain::store::AnyStore;
    if roots.len() != KINDS.len() {
        return Err(bad());
    }
    let validator = read::SchemaValidator::new();
    let log = match &roots[4].0 {
        None => None,
        Some(bytes) => match read::decode(StoreKind::Log, bytes, &validator)? {
            AnyStore::Log(file) => Some(file),
            _ => return Err(bad()),
        },
    };
    let backup = dir.as_path().join("backup");
    let entries = match fs.list_dir(&backup) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(io_error(WriteStep::Read, e)),
    };
    if entries.len() > 10_000 {
        return Err(bad());
    }
    let mut reports = Vec::new();
    for entry in entries {
        let Some(name) = entry.to_str() else {
            continue;
        };
        if !crate::report_name::split_report_file_name(name).is_some_and(|(_, ext)| ext == "json") {
            continue;
        }
        let path = StorePath::report(name)?;
        let bytes = read_optional(fs, &dir.join(&path), LIMIT)?.ok_or_else(bad)?;
        let report = match read::decode(StoreKind::Report, &bytes, &validator)? {
            AnyStore::Report(report) => *report,
            _ => return Err(bad()),
        };
        if let Some(log) = &log {
            for manifest in &log.reports {
                if manifest.json_path.as_str() == path.as_str()
                    && (manifest.report_id != report.report_id
                        || manifest.run_id != report.run_id
                        || manifest.repo_id != report.repo_id)
                {
                    return Err(bad());
                }
            }
        }
        reports.push(report);
    }
    crate::crossroot15::validate(log.as_ref(), &reports).map_err(|_| bad())?;
    Ok(())
}

fn bad() -> StoreError {
    StoreError::PathRejected
}
fn io_error(step: WriteStep, e: io::Error) -> StoreError {
    StoreError::Io {
        step,
        kind: e.kind(),
    }
}
fn digest(bytes: &[u8]) -> Result<Hash, StoreError> {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        write!(&mut hex, "{b:02x}").map_err(|_| bad())?;
    }
    Hash::parse(&hex).ok_or_else(bad)
}
fn root_path(dir: &DataDir, kind: StoreKind) -> Result<PathBuf, StoreError> {
    Ok(dir.join(&StorePath::root(kind).ok_or_else(bad)?))
}
fn read_optional(fs: &dyn FsOps, path: &Path, limit: u64) -> Result<Option<Vec<u8>>, StoreError> {
    match fs.symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(WriteStep::Read, e)),
        Ok(info) if info.is_regular() => fs
            .read(path, limit)
            .map(Some)
            .map_err(|e| io_error(WriteStep::Read, e)),
        Ok(_) => Err(bad()),
    }
}
fn persist_new(fs: &dyn FsOps, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    fs.create_new(WriteStep::CreateTemp, path, 0o600)
        .map_err(|e| io_error(WriteStep::CreateTemp, e))?;
    fs.write_all(WriteStep::WriteTemp, path, bytes)
        .map_err(|e| io_error(WriteStep::WriteTemp, e))?;
    fs.sync_file(WriteStep::SyncTemp, path)
        .map_err(|e| io_error(WriteStep::SyncTemp, e))
}
fn sync(fs: &dyn FsOps, path: &Path) -> Result<(), StoreError> {
    fs.sync_dir(WriteStep::SyncDir, path)
        .map_err(|e| io_error(WriteStep::SyncDir, e))
}
fn marker(fs: &dyn FsOps, dir: &Path, name: &str, bytes: &[u8]) -> Result<(), StoreError> {
    persist_new(fs, &dir.join(name), bytes)?;
    sync(fs, dir)
}
fn parse_marker(fs: &dyn FsOps, dir: &Path, name: &str) -> Result<Marker<Hash>, StoreError> {
    Ok(match read_optional(fs, &dir.join(name), 64)? {
        None => Marker::Absent,
        Some(bytes) => match std::str::from_utf8(&bytes).ok().and_then(Hash::parse) {
            Some(h) => Marker::Valid(h),
            None => Marker::Invalid,
        },
    })
}
fn hash_bytes(h: &Hash) -> Result<Vec<u8>, StoreError> {
    // Hash is serialized as one JSON string; deserialize to get its exact ASCII payload.
    let v = serde_json::to_value(h).map_err(|_| bad())?;
    Ok(v.as_str().ok_or_else(bad)?.as_bytes().to_vec())
}
fn artifact_name(prefix: &str, index: usize) -> String {
    format!("{prefix}-{index}")
}
fn observed(fs: &dyn FsOps, path: &Path) -> Result<Observation, StoreError> {
    Ok(match read_optional(fs, path, LIMIT)? {
        None => Observation::Absent,
        Some(b) => Observation::Hash(digest(&b)?),
    })
}
fn allowed_names() -> HashSet<String> {
    let mut names: HashSet<String> = ["begin", "manifest", "prepare", "commit", "done"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for i in 0..ROOTS.len() {
        names.insert(artifact_name("old", i));
        names.insert(artifact_name("new", i));
    }
    names
}
fn inspect(
    fs: &dyn FsOps,
    dir: &DataDir,
    owner: u32,
) -> Result<Option<JournalSnapshot>, StoreError> {
    let control = dir.as_path().join(CONTROL);
    match fs.symlink_metadata(&control) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_error(WriteStep::Read, e)),
        Ok(_) => check_private_dir(fs, &control, owner)?,
    }
    let allowed = allowed_names();
    let names = fs
        .list_dir(&control)
        .map_err(|e| io_error(WriteStep::Read, e))?;
    if names
        .iter()
        .any(|n| n.to_str().is_none_or(|s| !allowed.contains(s)))
    {
        return Err(bad());
    }
    let begin = match read_optional(fs, &control.join("begin"), 16)? {
        None => Marker::Absent,
        Some(b) if b == b"pwt15-begin-v1" => Marker::Valid(()),
        Some(_) => Marker::Invalid,
    };
    let manifest = match read_optional(fs, &control.join("manifest"), MANIFEST_LIMIT)? {
        None => Marker::Absent,
        Some(b) => match read::migration_manifest(&b) {
            Some(m) => Marker::Valid(m),
            None => Marker::Invalid,
        },
    };
    let mut roots = Vec::with_capacity(ROOTS.len());
    for (i, (&root, &kind)) in ROOTS.iter().zip(&KINDS).enumerate() {
        roots.push(RootObservation {
            root,
            live: observed(fs, &root_path(dir, kind)?)?,
            backup: observed(fs, &control.join(artifact_name("old", i)))?,
            candidate: observed(fs, &control.join(artifact_name("new", i)))?,
        });
    }
    Ok(Some(JournalSnapshot {
        begin,
        manifest,
        prepare: parse_marker(fs, &control, "prepare")?,
        commit: parse_marker(fs, &control, "commit")?,
        done: parse_marker(fs, &control, "done")?,
        roots,
        control_inventory_complete: true,
    }))
}
fn verify_version(fs: &dyn FsOps, dir: &DataDir, v15: bool) -> Result<(), StoreError> {
    let validator = read::SchemaValidator::new();
    for kind in KINDS {
        if let Some(bytes) = read_optional(fs, &root_path(dir, kind)?, LIMIT)? {
            let doc = read::decode_versioned(kind, &bytes, &validator)?;
            if !matches!(
                (v15, doc),
                (true, VersionedStore::V15(_)) | (false, VersionedStore::V14(_))
            ) {
                return Err(bad());
            }
        }
    }
    Ok(())
}

// Preserve legacy startup's malformed-root classification while refusing an
// unjournaled 1.5 root before reconciliation can mutate any file.
fn reject_unjournaled_15(fs: &dyn FsOps, dir: &DataDir) -> Result<(), StoreError> {
    for kind in KINDS {
        let bytes = match read_optional(fs, &root_path(dir, kind)?, LIMIT) {
            Ok(Some(bytes)) => bytes,
            Ok(None) | Err(StoreError::PathRejected) => continue,
            Err(StoreError::Io {
                kind: io::ErrorKind::FileTooLarge,
                ..
            }) => continue,
            Err(error) => return Err(error),
        };
        if read::claims_version15(&bytes) {
            return Err(bad());
        }
    }
    Ok(())
}
// A hash only binds journal bytes to the manifest. Revalidate both versions and the
// deterministic conversion before publishing any of the candidate roots.
fn verify_artifacts(
    fs: &dyn FsOps,
    control: &Path,
    manifest: &Manifest,
) -> Result<Vec<RootPair>, StoreError> {
    let validator = read::SchemaValidator::new();
    let mut roots = Vec::with_capacity(ROOTS.len());
    for (i, (entry, kind)) in manifest.entries.iter().zip(KINDS).enumerate() {
        match (&entry.original, &entry.candidate) {
            (None, None) => roots.push((None, None)),
            (Some(original_hash), Some(candidate_hash)) => {
                let original = read_optional(fs, &control.join(artifact_name("old", i)), LIMIT)?
                    .ok_or_else(bad)?;
                let candidate = read_optional(fs, &control.join(artifact_name("new", i)), LIMIT)?
                    .ok_or_else(bad)?;
                if &digest(&original)? != original_hash || &digest(&candidate)? != candidate_hash {
                    return Err(bad());
                }
                if read::prepare_operational15(kind, &original, &validator)? != candidate {
                    return Err(bad());
                }
                roots.push((Some(original), Some(candidate)));
            }
            _ => return Err(bad()),
        }
    }
    Ok(roots)
}
/// Inspect/recover before any ordinary write. Requires the matching lock already held.
pub(crate) fn recover(
    fs: &dyn FsOps,
    dir: &DataDir,
    owner: u32,
    preflight: &Preflight<'_>,
) -> Result<bool, StoreError> {
    let Some(snapshot) = inspect(fs, dir, owner)? else {
        reject_unjournaled_15(fs, dir)?;
        return Ok(false);
    };
    let decision = migration15::decide(&snapshot).map_err(|_| bad())?;
    match decision {
        migration15::Decision::RollForward => {
            let Marker::Valid(manifest) = &snapshot.manifest else {
                return Err(bad());
            };
            let control = dir.as_path().join(CONTROL);
            let roots = verify_artifacts(fs, &control, manifest)?;
            preflight(&roots)?;
            fs.sync_file(WriteStep::SyncTemp, &control.join("commit"))
                .map_err(|e| io_error(WriteStep::SyncTemp, e))?;
            sync(fs, &control)?;
            for (i, (entry, kind)) in manifest.entries.iter().zip(KINDS).enumerate() {
                if entry.candidate.is_some() {
                    let bytes = read_optional(fs, &control.join(artifact_name("new", i)), LIMIT)?
                        .ok_or_else(bad)?;
                    if Some(digest(&bytes)?) != entry.candidate {
                        return Err(bad());
                    }
                    let path = StorePath::root(kind).ok_or_else(bad)?;
                    atomic::write_replace(fs, dir, &path, &bytes, false)?;
                }
            }
            verify_version(fs, dir, true)?;
            // Reinspect prior to DONE, including backups and live candidate hashes.
            let checked = inspect(fs, dir, owner)?.ok_or_else(bad)?;
            if migration15::decide(&checked).map_err(|_| bad())?
                != migration15::Decision::RollForward
            {
                return Err(bad());
            }
            for (entry, obs) in manifest.entries.iter().zip(&checked.roots) {
                let expected = entry
                    .candidate
                    .clone()
                    .map_or(Observation::Absent, Observation::Hash);
                if obs.live != expected {
                    return Err(bad());
                }
            }
            marker(
                fs,
                &control,
                "done",
                &hash_bytes(&manifest.digest().map_err(|_| bad())?)?,
            )?;
            Ok(true)
        }
        migration15::Decision::DoneNeedsValidation => {
            let control = dir.as_path().join(CONTROL);
            fs.sync_file(WriteStep::SyncTemp, &control.join("done"))
                .map_err(|e| io_error(WriteStep::SyncTemp, e))?;
            sync(fs, &control)?;
            verify_version(fs, dir, true)?;
            Ok(true)
        }
        _ => Err(bad()),
    }
}
/// Explicit upgrade only. Requires the matching lock already held and successful startup recovery.
pub(crate) fn upgrade(
    fs: &dyn FsOps,
    dir: &DataDir,
    owner: u32,
    preflight: &Preflight<'_>,
) -> Result<(), StoreError> {
    if inspect(fs, dir, owner)?.is_some() {
        return Err(bad());
    }
    let validator = read::SchemaValidator::new();
    let mut entries = Vec::with_capacity(ROOTS.len());
    let mut data = Vec::with_capacity(ROOTS.len());
    for (&root, &kind) in ROOTS.iter().zip(&KINDS) {
        let old = read_optional(fs, &root_path(dir, kind)?, LIMIT)?;
        let new = old
            .as_ref()
            .map(|bytes| read::prepare_operational15(kind, bytes, &validator))
            .transpose()?;
        entries.push(Entry {
            root,
            original: old.as_ref().map(|b| digest(b)).transpose()?,
            candidate: new.as_ref().map(|b| digest(b)).transpose()?,
        });
        data.push((old, new));
    }
    preflight(&data)?;
    let manifest = Manifest {
        version: 1,
        entries,
    };
    let hash = manifest.digest().map_err(|_| bad())?;
    let encoded = serde_json::to_vec(&manifest).map_err(|_| bad())?;
    if u64::try_from(encoded.len()).map_or(true, |n| n > MANIFEST_LIMIT) {
        return Err(bad());
    }
    let control = dir.as_path().join(CONTROL);
    fs.create_dir(WriteStep::CreateDir, &control, 0o700)
        .map_err(|e| io_error(WriteStep::CreateDir, e))?;
    check_private_dir(fs, &control, owner)?;
    sync(fs, dir.as_path())?;
    marker(fs, &control, "begin", b"pwt15-begin-v1")?;
    for (i, (old, new)) in data.iter().enumerate() {
        if let (Some(old), Some(new)) = (old, new) {
            persist_new(fs, &control.join(artifact_name("old", i)), old)?;
            persist_new(fs, &control.join(artifact_name("new", i)), new)?;
        }
    }
    sync(fs, &control)?;
    persist_new(fs, &control.join("manifest"), &encoded)?;
    sync(fs, &control)?;
    marker(fs, &control, "prepare", &hash_bytes(&hash)?)?;
    let prepared = inspect(fs, dir, owner)?.ok_or_else(bad)?;
    if migration15::decide(&prepared).map_err(|_| bad())?
        != migration15::Decision::PreparedNeedsAbort
    {
        return Err(bad());
    }
    marker(fs, &control, "commit", &hash_bytes(&hash)?)?;
    if !recover(fs, dir, owner, preflight)? {
        return Err(bad());
    }
    Ok(())
}
