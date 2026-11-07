//! Pure journal inspection for a future 1.4 to 1.5 migration.
//!
//! This module performs no I/O and does not authorize migration or writes. The caller must hold
//! the store lock, bound and validate all input, inspect the complete control directory and read
//! artifacts without following links. In particular, `Absent` is trustworthy only after an
//! exhaustive directory inventory. File hashes are observations, not proof of durable fsync.

use domain::store::StoreKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Candidate documents in the fixed operational-root order. Contents are never
/// shown by `Debug`; constructing this value performs no filesystem writes.
pub struct PreparedRoots {
    candidates: [Option<Vec<u8>>; 8],
}

impl std::fmt::Debug for PreparedRoots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedRoots(<withheld>)")
    }
}

impl PreparedRoots {
    /// Return the candidate at the fixed `ROOTS` index, or `None` for an absent
    /// root or an out-of-range index.
    pub fn candidate(&self, index: usize) -> Option<&[u8]> {
        self.candidates.get(index).and_then(Option::as_deref)
    }
}

/// Prepare every present operational root before beginning a migration.
///
/// Inputs correspond exactly to [`ROOTS`]; absent roots remain absent. This
/// checks each legacy document and its candidate but does not establish a
/// filesystem snapshot, cross-root referential integrity, or recovery safety.
pub fn prepare_roots(
    originals: [Option<&[u8]>; 8],
) -> Result<PreparedRoots, crate::error::StoreError> {
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
    let validator = crate::read::SchemaValidator::new();
    let mut candidates = std::array::from_fn(|_| None);
    for (index, (kind, original)) in KINDS.into_iter().zip(originals).enumerate() {
        if let Some(bytes) = original {
            candidates[index] = Some(crate::read::prepare_operational15(kind, bytes, &validator)?);
        }
    }
    Ok(PreparedRoots { candidates })
}

/// The complete writable operational root inventory; values never contain paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Root {
    Config,
    Repositories,
    Workflows,
    Approvals,
    Log,
    RagIndex,
    Conversations,
    ToolManifest,
}

pub const ROOTS: [Root; 8] = [
    Root::Config,
    Root::Repositories,
    Root::Workflows,
    Root::Approvals,
    Root::Log,
    Root::RagIndex,
    Root::Conversations,
    Root::ToolManifest,
];

/// Exact lowercase SHA-256 encoding. Constructed only after validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Hash(String);

impl Hash {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| serde::de::Error::custom("invalid digest"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub root: Root,
    /// Both absent for an absent optional root; otherwise both must be present.
    pub original: Option<Hash>,
    pub candidate: Option<Hash>,
}

/// Entries must appear once in `ROOTS` order. The fixed order makes hashing deterministic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u8,
    pub entries: Vec<Entry>,
}

impl Manifest {
    pub fn digest(&self) -> Result<Hash, JournalError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| JournalError::InvalidManifest)?;
        let hash = Sha256::digest(bytes);
        let mut text = String::with_capacity(64);
        use std::fmt::Write as _;
        for byte in hash {
            write!(&mut text, "{byte:02x}").map_err(|_| JournalError::InvalidManifest)?;
        }
        Hash::parse(&text).ok_or(JournalError::InvalidManifest)
    }

    fn validate(&self) -> Result<(), JournalError> {
        if self.version != 1 || self.entries.len() != ROOTS.len() {
            return Err(JournalError::InvalidManifest);
        }
        for (entry, expected) in self.entries.iter().zip(ROOTS) {
            if entry.root != expected || entry.original.is_some() != entry.candidate.is_some() {
                return Err(JournalError::InvalidManifest);
            }
        }
        Ok(())
    }
}

/// Marker content has already been parsed strictly by the caller. `Invalid` includes unreadable,
/// malformed, duplicate and otherwise ambiguous markers, and must never be treated as absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Marker<T> {
    Absent,
    Valid(T),
    Invalid,
}

/// Observed hash of a regular file, absence, or any unsafe/unreadable/ambiguous object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    Absent,
    Hash(Hash),
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootObservation {
    pub root: Root,
    pub live: Observation,
    pub backup: Observation,
    pub candidate: Observation,
}

