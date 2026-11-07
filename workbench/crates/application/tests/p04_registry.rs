use application::dev_store;
use application::emit::{Clock, Emitter, IdSource, IdSourceError};
use application::repository_registry::{self as registry, RegistryError, StatusContext};
use domain::contract::{RepoOperation, RepoOperationStatus};
use domain::store::{AnyStore, StoreKind};
use domain::time::UtcTimestamp;
use persistence::fs::RealFs;
use persistence::paths::DataDir;
use persistence::read::{SchemaValidator, decode};
use persistence::writer::StoreWriter;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
struct FixedClock(UtcTimestamp);
impl Clock for FixedClock {
    fn now(&self) -> UtcTimestamp {
        self.0
    }
}
#[derive(Debug)]
struct Ids {
    next: u8,
    fail: bool,
}
impl IdSource for Ids {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        if self.fail {
            return Err(IdSourceError::ShortRead);
        }
        self.next += 1;
        let mut bytes = [0; 16];
        bytes[0] = self.next;
        Ok(bytes)
    }
}

#[test]
fn registry_state_events_duplicates_and_failure_order() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("p04-registry-{}", std::process::id()));
    std::fs::create_dir(&dir)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    assert_eq!(dev_store::init(&dir).failure, None);
    let fs = Arc::new(RealFs);
    let data = DataDir::new(&*fs, &dir)?;
    let (mut writer, _) = StoreWriter::open(data, fs, Arc::new(SchemaValidator::new()))?;
    let fixture = include_bytes!("../../persistence/tests/fixtures/store/repositories.max.json");
    let AnyStore::Repositories(mut source) =
        decode(StoreKind::Repositories, fixture, &SchemaValidator::new())?
    else {
        return Err("fixture is not repositories".into());
    };
    let repo = source.repositories.remove(0);
    let clock = FixedClock(UtcTimestamp::from_unix(1_800_000_000, 0)?);
    let mut emitter = Emitter::new(
        clock,
        Ids {
            next: 0,
            fail: false,
        },
    );
    registry::insert(&mut writer, &mut emitter, &clock, repo.clone())?;
    assert_eq!(registry::read(&mut writer, repo.repo_id)?, repo);
    assert_eq!(
        registry::insert(&mut writer, &mut emitter, &clock, repo.clone()),
        Err(RegistryError::Duplicate)
    );
    let context = StatusContext {
        operation: RepoOperation::Update,
        status: RepoOperationStatus::Succeeded,
        reason: None,
    };
    registry::replace(&mut writer, &mut emitter, &clock, repo.clone(), context)?;
    registry::remove(&mut writer, &mut emitter, &clock, repo.repo_id)?;
    assert_eq!(
        registry::remove(&mut writer, &mut emitter, &clock, repo.repo_id),
        Err(RegistryError::Missing)
    );
    let AnyStore::Log(log) = writer.read(StoreKind::Log)? else {
        return Err("not a log".into());
    };
    assert_eq!(
        log.events
            .iter()
            .map(|e| e.event_type.as_str())
            .collect::<Vec<_>>(),
        [
            "RepositoryRegistered",
            "RepositoryStatusChanged",
            "RepositoryRemoved"
        ]
    );
    let mut failing = Emitter::new(
        clock,
        Ids {
            next: 0,
            fail: true,
        },
    );
    assert_eq!(
        registry::insert(&mut writer, &mut failing, &clock, repo.clone()),
        Err(RegistryError::Emit)
    );
    assert_eq!(registry::read(&mut writer, repo.repo_id)?, repo);
    let AnyStore::Log(log) = writer.read(StoreKind::Log)? else {
        return Err("not a log".into());
    };
    assert_eq!(log.events.len(), 3);
    std::fs::rename(dir.join("log.json"), dir.join("log.saved"))?;
    assert_eq!(
        registry::remove(&mut writer, &mut emitter, &clock, repo.repo_id),
        Err(RegistryError::PublishedLogUnavailable)
    );
    assert_eq!(
        registry::read(&mut writer, repo.repo_id),
        Err(RegistryError::Missing)
    );
    drop(writer);
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn sync_failure_reports_which_publication_is_uncertain() -> Result<(), Box<dyn std::error::Error>> {
    use persistence::error::WriteStep;
    use persistence::fault::{FaultFs, FaultPlan};
    use std::os::unix::fs::PermissionsExt;

    for nth in [1, 2] {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p04-registry-fault-{}-{nth}", std::process::id()));
        std::fs::create_dir(&dir)?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        assert_eq!(dev_store::init(&dir).failure, None);
        let fs = Arc::new(FaultFs::new(FaultPlan {
            step: WriteStep::SyncDir,
            nth,
            error: std::io::ErrorKind::Other,
        }));
        let data = DataDir::new(&*fs, &dir)?;
        let (mut writer, _) =
            StoreWriter::open(data, fs.clone(), Arc::new(SchemaValidator::new()))?;
        let fixture =
            include_bytes!("../../persistence/tests/fixtures/store/repositories.max.json");
        let AnyStore::Repositories(mut source) =
            decode(StoreKind::Repositories, fixture, &SchemaValidator::new())?
        else {
            return Err("fixture is not repositories".into());
        };
        let repo = source.repositories.remove(0);
        let clock = FixedClock(UtcTimestamp::from_unix(1_800_000_000, 0)?);
        let mut emitter = Emitter::new(
            clock,
            Ids {
                next: 0,
                fail: false,
            },
        );
        let expected = if nth == 1 {
            RegistryError::RegistryWriteUncertain
        } else {
            RegistryError::PublishedLogWriteUncertain
        };
        assert_eq!(
            registry::insert(&mut writer, &mut emitter, &clock, repo.clone()),
            Err(expected)
        );
        assert!(fs.fired());
        assert!(writer.is_halted());
        assert_eq!(registry::read(&mut writer, repo.repo_id)?, repo);
        let AnyStore::Log(log) = writer.read(StoreKind::Log)? else {
            return Err("not a log".into());
        };
        assert_eq!(log.events.len(), (nth - 1) as usize);
        drop(writer);
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}
