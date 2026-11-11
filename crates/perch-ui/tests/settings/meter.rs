//! The context meter and the hint beside it: a token count, the share of the model's window, what
//! the turn cost in dollars, and the command a person finds by reading.

use perch_ui::Shell;
use perch_ui::shell::{context_words, meter_words, price_words, token_words};

use crate::fake::Fake;
use crate::harness::layout;

#[test]
fn the_meter_writes_tokens_and_their_share() {
    assert_eq!(
        context_words(Some(12_345), Some(200_000)).as_deref(),
        Some("12.3K (6%)"),
        "the count and the rounded share"
    );
    assert_eq!(
        context_words(Some(950), Some(200_000)).as_deref(),
        Some("950 (0%)"),
        "a count under a thousand is written whole"
    );
    assert_eq!(
        context_words(Some(12_345), None).as_deref(),
        Some("12.3K"),
        "a provider that states no window leaves the share out rather than inventing one"
    );
    assert_eq!(
        context_words(None, Some(200_000)),
        None,
        "a session with no turn has nothing measured, so the meter says nothing"
    );
}

#[test]
fn a_turn_that_states_a_price_costs_money_beside_the_tokens() {
    assert_eq!(
        meter_words(Some(12_345), Some(200_000), Some(0.0005)).as_deref(),
        Some("12.3K (6%) · $0.0005"),
        "tokens, share and dollars, beside each other"
    );
    assert_eq!(
        meter_words(Some(12_345), Some(200_000), None).as_deref(),
        Some("12.3K (6%)"),
        "no stated price means no money at all, never a zero"
    );
    assert_eq!(
        meter_words(Some(12_345), None, None).as_deref(),
        Some("12.3K"),
        "neither a window nor a price leaves only the count"
    );
    assert_eq!(
        meter_words(Some(12_345), None, Some(0.0042)).as_deref(),
        Some("12.3K · $0.0042"),
        "a price without a window still shows what the turn cost"
    );
    assert_eq!(
        meter_words(None, Some(200_000), None),
        None,
        "a turn that reported no usage has nothing to price"
    );
}

#[test]
fn dollars_keep_four_decimals_until_a_dollar_then_two() {
    assert_eq!(price_words(0.0005), "$0.0005");
    assert_eq!(price_words(0.00004), "$0.0000");
    assert_eq!(price_words(0.0), "$0.0000");
    assert_eq!(price_words(0.9999), "$0.9999");
    assert_eq!(price_words(1.0), "$1.00");
    assert_eq!(price_words(1.2345), "$1.23");
    assert_eq!(price_words(42.5), "$42.50");
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
        cost: Some(0.0005),
        limits: [("test/model".to_string(), 200_000)].into(),
        ..Fake::default()
    };
    let runtime = layout(Shell::new(Vec::new(), 0).with_live(fake));
    let tree = runtime.accessibility_json().to_string();

    assert!(
        tree.contains("12.3K (6%) · $0.0005"),
        "the meter is drawn beside the composer, money included:\n{tree}"
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
        !tree.contains('$') && !tree.contains("K ("),
        "a session that has spent nothing claims no measurement and no bill:\n{tree}"
    );
}
