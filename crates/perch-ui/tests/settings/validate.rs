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
fn the_update_check_is_optional_and_a_bad_one_is_named() {
    // Empty is allowed on purpose: it is the app never reaching the network, which is the default
    // a fresh install gets.
    for empty in ["", "   "] {
        let mut allowed = draft("https://example.com/v1", "test/model", "dark", "");
        allowed.update_check = empty.to_string();
        allowed
            .validate()
            .expect("an empty update check means no check, so it is valid");
    }

    for bad in [
        "not a url",
        "ftp://example.com/latest.json",
        "https://",
        "release.perch.test/latest.json",
    ] {
        let mut refused = draft("https://example.com/v1", "test/model", "dark", "");
        refused.update_check = bad.to_string();
        let invalid = refused
            .validate()
            .expect_err(&format!("`{bad}` is not a manifest URL"));
        assert_eq!(
            invalid.field,
            perch_ui::Field::UpdateCheck,
            "`{bad}` must be refused as the update check field: {}",
            invalid.words
        );
        assert!(
            invalid.words.contains("update check URL"),
            "the refusal names the field: {}",
            invalid.words
        );
    }

    let mut manifest = draft("https://example.com/v1", "test/model", "dark", "");
    manifest.update_check = "https://release.perch.test/latest.json".to_string();
    manifest
        .validate()
        .expect("an http(s) manifest URL is what the field is for");

    // The panel is where the refusal has to land: a bad value keeps it open and names the field.
    let mut shell = Shell::idle(Vec::new(), 0);
    shell.toggle_settings();
    let mut bad = draft("https://example.com/v1", "test/model", "dark", "");
    bad.update_check = "not a url".to_string();
    shell.stage_settings(bad);
    assert!(!shell.save_settings(), "the panel refused the save");
    assert!(shell.settings_open(), "the panel stays open on a refusal");
    let refused = shell
        .settings_panel()
        .and_then(|panel| panel.error())
        .expect("the panel says which field refused");
    assert_eq!(
        refused.field,
        perch_ui::Field::UpdateCheck,
        "the update check field is the one named: {}",
        refused.words
    );
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

#[test]
fn the_font_size_is_optional_and_clamped_rather_than_refused() {
    // Empty is the default scale, which is what a fresh install has.
    let mut empty = draft("https://example.com/v1", "test/model", "dark", "");
    empty.font_size = "  ".to_string();
    empty
        .validate()
        .expect("an empty size is the default scale");
    assert_eq!(empty.font_points(), None, "empty means no override");

    // A number is clamped to the scale's bounds and saves as the size the pane says it draws.
    let mut small = draft("https://example.com/v1", "test/model", "dark", "");
    small.font_size = "8".to_string();
    small
        .validate()
        .expect("8 is a number, so it is not refused");
    assert_eq!(small.font_points(), Some(10.0), "8 draws at the minimum");

    let mut large = draft("https://example.com/v1", "test/model", "dark", "");
    large.font_size = "30".to_string();
    large
        .validate()
        .expect("30 is a number, so it is not refused");
    assert_eq!(large.font_points(), Some(20.0), "30 draws at the maximum");

    let mut exact = draft("https://example.com/v1", "test/model", "dark", "");
    exact.font_size = "14".to_string();
    exact.validate().expect("14 is inside the scale");
    assert_eq!(exact.font_points(), Some(14.0), "an in-range size is kept");

    // Text that is not a number is the one refusal, and it names the field.
    for bad in ["big", "13pt", "inf"] {
        let mut refused = draft("https://example.com/v1", "test/model", "dark", "");
        refused.font_size = bad.to_string();
        let invalid = refused
            .validate()
            .expect_err(&format!("`{bad}` is not a size"));
        assert_eq!(
            invalid.field,
            perch_ui::Field::FontSize,
            "`{bad}` must be refused as the font size field: {}",
            invalid.words
        );
        assert!(
            invalid.words.contains("font size"),
            "the refusal names the field: {}",
            invalid.words
        );
    }
}
