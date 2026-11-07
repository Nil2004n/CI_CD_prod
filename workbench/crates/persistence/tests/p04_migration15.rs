use persistence::migration15::*;

#[test]
fn all_root_preflight_refuses_one_bad_document_and_preserves_absence()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = include_bytes!("fixtures/store/repositories.min.json");
    let approvals = include_bytes!("fixtures/store/approvals.min.json");
    let roots: [Option<&[u8]>; 8] = [
        None,
        Some(repository),
        None,
        Some(approvals),
        None,
        None,
        None,
        None,
    ];
    let result = prepare_roots(roots)?;
    assert!(result.candidate(0).is_none());
    assert!(result.candidate(8).is_none());
    for index in [1, 3] {
        let value: serde_json::Value =
            serde_json::from_slice(result.candidate(index).ok_or("candidate")?)?;
        assert_eq!(value["schema_version"], "1.5");
    }
    let bad: [Option<&[u8]>; 8] = [
        None,
        Some(repository),
        None,
        Some(b"not json"),
        None,
        None,
        None,
        None,
    ];
    assert!(prepare_roots(bad).is_err());
    assert_eq!(format!("{result:?}"), "PreparedRoots(<withheld>)");
    Ok(())
}

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn hash(c: char) -> Result<Hash, Box<dyn std::error::Error>> {
    Hash::parse(&c.to_string().repeat(64)).ok_or_else(|| "invalid fixture digest".into())
}
fn manifest() -> Result<Manifest, Box<dyn std::error::Error>> {
    let original = hash('a')?;
    let candidate = hash('b')?;
    Ok(Manifest {
        version: 1,
        entries: ROOTS
            .iter()
            .enumerate()
            .map(|(i, &root)| Entry {
                root,
                original: (i != 2).then(|| original.clone()),
                candidate: (i != 2).then(|| candidate.clone()),
            })
            .collect(),
    })
}
fn fixture() -> Result<JournalSnapshot, Box<dyn std::error::Error>> {
    let manifest = manifest()?;
    let digest = manifest
        .digest()
        .map_err(|e| format!("invalid fixture manifest: {e:?}"))?;
    let roots = manifest
        .entries
        .iter()
        .map(|e| RootObservation {
            root: e.root,
            live: e
                .original
                .clone()
                .map_or(Observation::Absent, Observation::Hash),
            backup: e
                .original
                .clone()
                .map_or(Observation::Absent, Observation::Hash),
            candidate: e
                .candidate
                .clone()
                .map_or(Observation::Absent, Observation::Hash),
        })
        .collect();
    Ok(JournalSnapshot {
        begin: Marker::Valid(()),
        manifest: Marker::Valid(manifest),
        prepare: Marker::Valid(digest),
        commit: Marker::Absent,
        done: Marker::Absent,
        roots,
        control_inventory_complete: true,
    })
}
fn digest(s: &JournalSnapshot) -> Result<Hash, Box<dyn std::error::Error>> {
    match &s.prepare {
        Marker::Valid(h) => Ok(h.clone()),
        _ => Err("missing fixture marker".into()),
    }
}

#[test]
fn state_sequence_and_mixed_committed_roots() -> TestResult {
    let mut s = fixture()?;
    assert_eq!(decide(&s), Ok(Decision::PreparedNeedsAbort));
    s.roots[0].live = Observation::Hash(hash('b')?);
    assert_eq!(decide(&s), Err(JournalError::LiveMismatch));
    s.commit = Marker::Valid(digest(&s)?);
    assert_eq!(decide(&s), Ok(Decision::RollForward));
    s.done = Marker::Valid(digest(&s)?);
    s.roots[0].live = Observation::Hash(hash('c')?);
    assert_eq!(decide(&s), Ok(Decision::DoneNeedsValidation));
    s.roots[0].live = Observation::Invalid;
    assert_eq!(decide(&s), Err(JournalError::LiveMismatch));
    Ok(())
}

#[test]
fn malformed_or_inconsistent_markers_fail_closed() -> TestResult {
    let mut s = fixture()?;
    s.begin = Marker::Absent;
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    let mut s = fixture()?;
    s.done = Marker::Valid(digest(&s)?);
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    let mut s = fixture()?;
    s.commit = Marker::Valid(hash('c')?);
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    let mut s = fixture()?;
    s.prepare = Marker::Invalid;
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    let mut s = fixture()?;
    s.control_inventory_complete = false;
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    Ok(())
}

#[test]
fn artifacts_live_inventory_and_manifest_tampering_fail() -> TestResult {
    let mut s = fixture()?;
    s.roots[0].backup = Observation::Hash(hash('c')?);
    assert_eq!(decide(&s), Err(JournalError::ArtifactMismatch));
    let mut s = fixture()?;
    s.roots[2].candidate = Observation::Hash(hash('b')?);
    assert_eq!(decide(&s), Err(JournalError::ArtifactMismatch));
    let mut s = fixture()?;
    s.roots[2].live = Observation::Hash(hash('b')?);
    assert_eq!(decide(&s), Err(JournalError::LiveMismatch));
    let mut s = fixture()?;
    s.roots[0].root = Root::Log;
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    let mut m = manifest()?;
    m.entries.swap(0, 1);
    assert_eq!(m.digest(), Err(JournalError::InvalidManifest));
    let mut m = manifest()?;
    m.entries[0].candidate = None;
    assert_eq!(m.digest(), Err(JournalError::InvalidManifest));
    let mut m = manifest()?;
    m.version = 2;
    assert_eq!(m.digest(), Err(JournalError::InvalidManifest));
    Ok(())
}

#[test]
fn no_journal_requires_clean_inventory_and_external_validation() -> TestResult {
    let mut s = fixture()?;
    s.begin = Marker::Absent;
    s.manifest = Marker::Absent;
    s.prepare = Marker::Absent;
    for root in &mut s.roots {
        root.backup = Observation::Absent;
        root.candidate = Observation::Absent;
    }
    assert_eq!(decide(&s), Ok(Decision::LegacyNeedsValidation));
    s.roots[0].backup = Observation::Hash(hash('a')?);
    assert_eq!(decide(&s), Err(JournalError::Ambiguous));
    Ok(())
}

#[test]
fn hashes_and_json_are_strict() -> TestResult {
    assert!(Hash::parse(&"A".repeat(64)).is_none());
    assert!(Hash::parse(&"g".repeat(64)).is_none());
    assert!(Hash::parse(&"0".repeat(63)).is_none());
    assert!(
        serde_json::from_str::<Manifest>(r#"{"version":1,"entries":[],"extra":true}"#).is_err()
    );
    assert!(serde_json::from_str::<Hash>(r#""ABCD""#).is_err());
    Ok(())
}
