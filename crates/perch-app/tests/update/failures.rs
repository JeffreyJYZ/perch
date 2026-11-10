//! A manifest that is wrong, or a server that will not answer: said in the column, never a crash.

use perch_app::update::STAGE;

use crate::harness::{ARCHIVE, Fake, HELLO, MANIFEST, app, data, entry, line, lines, pump_until};

#[test]
fn a_manifest_field_that_is_missing_is_a_stated_failure() {
    let body = serde_json::json!({
        "version": "99.0.0",
        "url": ARCHIVE,
        "notes": "Fixed the updater.",
        "min_os": 1,
    })
    .to_string();
    let fake = Fake::new(&[(MANIFEST, &body)]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);

    assert!(
        pump_until(&mut app, |app| line(app)
            .is_some_and(|words| words.starts_with("update check failed:"))),
        "the failure was not said: {:?}",
        lines(&app)
    );
    assert_eq!(
        lines(&app),
        vec!["update check failed: manifest field `sha256` is missing".to_string()]
    );
    let entry = entry(&app).expect("a failure is still a line in the column");
    assert!(
        !entry.model.has_control(STAGE),
        "a failed check offers nothing to press"
    );
}

#[test]
fn a_manifest_field_that_is_invalid_names_the_field() {
    let body = serde_json::json!({
        "version": "0.1",
        "url": ARCHIVE,
        "notes": "Fixed the updater.",
        "min_os": 1,
        "sha256": HELLO,
    })
    .to_string();
    let fake = Fake::new(&[(MANIFEST, &body)]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);

    assert!(
        pump_until(&mut app, |app| line(app).is_some_and(
            |words| words.contains("manifest field `version` is invalid")
        )),
        "the failure did not name the field: {:?}",
        lines(&app)
    );
}

#[test]
fn a_manifest_that_cannot_be_fetched_is_a_stated_failure() {
    let fake = Fake::new(&[]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);

    assert!(
        pump_until(&mut app, |app| line(app)
            .is_some_and(|words| words.contains("no route for"))),
        "the transport failure was not said: {:?}",
        lines(&app)
    );
    assert!(
        line(&app).is_some_and(|words| words.starts_with("update check failed:")),
        "{:?}",
        line(&app)
    );
}
