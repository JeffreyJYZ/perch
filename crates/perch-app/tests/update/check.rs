//! The check: it runs off the frame thread, and its answer is one line in words.

use std::sync::Arc;
use std::time::{Duration, Instant};

use perch_app::update::STAGE;

use crate::harness::{
    Fake, HELLO, MANIFEST, TIMEOUT, app, data, entry, line, lines, manifest, pump_until,
};

/// How long the fake manifest takes to answer: longer than any frame may be held.
const SLOW: Duration = Duration::from_millis(2000);

#[test]
fn frames_keep_coming_while_the_manifest_is_still_being_fetched() {
    let fake = Fake::slow(&[(MANIFEST, &manifest("99.0.0", HELLO))], SLOW);
    let dir = data(Some(MANIFEST));

    let began = Instant::now();
    let mut app = app(&dir, Arc::clone(&fake));
    let opened = began.elapsed();

    assert!(
        opened < SLOW,
        "opening the app took {opened:?}, so it waited for the manifest"
    );

    let mut slowest = Duration::ZERO;
    let mut arrived = None;
    while began.elapsed() < TIMEOUT {
        let frame = Instant::now();
        app.pump(Duration::from_millis(1));
        slowest = slowest.max(frame.elapsed());
        if line(&app).is_some_and(|words| words.contains("available")) {
            arrived = Some(began.elapsed());
            break;
        }
    }
    let arrived = arrived.unwrap_or_else(|| panic!("no offer arrived within {TIMEOUT:?}"));

    assert!(
        arrived >= SLOW,
        "the offer arrived in {arrived:?}, before the server could have answered"
    );
    assert!(
        slowest < perch_app::watch::DEBOUNCE,
        "a frame took {slowest:?} while the manifest was being fetched; the debounce is {:?}, \
         so that frame stalled the window",
        perch_app::watch::DEBOUNCE
    );
    assert!(
        fake.threads()
            .iter()
            .all(|thread| *thread != std::thread::current().id()),
        "the request was made on the frame thread itself"
    );
}

#[test]
fn the_offer_is_one_line_naming_the_version_and_its_notes() {
    let fake = Fake::new(&[(MANIFEST, &manifest("99.0.0", HELLO))]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);

    assert!(
        pump_until(&mut app, |app| line(app)
            .is_some_and(|words| words.contains("available"))),
        "no offer arrived: {:?}",
        lines(&app)
    );
    assert_eq!(
        lines(&app),
        vec!["update 99.0.0 available: Fixed the updater.".to_string()],
        "the offer is one line, with the release's notes"
    );

    let entry = entry(&app).expect("the column draws the offer");
    assert_eq!(entry.model.title, "Update");
    assert!(
        entry.model.has_control(STAGE),
        "an offer names the control that stages it: {:?}",
        entry.model.sections
    );
}

#[test]
fn a_release_that_is_not_newer_says_no_update_and_offers_nothing() {
    let fake = Fake::new(&[(MANIFEST, &manifest(env!("CARGO_PKG_VERSION"), HELLO))]);
    let dir = data(Some(MANIFEST));
    let mut app = app(&dir, fake);

    assert!(
        pump_until(&mut app, |app| line(app).as_deref() == Some("no update")),
        "the check said {:?}",
        lines(&app)
    );
    let entry = entry(&app).expect("the result is still one line");
    assert!(
        !entry.model.has_control(STAGE),
        "there is nothing to download, so there is no button"
    );
}

#[test]
fn an_empty_url_never_reaches_the_network() {
    let fake = Fake::new(&[(MANIFEST, &manifest("99.0.0", HELLO))]);
    let dir = data(None);
    let mut app = app(&dir, Arc::clone(&fake));

    for _ in 0..20 {
        app.pump(Duration::from_millis(5));
    }

    assert!(
        fake.asked().is_empty(),
        "an unconfigured window asked for {:?}",
        fake.asked()
    );
    assert!(
        entry(&app).is_none(),
        "an unconfigured window drew an update line"
    );
    assert!(!app.busy(), "nothing is left pending for the frame");
}
