//! File identity on every platform the transaction supports. These tests
//! run on Windows too, where the identity comes from an open handle: an
//! identity that compared equal for any two files would pass every
//! replacement check in the transaction.

use super::confine::{file_identity, is_same_file};
use super::verify::hash_file_with_identity;
use std::fs;
use tempfile::TempDir;

#[test]
fn two_files_have_different_identities() {
    let directory = TempDir::new().unwrap();
    let first = directory.path().join("first.md");
    let second = directory.path().join("second.md");
    fs::write(&first, "same bytes\n").unwrap();
    fs::write(&second, "same bytes\n").unwrap();

    assert_ne!(
        file_identity(&first).unwrap(),
        file_identity(&second).unwrap()
    );
    assert!(!is_same_file(&first, &second).unwrap());
}

#[test]
fn hard_link_has_the_identity_of_its_target() {
    let directory = TempDir::new().unwrap();
    let target = directory.path().join("target.md");
    let link = directory.path().join("link.md");
    fs::write(&target, "content\n").unwrap();
    fs::hard_link(&target, &link).unwrap();

    assert_eq!(
        file_identity(&target).unwrap(),
        file_identity(&link).unwrap()
    );
    assert!(is_same_file(&target, &link).unwrap());
    assert!(is_same_file(&target, &target).unwrap());
}

#[test]
fn open_handle_and_path_agree_on_the_identity() {
    let directory = TempDir::new().unwrap();
    let first = directory.path().join("first.md");
    let second = directory.path().join("second.md");
    fs::write(&first, "first\n").unwrap();
    fs::write(&second, "second\n").unwrap();

    let (_, first_identity) = hash_file_with_identity(&first).unwrap();
    let (_, second_identity) = hash_file_with_identity(&second).unwrap();

    assert_eq!(first_identity, file_identity(&first).unwrap());
    assert_eq!(second_identity, file_identity(&second).unwrap());
    assert_ne!(first_identity, second_identity);
}

#[test]
fn replaced_file_has_a_new_identity() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("source.md");
    let replacement = directory.path().join("replacement.md");
    fs::write(&path, "content\n").unwrap();
    let (_, before) = hash_file_with_identity(&path).unwrap();

    // The replacement exists beside the original before the rename, so
    // the file system cannot give it the original's identity.
    fs::write(&replacement, "content\n").unwrap();
    fs::rename(&replacement, &path).unwrap();

    assert_ne!(before, file_identity(&path).unwrap());
}

#[test]
fn directory_has_no_file_identity() {
    let directory = TempDir::new().unwrap();

    let error = file_identity(directory.path()).unwrap_err();
    assert!(
        error.to_string().contains("is not a regular file"),
        "{error}"
    );
    assert!(hash_file_with_identity(directory.path()).is_err());
}

#[test]
fn missing_file_has_no_identity() {
    let directory = TempDir::new().unwrap();
    let missing = directory.path().join("missing.md");

    assert!(file_identity(&missing).is_err());
    assert!(is_same_file(&missing, &missing).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_has_no_file_identity() {
    let directory = TempDir::new().unwrap();
    let target = directory.path().join("target.md");
    let link = directory.path().join("link.md");
    fs::write(&target, "content\n").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let error = file_identity(&link).unwrap_err();
    assert!(
        error.to_string().contains("is not a regular file"),
        "{error}"
    );
    assert!(hash_file_with_identity(&link).is_err());
}

// A file system reports zero when it keeps no index, and all bits set
// when it cannot give a file a unique one. Two such files would compare
// equal, so neither value is an identity.
#[cfg(windows)]
#[test]
fn index_without_a_unique_value_names_no_file() {
    use super::confine::index_names_one_file;

    assert!(!index_names_one_file(0));
    assert!(!index_names_one_file(u64::MAX));
    assert!(index_names_one_file(1));
    assert!(index_names_one_file(u64::MAX - 1));
}

// A symlink needs a privilege on Windows that a test process may not
// hold, so the test passes without a link when the system refuses one.
#[cfg(windows)]
#[test]
fn symlink_has_no_file_identity() {
    let directory = TempDir::new().unwrap();
    let target = directory.path().join("target.md");
    let link = directory.path().join("link.md");
    fs::write(&target, "content\n").unwrap();
    if std::os::windows::fs::symlink_file(&target, &link).is_err() {
        return;
    }

    let error = file_identity(&link).unwrap_err();
    assert!(
        error.to_string().contains("is not a regular file"),
        "{error}"
    );
    assert!(hash_file_with_identity(&link).is_err());
}
