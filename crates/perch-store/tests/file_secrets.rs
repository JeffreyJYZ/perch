//! The file store: a key survives a reopen, and the file is readable only by its owner.

use std::os::unix::fs::PermissionsExt;

use perch_store::secret::{FileSecrets, SecretStore};

#[test]
fn a_key_survives_a_reopen_and_the_file_is_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");

    let first = FileSecrets::new(&path);
    first.store("provider", "sk-test-key").unwrap();
    assert_eq!(
        first.load("provider").unwrap().as_deref(),
        Some("sk-test-key"),
        "a store reads back the key it just wrote"
    );

    // A new process reads the same file with no keychain in the way: this is what a rebuilt,
    // ad-hoc-signed binary could not do with the login keychain.
    let second = FileSecrets::new(&path);
    assert_eq!(
        second.load("provider").unwrap().as_deref(),
        Some("sk-test-key")
    );

    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "the key file must be readable only by its owner"
    );

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("sk-test-key"), "{text}");
    assert!(
        !text.contains("sk-test-key\n") || text.trim_end().ends_with('}'),
        "the file is JSON, not a bare key: {text}"
    );

    assert!(
        second.load("nothing").unwrap().is_none(),
        "an account that was never stored answers nothing, not an error"
    );
}

#[test]
fn deleting_a_key_removes_it_from_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    let store = FileSecrets::new(&path);
    store.store("provider", "sk-test-key").unwrap();

    store.delete("provider").unwrap();

    assert_eq!(store.load("provider").unwrap(), None);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("sk-test-key"), "{text}");
}
