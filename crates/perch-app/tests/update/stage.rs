//! The press: the archive is downloaded, verified and named, and never installed.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use perch_app::update::{OWNER, STAGE, UPDATES};
use perch_ui::shell::Panels;
use tempfile::TempDir;

use crate::harness::{ARCHIVE, Fake, HELLO, MANIFEST, app, data, line, manifest, pump_until};

/// Where the worker stages `version` under `dir`.
fn staged(dir: &TempDir, version: &str) -> PathBuf {
    dir.path()
        .join(UPDATES)
        .join(format!("perch-{version}.zip"))
}

/// An app that has been pumped until the offer stands.
fn offering(dir: &TempDir, fake: Arc<Fake>) -> perch_app::AppState {
    let mut app = app(dir, fake);
    assert!(
        pump_until(&mut app, |app| line(app)
            .is_some_and(|words| words.contains("available"))),
        "no offer arrived: {:?}",
        line(&app)
    );
    app
}

#[test]
fn a_press_downloads_verifies_and_reports_where_the_archive_landed() {
    let fake = Fake::new(&[(MANIFEST, &manifest("99.0.0", HELLO)), (ARCHIVE, "hello")]);
    let dir = data(Some(MANIFEST));
    let mut app = offering(&dir, Arc::clone(&fake));

    Panels::panel_action(&mut app, OWNER, STAGE, None).expect("the offer is pressable");
    assert!(
        pump_until(&mut app, |app| line(app)
            .is_some_and(|words| words.contains("staged at"))),
        "the press said nothing: {:?}",
        line(&app)
    );

    let dest = staged(&dir, "99.0.0");
    assert_eq!(
        std::fs::read(&dest).expect("the verified archive landed"),
        b"hello"
    );
    let words = line(&app).expect("there is a line");
    assert!(
        words.contains(&dest.display().to_string()),
        "the line names where it landed: {words}"
    );
    assert!(
        words.contains("installing it is a manual step"),
        "installing is manual, and the line says so: {words}"
    );
}

#[test]
fn nothing_is_downloaded_before_the_press() {
    let fake = Fake::new(&[(MANIFEST, &manifest("99.0.0", HELLO)), (ARCHIVE, "hello")]);
    let dir = data(Some(MANIFEST));
    let app = offering(&dir, Arc::clone(&fake));

    assert_eq!(
        fake.asked(),
        vec![MANIFEST.to_string()],
        "only the manifest was read"
    );
    assert!(
        !dir.path().join(UPDATES).exists(),
        "an offer alone puts no file on disk"
    );
    assert!(line(&app).is_some_and(|words| words.contains("available")));
}

#[test]
fn a_checksum_mismatch_is_a_stated_failure_and_leaves_nothing_behind() {
    let fake = Fake::new(&[(MANIFEST, &manifest("99.0.0", HELLO)), (ARCHIVE, "perch")]);
    let dir = data(Some(MANIFEST));
    let mut app = offering(&dir, Arc::clone(&fake));

    Panels::panel_action(&mut app, OWNER, STAGE, None).expect("the offer is pressable");
    assert!(
        pump_until(&mut app, |app| line(app)
            .is_some_and(|words| words.contains("failed"))),
        "the mismatch was not said: {:?}",
        line(&app)
    );

    let words = line(&app).expect("there is a line");
    assert!(words.contains("sha256 mismatch"), "{words}");
    assert!(words.contains("deleted"), "{words}");
    assert!(
        !staged(&dir, "99.0.0").exists(),
        "an archive nothing vouched for stays off the disk"
    );
}

#[test]
fn a_press_with_no_offer_is_refused_in_words_and_fetches_nothing() {
    let fake = Fake::new(&[
        (MANIFEST, &manifest(env!("CARGO_PKG_VERSION"), HELLO)),
        (ARCHIVE, "hello"),
    ]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, Arc::clone(&fake));
    assert!(pump_until(&mut app, |app| line(app).as_deref() == Some("no update")));

    let refused = Panels::panel_action(&mut app, OWNER, STAGE, None)
        .expect_err("there is nothing on offer to stage");
    assert!(refused.contains("no update on offer"), "{refused}");
    assert_eq!(
        fake.asked(),
        vec![MANIFEST.to_string()],
        "a refused press fetched nothing"
    );
}

#[test]
fn a_press_before_the_check_finishes_is_refused_in_words() {
    let fake = Fake::slow(
        &[(MANIFEST, &manifest("99.0.0", HELLO))],
        Duration::from_millis(1000),
    );
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);

    let refused = Panels::panel_action(&mut app, OWNER, STAGE, None)
        .expect_err("the check has not answered yet");
    assert!(refused.contains("has not finished"), "{refused}");
}
