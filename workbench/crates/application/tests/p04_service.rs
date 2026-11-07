use application::dev_store;
use application::emit::{Clock, Emitter, IdSource, IdSourceError};
use application::repository_registry;
use application::repository_service::{
    CheckoutAccess, RemoveOptions, RepositoryError, RepositoryService,
};
use domain::ids::RepoId;
use domain::store::values::GitOid;
use domain::store::{AnyStore, StoreKind};
use domain::time::UtcTimestamp;
use persistence::fs::RealFs;
use persistence::paths::DataDir;
use persistence::read::{SchemaValidator, decode};
use persistence::writer::StoreWriter;
use repositories::git::ops::{GitOutcome, HardenedGit, TargetRef};
use repositories::git::runner::GitError;
use repositories::lock::LockError;
use repositories::remote::RemoteSpec;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
struct FixedClock(UtcTimestamp);
impl Clock for FixedClock {
    fn now(&self) -> UtcTimestamp {
        self.0
    }
}
#[derive(Debug)]
struct Ids {
    fail: bool,
    next: u8,
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
struct NoGit {
    calls: usize,
}
impl HardenedGit for NoGit {
    fn clone_verified(
        &mut self,
        _: &RemoteSpec,
        _: &TargetRef,
        _: &Path,
    ) -> Result<GitOutcome, GitError> {
        self.calls += 1;
        Err(GitError::InvalidInvocation)
    }
    fn update_verified(
        &mut self,
        _: &RemoteSpec,
        _: &TargetRef,
        _: &GitOid,
        _: &Path,
    ) -> Result<GitOutcome, GitError> {
        self.calls += 1;
        Err(GitError::InvalidInvocation)
    }
}
struct Checkout {
    locked: bool,
    path_calls: std::cell::Cell<usize>,
    delete_calls: usize,
}
impl CheckoutAccess for Checkout {
    type Guard = ();
    fn lock(&mut self, _: RepoId) -> Result<(), LockError> {
        if self.locked {
            Err(LockError::Locked)
        } else {
            Ok(())
        }
    }
    fn checkout_path(&self, _: RepoId, _: &()) -> Result<PathBuf, RepositoryError> {
        self.path_calls.set(self.path_calls.get() + 1);
        Err(RepositoryError::Unsafe)
    }
    fn delete(&mut self, _: RepoId, _: &()) -> Result<(), RepositoryError> {
        self.delete_calls += 1;
        Err(RepositoryError::Unsafe)
    }
}
fn options(delete_checkout: bool) -> RemoveOptions {
    RemoveOptions {
        delete_checkout,
        purge_derived_index: true,
        keep_reports: true,
    }
}
#[test]
fn reads_removes_and_reports_published_event_failure() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p04-service-{}", std::process::id()));
    std::fs::create_dir(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    assert_eq!(dev_store::init(&dir).failure, None);
    let fs = Arc::new(RealFs);
    let data = DataDir::new(&*fs, &dir)?;
    let (mut writer, _) = StoreWriter::open(data, fs, Arc::new(SchemaValidator::new()))?;
    let AnyStore::Repositories(mut source) = decode(
        StoreKind::Repositories,
        include_bytes!("../../persistence/tests/fixtures/store/repositories.max.json"),
        &SchemaValidator::new(),
    )?
    else {
        return Err("fixture is not repositories".into());
    };
    let repo = source.repositories.remove(0);
    let clock = FixedClock(UtcTimestamp::from_unix(1_800_000_000, 0)?);
    let mut emitter = Emitter::new(
        clock,
        Ids {
            fail: false,
            next: 0,
        },
    );
    let mut ids = Ids {
        fail: false,
        next: 0,
    };
    let mut git = NoGit { calls: 0 };
    let mut checkout = Checkout {
        locked: false,
        path_calls: std::cell::Cell::new(0),
        delete_calls: 0,
    };
    {
        let mut svc = RepositoryService::new(
            &mut writer,
            &mut emitter,
            &clock,
            &mut ids,
            &mut git,
            &mut checkout,
        );
        assert_eq!(svc.list(), Ok(vec![]));
        assert_eq!(svc.status(repo.repo_id), Err(RepositoryError::Missing));
    }
    repository_registry::insert(&mut writer, &mut emitter, &clock, repo.clone())?;
    {
        let mut svc = RepositoryService::new(
            &mut writer,
            &mut emitter,
            &clock,
            &mut ids,
            &mut git,
            &mut checkout,
        );
        assert_eq!(svc.list(), Ok(vec![repo.clone()]));
        assert_eq!(svc.status(repo.repo_id), Ok(repo.clone()));
        assert_eq!(
            svc.add(domain::contract::RawInput::new(
                repo.remote_url.as_str().to_owned()
            )),
            Err(RepositoryError::Duplicate)
        );
        assert_eq!(
            svc.add(domain::contract::RawInput::new(
                "file:///tmp/not-allowed".to_owned()
            )),
            Err(RepositoryError::Invalid)
        );
        assert_eq!(
            svc.remove(repo.repo_id, options(true)),
            Err(RepositoryError::Unsafe)
        );
    }
    checkout.locked = true;
    {
        let mut svc = RepositoryService::new(
            &mut writer,
            &mut emitter,
            &clock,
            &mut ids,
            &mut git,
            &mut checkout,
        );
        assert_eq!(
            svc.remove(repo.repo_id, options(false)),
            Err(RepositoryError::Locked)
        );
        assert_eq!(svc.status(repo.repo_id), Ok(repo.clone()));
    }
    checkout.locked = false;
    {
        let mut svc = RepositoryService::new(
            &mut writer,
            &mut emitter,
            &clock,
            &mut ids,
            &mut git,
            &mut checkout,
        );
        assert_eq!(svc.remove(repo.repo_id, options(false)), Ok(()));
        assert_eq!(svc.status(repo.repo_id), Err(RepositoryError::Missing));
    }
    repository_registry::insert(&mut writer, &mut emitter, &clock, repo.clone())?;
    let mut failing = Emitter::new(
        clock,
        Ids {
            fail: true,
            next: 0,
        },
    );
    {
        let mut svc = RepositoryService::new(
            &mut writer,
            &mut failing,
            &clock,
            &mut ids,
            &mut git,
            &mut checkout,
        );
        assert_eq!(
            svc.remove(repo.repo_id, options(false)),
            Err(RepositoryError::PublishedEventUnavailable)
        );
        assert_eq!(svc.status(repo.repo_id), Err(RepositoryError::Missing));
    }
    assert_eq!(git.calls, 0);
    assert_eq!(checkout.path_calls.get(), 0);
    assert_eq!(checkout.delete_calls, 0);
    drop(writer);
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
