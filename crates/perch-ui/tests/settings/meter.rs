//! The context meter and the hint beside it: a token count, the share of the model's window, and
//! the command a person finds by reading.

use perch_ui::Shell;
use perch_ui::shell::{context_words, token_words};

use crate::fake::Fake;
use crate::harness::layout;

#[test]
fn the_meter_writes_tokens_and_their_share() {
    assert_eq!(
        context_words(Some(12_345), Some(200_000)).as_deref(),
        Some("12.3K (6%) used"),
        "the count, the rounded share, and the word saying what it is"
    );
    assert_eq!(
        context_words(Some(950), Some(200_000)).as_deref(),
        Some("950 (0%) used"),
        "a count under a thousand is written whole"
    );
    assert_eq!(
        context_words(Some(12_345), None).as_deref(),
        Some("12.3K used"),
        "a provider that states no window leaves the share out rather than inventing one"
    );
    assert_eq!(
        context_words(None, Some(200_000)),
        None,
        "a session with no turn has nothing measured, so the meter says nothing"
    );
}

#[test]
fn a_token_count_is_one_decimal_of_a_thousand_or_a_million() {
    assert_eq!(token_words(0), "0");
    assert_eq!(token_words(999), "999");
    assert_eq!(token_words(1_000), "1.0K");
    assert_eq!(token_words(12_345), "12.3K");
    // 999,950 would otherwise round to `1000.0K`.
    assert_eq!(token_words(999_950), "1.0M");
    assert_eq!(token_words(2_400_000), "2.4M");
}

#[test]
fn the_composer_draws_the_meter_and_names_compact() {
    let fake = Fake {
        tokens: Some(12_345),
        limits: [("test/model".to_string(), 200_000)].into(),
        ..Fake::default()
    };
    let runtime = layout(Shell::new(Vec::new(), 0).with_live(fake));
    let tree = runtime.accessibility_json().to_string();

    assert!(
        tree.contains("12.3K (6%) used"),
        "the meter is drawn beside the composer:\n{tree}"
    );
    assert!(
        tree.contains("/compact compacts the conversation"),
        "the command is discoverable without being told:\n{tree}"
    );
}

#[test]
fn a_fresh_session_shows_the_hint_but_no_meter() {
    let runtime = layout(Shell::new(Vec::new(), 0).with_live(Fake::default()));
    let tree = runtime.accessibility_json().to_string();

    assert!(
        tree.contains("/compact compacts the conversation"),
        "the hint is there before any turn:\n{tree}"
    );
    assert!(
        !tree.contains("% used") && !tree.contains("K used"),
        "a session that has spent nothing does not claim a measurement:\n{tree}"
    );
}
