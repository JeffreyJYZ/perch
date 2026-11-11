//! The provider's list: whatever it returns, sorted, and a failure in words.

use perch_agent::Price;

use crate::harness::{Answer, Table, app, defaults};
use crate::lazy::wait_for_models;

#[test]
fn the_listed_window_and_prices_reach_the_app() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let table = Table::new(Answer::Body(
        r#"{"data":[
            {"id":"test/model","context_length":200000,
             "pricing":{"prompt":"0.00000015","completion":"0.0000006"}}
        ]}"#,
    ));
    let mut app =
        app(&data, defaults("https://provider.test/v1", "test/model")).with_listings(table);
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();

    assert_eq!(
        app.turn_cost(),
        None,
        "a session with no turn has no usage to price"
    );

    app.start_models();
    wait_for_models(&mut app).expect("the table answers");

    assert_eq!(
        app.model_limit(),
        Some(200_000),
        "the window the listing stated is reachable from the app"
    );
    assert_eq!(
        app.model_price(),
        Some(Price {
            prompt: 0.00000015,
            completion: 0.0000006,
        }),
        "the price the listing stated is kept beside the window, in dollars per token"
    );
    assert_eq!(
        app.turn_cost(),
        None,
        "a price is not a bill: only a turn that reported usage has a cost"
    );
}

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
