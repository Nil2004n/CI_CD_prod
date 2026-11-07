#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use application::repository_service::{CheckoutAccess, LocalCheckoutAccess, RepositoryError};
use domain::ids::RepoId;
use persistence::fs::RealFs;
use persistence::paths::DataDir;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "p04-checkout-adapter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        fs::create_dir(path.join("checkouts"))?;
        fs::set_permissions(path.join("checkouts"), fs::Permissions::from_mode(0o700))?;
        fs::create_dir(path.join("checkouts/.locks"))?;
        fs::set_permissions(
            path.join("checkouts/.locks"),
            fs::Permissions::from_mode(0o700),
        )?;
        Ok(Self(path))
    }

    fn adapter(&self) -> Result<LocalCheckoutAccess, Box<dyn std::error::Error>> {
        Ok(LocalCheckoutAccess::new(DataDir::new(&RealFs, &self.0)?))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn id(value: &str) -> Result<RepoId, Box<dyn std::error::Error>> {
    Ok(RepoId::parse(value)?)
}

#[test]
fn guard_binds_repository_and_data_directory() -> Result<(), Box<dyn std::error::Error>> {
    let first = Fixture::new()?;
    let second = Fixture::new()?;
    let mut adapter = first.adapter()?;
    let other_adapter = second.adapter()?;
    let repo = id("00000000-0000-4000-8000-000000000001")?;
    let other = id("00000000-0000-4000-8000-000000000002")?;
    let guard = adapter.lock(repo).unwrap();
    assert_eq!(
        adapter.checkout_path(repo, &guard).unwrap(),
        first.0.join("checkouts").join(repo.to_string())
    );
    assert_eq!(
        adapter.checkout_path(other, &guard),
        Err(RepositoryError::Unsafe)
    );
    assert_eq!(
        other_adapter.checkout_path(repo, &guard),
        Err(RepositoryError::Unsafe)
    );
    assert!(!format!("{adapter:?}").contains(first.0.to_str().unwrap()));
    Ok(())
}

#[test]
fn refuses_delete_and_revalidates_checkout_path() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let mut adapter = fixture.adapter()?;
    let repo = id("00000000-0000-4000-8000-000000000003")?;
    let path = fixture.0.join("checkouts").join(repo.to_string());
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(path.join("marker"), b"synthetic").unwrap();
    let guard = adapter.lock(repo).unwrap();
    assert!(adapter.checkout_path(repo, &guard).is_ok());
    assert_eq!(adapter.delete(repo, &guard), Err(RepositoryError::Unsafe));
    assert_eq!(fs::read(path.join("marker")).unwrap(), b"synthetic");

    let link_repo = id("00000000-0000-4000-8000-000000000004")?;
    let link_guard = adapter.lock(link_repo).unwrap();
    symlink(
        &path,
        fixture.0.join("checkouts").join(link_repo.to_string()),
    )
    .unwrap();
    assert_eq!(
        adapter.checkout_path(link_repo, &link_guard),
        Err(RepositoryError::Unsafe)
    );
    Ok(())
}
