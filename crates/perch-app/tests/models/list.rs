//! The provider's list: whatever it returns, sorted, and a failure in words.

use crate::harness::{Answer, Table, app, defaults};

#[test]
fn the_list_is_the_providers_sorted_and_free_models_are_not_privileged() {
    let data = tempfile::tempdir().unwrap();
    let table = Table::new(Answer::Body(
        r#"{"data":[{"id":"vendor/zeta"},{"id":"free/alpha:free"},{"id":"vendor/zeta"},{"id":"free/beta:free"}]}"#,
    ));
    let app = app(&data, defaults("https://provider.test/v1", "")).with_listings(table.clone());

    let models = app.list_models().expect("the table answers");

    assert_eq!(
        models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["free/alpha:free", "free/beta:free", "vendor/zeta"],
        "sorted, deduplicated, and a free id sits where its name sorts"
    );
    assert_eq!(
        table.url(0),
        "https://provider.test/v1/models",
        "the listing is the base URL plus /models"
    );
}

#[test]
fn a_provider_without_the_endpoint_says_so_in_words() {
    let data = tempfile::tempdir().unwrap();
    let app = app(&data, defaults("https://provider.test/v1", ""))
        .with_listings(Table::new(Answer::Status(404)));

    let words = app.list_models().expect_err("a 404 lists nothing");

    assert!(
        words.contains("404"),
        "the failure names the provider's answer: {words}"
    );
}