/// All markers and the exact inventory must be obtained while the writer lock is held.
#[derive(Clone, Debug)]
pub struct JournalSnapshot {
    pub begin: Marker<()>,
    pub manifest: Marker<Manifest>,
    pub prepare: Marker<Hash>,
    pub commit: Marker<Hash>,
    pub done: Marker<Hash>,
    pub roots: Vec<RootObservation>,
    /// False if the control directory contains unexpected or unclassified entries.
    pub control_inventory_complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// No journal was found. The caller must separately validate legacy roots and version policy.
    LegacyNeedsValidation,
    /// Durable prepare without commit; no roots may have changed. Requires a separately designed abort.
    PreparedNeedsAbort,
    /// Commit exists; a separately implemented and validated roll-forward is required.
    RollForward,
    /// A complete receipt exists. Normal startup still requires version and document validation.
    DoneNeedsValidation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalError {
    Ambiguous,
    InvalidManifest,
    ArtifactMismatch,
    LiveMismatch,
}

fn matches_observation(observed: &Observation, expected: &Option<Hash>) -> bool {
    match (observed, expected) {
        (Observation::Absent, None) => true,
        (Observation::Hash(actual), Some(hash)) => actual == hash,
        _ => false,
    }
}

/// Classifies the snapshot; this never modifies it or completes a transaction.
pub fn decide(snapshot: &JournalSnapshot) -> Result<Decision, JournalError> {
    use Marker::{Absent, Invalid, Valid};
    if !snapshot.control_inventory_complete
        || snapshot.roots.len() != ROOTS.len()
        || snapshot
            .roots
            .iter()
            .zip(ROOTS)
            .any(|(o, root)| o.root != root)
    {
        return Err(JournalError::Ambiguous);
    }
    if matches!(snapshot.begin, Invalid)
        || matches!(snapshot.manifest, Invalid)
        || matches!(snapshot.prepare, Invalid)
        || matches!(snapshot.commit, Invalid)
        || matches!(snapshot.done, Invalid)
    {
        return Err(JournalError::Ambiguous);
    }
    if matches!(snapshot.begin, Absent)
        && matches!(snapshot.manifest, Absent)
        && matches!(snapshot.prepare, Absent)
        && matches!(snapshot.commit, Absent)
        && matches!(snapshot.done, Absent)
    {
        if snapshot.roots.iter().any(|r| {
            r.backup != Observation::Absent
                || r.candidate != Observation::Absent
                || r.live == Observation::Invalid
        }) {
            return Err(JournalError::Ambiguous);
        }
        return Ok(Decision::LegacyNeedsValidation);
    }
    let (Valid(()), Valid(manifest), Valid(prepare)) =
        (&snapshot.begin, &snapshot.manifest, &snapshot.prepare)
    else {
        return Err(JournalError::Ambiguous);
    };
    let digest = manifest.digest()?;
    if prepare != &digest {
        return Err(JournalError::Ambiguous);
    }
    let committed = match &snapshot.commit {
        Absent => false,
        Valid(h) if h == &digest => true,
        _ => return Err(JournalError::Ambiguous),
    };
    let done = match &snapshot.done {
        Absent => false,
        Valid(h) if committed && h == &digest => true,
        _ => return Err(JournalError::Ambiguous),
    };
    for (entry, observed) in manifest.entries.iter().zip(&snapshot.roots) {
        if !matches_observation(&observed.backup, &entry.original)
            || !matches_observation(&observed.candidate, &entry.candidate)
        {
            return Err(JournalError::ArtifactMismatch);
        }
        // Once DONE is durable, later normal writes may legitimately alter live roots.
        if !done
            && !matches_observation(&observed.live, &entry.original)
            && !(committed && matches_observation(&observed.live, &entry.candidate))
        {
            return Err(JournalError::LiveMismatch);
        }
        if done && observed.live == Observation::Invalid {
            return Err(JournalError::LiveMismatch);
        }
    }
    Ok(if done {
        Decision::DoneNeedsValidation
    } else if committed {
        Decision::RollForward
    } else {
        Decision::PreparedNeedsAbort
    })
}
