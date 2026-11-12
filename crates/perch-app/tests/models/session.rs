//! What picking a model does: this session's next turn, its log header, and the next session's
//! default — while the saved settings file is not touched.

use perch_app::ProviderDefaults;

use crate::harness::{app, defaults, header_model, said};

#[test]
fn a_pick_changes_this_session_only_and_its_log_header() {
    let data = tempfile::tempdir().unwrap();
    let first_worktree = tempfile::tempdir().unwrap();
    let second_worktree = tempfile::tempdir().unwrap();
    let mut app = app(&data, defaults("https://provider.test/v1", "first/model"));

    let first = app.new_session(first_worktree.path()).unwrap();
    let second = app.new_session(second_worktree.path()).unwrap();
    assert_eq!(app.model().as_deref(), Some("first/model"));

    app.switch(first).unwrap();
    app.choose_model("second/model")
        .expect("a model id is picked");

    assert_eq!(
        app.model().as_deref(),
        Some("second/model"),
        "the session the pick was made in asks for it"
    );
    let picked = app.metas()[first].clone();
    assert_eq!(picked.model, "second/model");
    assert_eq!(
        header_model(&data, &picked),
        "second/model",
        "the log header says which model the session asks for"
    );

    app.switch(second).unwrap();
    assert_eq!(
        app.model().as_deref(),
        Some("first/model"),
        "the other session kept its own model"
    );
    assert_eq!(
        header_model(&data, &app.metas()[second].clone()),
        "first/model"
    );

    // A session opened after the pick inherits it.
    let third = tempfile::tempdir().unwrap();
    let third = app.new_session(third.path()).unwrap();
    assert_eq!(
        app.metas()[third].model,
        "second/model",
        "a new session inherits the last chosen model"
    );
}

#[test]
fn an_empty_pick_is_refused_in_words() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = app(&data, defaults("https://provider.test/v1", "first/model"));
    app.new_session(worktree.path()).unwrap();

    let error = app.choose_model("  ").expect_err("an empty id is refused");

    assert!(
        error.to_string().contains("model id"),
        "the refusal says what is missing: {error}"
    );
    assert_eq!(app.model().as_deref(), Some("first/model"));
}

#[test]
fn a_fresh_profile_has_no_model_and_sending_says_what_is_missing() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let mut app = app(&data, ProviderDefaults::default());
    app.new_session(worktree.path()).unwrap();

    assert_eq!(app.model().as_deref(), Some(""), "nothing is chosen yet");
    let error = app.prompt("hello").expect_err("a turn needs a model");

    assert!(
        error.to_string().contains("no model is chosen"),
        "the refusal names what is missing: {error}"
    );
    assert!(
        said(&app).contains("no model is chosen"),
        "the column says it too: {}",
        said(&app)
    );
}
