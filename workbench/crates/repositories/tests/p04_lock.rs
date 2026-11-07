#![cfg(target_os = "linux")]

use domain::ids::{IdError, RepoId};
use repositories::lock::{LockError, acquire_checkout_lock};
use std::fs;
use std::os::unix::fs::{DirBuilderExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "p04-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join("checkouts"))?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join("checkouts/.locks"))?;
        Ok(Self(path))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let result = fs::remove_dir_all(&self.0);
        assert!(
            result.is_ok(),
            "remove fixture directory {:?}: {result:?}",
            self.0
        );
    }
}
fn id() -> Result<RepoId, IdError> {
    RepoId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b")
}

#[test]
fn contention_and_release() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    let guard = acquire_checkout_lock(&f.0, id()?).unwrap();
    assert_eq!(
        acquire_checkout_lock(&f.0, id()?).unwrap_err(),
        LockError::Locked
    );
    drop(guard);
    assert!(acquire_checkout_lock(&f.0, id()?).is_ok());
    Ok(())
}

#[test]
fn refuses_symlink_and_hardlinked_lock_files() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    let path = f.0.join(format!("checkouts/.locks/{}.lock", id()?));
    let other = f.0.join("other");
    fs::write(&other, b"").unwrap();
    symlink(&other, &path).unwrap();
    assert_eq!(
        acquire_checkout_lock(&f.0, id()?).unwrap_err(),
        LockError::UnsafePath
    );
    fs::remove_file(&path).unwrap();
    fs::hard_link(&other, &path).unwrap();
    assert_eq!(
        acquire_checkout_lock(&f.0, id()?).unwrap_err(),
        LockError::UnsafePath
    );
    Ok(())
}

#[test]
fn refuses_symlinked_directory() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    fs::remove_dir(f.0.join("checkouts/.locks")).unwrap();
    symlink(&f.0, f.0.join("checkouts/.locks")).unwrap();
    assert_eq!(
        acquire_checkout_lock(&f.0, id()?).unwrap_err(),
        LockError::UnsafePath
    );
    Ok(())
}

#[test]
fn independent_repositories_have_independent_locks() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    let other = RepoId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6c")?;
    let _first = acquire_checkout_lock(&f.0, id()?).unwrap();
    let _second = acquire_checkout_lock(&f.0, other).unwrap();
    Ok(())
}

#[test]
fn refuses_nonprivate_directories_and_lock_file() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let f = Fixture::new()?;
    let locks = f.0.join("checkouts/.locks");
    fs::set_permissions(&locks, fs::Permissions::from_mode(0o755))?;
    assert_eq!(
        acquire_checkout_lock(&f.0, id()?).unwrap_err(),
        LockError::UnsafePath
    );
    fs::set_permissions(&locks, fs::Permissions::from_mode(0o700))?;
    let path = locks.join(format!("{}.lock", id()?));
    fs::write(&path, b"")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
    assert_eq!(
        acquire_checkout_lock(&f.0, id()?).unwrap_err(),
        LockError::UnsafePath
    );
    Ok(())
}

#[test]
fn checkout_capability_binds_id_and_rejects_symlink() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    let repo = id()?;
    let other = RepoId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6c")?;
    let guard = acquire_checkout_lock(&f.0, repo).unwrap();
    assert!(matches!(guard.checkout(other), Err(LockError::UnsafePath)));
    let capability = guard.checkout(repo).unwrap();
    let checkout = f.0.join("checkouts").join(repo.to_string());
    assert_eq!(capability.path().unwrap(), checkout.as_path());
    symlink(&f.0, &checkout)?;
    assert!(matches!(capability.path(), Err(LockError::UnsafePath)));
    Ok(())
}

#[test]
fn checkout_capability_rejects_replaced_lock() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    let repo = id()?;
    let guard = acquire_checkout_lock(&f.0, repo).unwrap();
    let capability = guard.checkout(repo).unwrap();
    let lock = f.0.join("checkouts/.locks").join(format!("{repo}.lock"));
    fs::rename(&lock, lock.with_extension("old"))?;
    fs::write(&lock, b"")?;
    assert!(matches!(capability.path(), Err(LockError::UnsafePath)));
    Ok(())
}

#[test]
fn checkout_capability_rejects_replaced_parent_directory() -> Result<(), Box<dyn std::error::Error>>
{
    let f = Fixture::new()?;
    let repo = id()?;
    let guard = acquire_checkout_lock(&f.0, repo).map_err(|e| format!("lock: {e:?}"))?;
    let capability = guard
        .checkout(repo)
        .map_err(|e| format!("checkout: {e:?}"))?;
    let locks = f.0.join("checkouts/.locks");
    fs::rename(&locks, f.0.join("checkouts/old-locks"))?;
    fs::DirBuilder::new().mode(0o700).create(&locks)?;
    assert!(matches!(capability.path(), Err(LockError::UnsafePath)));
    Ok(())
}

#[test]
fn checkout_capability_rejects_permissions_and_link_changes()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let f = Fixture::new()?;
    let repo = id()?;
    let guard = acquire_checkout_lock(&f.0, repo).map_err(|e| format!("lock: {e:?}"))?;
    let capability = guard
        .checkout(repo)
        .map_err(|e| format!("checkout: {e:?}"))?;
    let checkouts = f.0.join("checkouts");
    fs::set_permissions(&checkouts, fs::Permissions::from_mode(0o755))?;
    assert!(matches!(capability.path(), Err(LockError::UnsafePath)));
    fs::set_permissions(&checkouts, fs::Permissions::from_mode(0o700))?;
    assert!(capability.path().is_ok());
    let lock = checkouts.join(".locks").join(format!("{repo}.lock"));
    fs::hard_link(&lock, f.0.join("extra-link"))?;
    assert!(matches!(capability.path(), Err(LockError::UnsafePath)));
    Ok(())
}

#[test]
fn checkout_capability_rejects_nonprivate_checkout() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let f = Fixture::new()?;
    let repo = id()?;
    let guard = acquire_checkout_lock(&f.0, repo).map_err(|e| format!("lock: {e:?}"))?;
    let capability = guard
        .checkout(repo)
        .map_err(|e| format!("checkout: {e:?}"))?;
    let checkout = f.0.join("checkouts").join(repo.to_string());
    fs::DirBuilder::new().mode(0o700).create(&checkout)?;
    assert!(capability.path().is_ok());
    fs::set_permissions(&checkout, fs::Permissions::from_mode(0o777))?;
    assert!(matches!(capability.path(), Err(LockError::UnsafePath)));
    Ok(())
}
