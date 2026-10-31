//! P03-T09 (application half): the FR-33 store error maps onto `DevError::RootForeignDataDir`, class
//! Operation (CLI exit 5), with a fixed English message that carries no path and no uid
//! (`planning/P03-T01-request.md` §5.6, §8 T09). The end-to-end CLI exit-5 scenario belongs to T10.

use application::dev_store::{DevError, FailureClass};
use persistence::error::StoreError;

/// Would fail if the mapping went to another variant (for example `DataDirRejected`).
#[test]
fn root_foreign_data_dir_maps_to_its_own_dev_error() {
    assert_eq!(
        DevError::from(StoreError::RootForeignDataDir),
        DevError::RootForeignDataDir
    );
    assert_ne!(
        DevError::from(StoreError::RootForeignDataDir),
        DevError::from(StoreError::PathRejected)
    );
}

/// Would fail if the class moved: `Operation` selects exit 5 in the CLI, and the three storage classes
/// select other exits.
#[test]
fn root_foreign_data_dir_is_an_operation_failure() {
    assert_eq!(
        DevError::RootForeignDataDir.class(),
        FailureClass::Operation
    );
    assert_eq!(
        DevError::from(StoreError::RootForeignDataDir).class(),
        FailureClass::Operation
    );
}

/// Fixed English text, no path, no uid, identical at both layers. Would fail if the text changed or
/// started to interpolate a value.
#[test]
fn root_foreign_data_dir_message_is_fixed_and_carries_no_path_or_uid() {
    const TEXT: &str = "refusing to use a data directory not owned by root while running as root";
    assert_eq!(DevError::RootForeignDataDir.to_string(), TEXT);
    assert_eq!(StoreError::RootForeignDataDir.to_string(), TEXT);
    assert!(!TEXT.contains('/'));
    assert!(
        !DevError::RootForeignDataDir
            .to_string()
            .chars()
            .any(|c| c.is_ascii_digit())
    );
    let debug = format!("{:?}", DevError::RootForeignDataDir);
    assert_eq!(debug, "RootForeignDataDir");
}
