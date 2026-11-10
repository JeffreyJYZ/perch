//! Validation: every bad field is named, and a refusal keeps the panel open.

use perch_ui::Shell;

use crate::harness::draft;

#[test]
fn each_bad_field_is_named_in_words() {
    let url = draft("not a url", "test/model", "dark", "")
        .validate()
        .expect_err("a URL that does not parse is refused");
    assert_eq!(url.field, perch_ui::Field::BaseUrl);
    assert!(
        url.words.contains("base URL"),
        "the refusal names the field: {}",
        url.words
    );

    let model = draft("https://example.com/v1", "  ", "dark", "")
        .validate()
        .expect_err("an empty model is refused");
    assert_eq!(model.field, perch_ui::Field::Model);
    assert!(model.words.contains("model"), "{}", model.words);

    let theme = draft("https://example.com/v1", "test/model", "solarized", "")
        .validate()
        .expect_err("a theme the window cannot draw is refused");
    assert_eq!(theme.field, perch_ui::Field::Theme);
    assert!(theme.words.contains("theme"), "{}", theme.words);

    // An address has to be reachable over HTTP: it is a provider root, not any URL.
    let scheme = draft("ftp://example.com", "test/model", "dark", "")
        .validate()
        .expect_err("a non-HTTP address is refused");
    assert_eq!(scheme.field, perch_ui::Field::BaseUrl);

    draft("https://example.com/v1", "test/model", "dark", "")
        .validate()
        .expect("a stated draft is valid");
    draft("http://127.0.0.1:8080/v1", "test/model", "light", "")
        .validate()
        .expect("a local provider is valid");
}

#[test]
fn a_bad_field_keeps_the_panel_open_and_nothing_is_written() {
    let mut shell = Shell::idle(Vec::new(), 0);
    shell.toggle_settings();
    shell.stage_settings(draft("not a url", "test/model", "dark", ""));

    assert!(!shell.save_settings(), "nothing was saved");
    assert!(shell.settings_open(), "the panel stays open");
    let error = shell
        .settings_panel()
        .and_then(|panel| panel.error().cloned())
        .expect("the panel says which field refused");
    assert_eq!(error.field, perch_ui::Field::BaseUrl);
    assert_eq!(
        shell
            .settings_panel()
            .and_then(|panel| panel.notice())
            .map(str::to_string),
        None,
        "a refused field is an error, not a notice"
    );
}
