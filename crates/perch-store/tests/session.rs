use std::io::Write;
use std::path::{Path, PathBuf};

use perch_store::{SessionEvent, SessionLog, SessionMeta, Settings, meta_path};

fn meta_for(dir: &Path, id: &str) -> SessionMeta {
    SessionMeta {
        id: id.to_string(),
        title: id.to_string(),
        worktree: dir.to_path_buf(),
        branch: "main".to_string(),
        model: "test/model".to_string(),
        created: 0,
    }
}

fn meta_path_for(dir: &Path, id: &str) -> PathBuf {
    meta_path(dir, id)
}

#[test]
fn append_then_read_returns_events_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let meta = meta_for(dir.path(), "perch-dev");
    let mut log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    log.append(&SessionEvent::User { text: "hi".into() })
        .unwrap();
    log.append(&SessionEvent::Assistant {
        text: "hello".into(),
    })
    .unwrap();
    assert_eq!(
        SessionLog::open(dir.path(), meta)
            .unwrap()
            .events()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn a_truncated_last_line_is_dropped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let meta = meta_for(dir.path(), "perch-dev");
    let mut log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    log.append(&SessionEvent::User { text: "hi".into() })
        .unwrap();
    drop(log);
    let path = meta_path_for(dir.path(), &meta.id);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"kind\":\"assis")
        .unwrap();
    assert_eq!(
        SessionLog::open(dir.path(), meta)
            .unwrap()
            .events()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_new_title_rewrites_the_header_and_keeps_every_event() {
    let dir = tempfile::tempdir().unwrap();
    let meta = meta_for(dir.path(), "perch-dev");
    let mut log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    log.append(&SessionEvent::User { text: "hi".into() })
        .unwrap();
    log.set_title("Renamed session").unwrap();
    drop(log);

    let text = std::fs::read_to_string(meta_path_for(dir.path(), &meta.id)).unwrap();
    let header: SessionMeta = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(header.title, "Renamed session");
    assert_eq!(
        SessionLog::open(dir.path(), meta)
            .unwrap()
            .events()
            .unwrap()
            .len(),
        1,
        "the conversation is untouched"
    );
}

#[test]
fn removing_a_log_deletes_the_file_and_its_header() {
    let dir = tempfile::tempdir().unwrap();
    let meta = meta_for(dir.path(), "perch-dev");
    let log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    let path = meta_path_for(dir.path(), &meta.id);
    assert!(path.exists());
    log.remove().unwrap();
    assert!(
        !path.exists(),
        "a delete that kept the file would revive it"
    );
}

#[test]
fn settings_default_when_absent_and_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    assert_eq!(settings, Settings::default());
    assert!(dir.path().join("settings.toml").exists());

    let changed = Settings {
        model: "some/model".to_string(),
        ..settings
    };
    changed.save(dir.path()).unwrap();
    assert_eq!(Settings::load(dir.path()).unwrap(), changed);
}
