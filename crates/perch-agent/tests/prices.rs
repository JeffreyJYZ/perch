//! The prices a provider states for its models: dollars per token, read beside the window.
//!
//! Every test here answers the one GET from a table, so no socket is opened and no provider is
//! required. The payloads are OpenRouter's shape: `pricing` sits on the same entry as
//! `context_length`, and both rates are strings in dollars per token.

use perch_agent::{ListedModel, ModelTransport, Price, Result, list_models_with};

/// A transport that answers one body, whatever it is asked.
struct Body(String);

impl ModelTransport for Body {
    fn get(&self, _url: &str, _api_key: &str) -> Result<Vec<u8>> {
        Ok(self.0.as_bytes().to_vec())
    }
}

/// The models `payload` lists.
fn listed(payload: &str) -> Vec<ListedModel> {
    list_models_with(&Body(payload.to_string()), "https://provider.test/v1", "").unwrap()
}

fn costs(payload: &str) -> Vec<(String, Option<f64>, Option<f64>)> {
    listed(payload)
        .into_iter()
        .map(|model| (model.id, model.prompt_price, model.completion_price))
        .collect()
}

#[test]
fn a_model_carries_the_prices_the_provider_states() {
    let models = listed(
        r#"{"data":[
            {"id":"vendor/paid","context_length":200000,
             "pricing":{"prompt":"0.00000015","completion":"0.0000006"}},
            {"id":"vendor/numbers","pricing":{"prompt":0.00000015,"completion":0.0000006}},
            {"id":"vendor/free","pricing":{"prompt":"0","completion":"0"}},
            {"id":"vendor/silent"},
            {"id":"vendor/empty","pricing":{}}
        ]}"#,
    );

    assert_eq!(
        models
            .iter()
            .map(|model| (
                model.id.as_str(),
                model.prompt_price,
                model.completion_price
            ))
            .collect::<Vec<_>>(),
        vec![
            ("vendor/empty", None, None),
            // A provider that says zero is saying free, and it parses as a stated zero.
            ("vendor/free", Some(0.0), Some(0.0)),
            // A number and a string are the same statement.
            ("vendor/numbers", Some(0.00000015), Some(0.0000006)),
            // Dollars per token, exactly as the listing spells them.
            ("vendor/paid", Some(0.00000015), Some(0.0000006)),
            ("vendor/silent", None, None),
        ],
        "the price travels with the id, and a model with no pricing states none"
    );
    assert_eq!(
        models[3].context,
        Some(200_000),
        "the price and the window are read from the same entry"
    );
}

#[test]
fn a_rate_that_is_not_money_leaves_both_unstated() {
    for pricing in [
        r#"{"prompt":"free","completion":"0.0000006"}"#,
        r#"{"prompt":"-1","completion":"0.0000006"}"#,
        r#"{"prompt":"0.00000015","completion":"NaN"}"#,
        r#"{"prompt":"0.00000015"}"#,
        r#"{"completion":"0.0000006"}"#,
        r#"{"prompt":true,"completion":"0.0000006"}"#,
    ] {
        let payload = format!(r#"{{"data":[{{"id":"vendor/odd","pricing":{pricing}}}]}}"#);
        assert_eq!(
            costs(&payload),
            vec![("vendor/odd".to_string(), None, None)],
            "`{pricing}` is not a price, so nothing is claimed about what it charges"
        );
    }
}

#[test]
fn a_duplicate_entry_keeps_the_price_the_other_copy_stated() {
    let models = costs(
        r#"{"data":[
            {"id":"vendor/twice"},
            {"id":"vendor/twice","pricing":{"prompt":"0.00000015","completion":"0.0000006"}}
        ]}"#,
    );

    assert_eq!(
        models,
        vec![(
            "vendor/twice".to_string(),
            Some(0.00000015),
            Some(0.0000006)
        )],
        "a listing that names a model twice does not lose the numbers that price it"
    );
}

#[test]
fn a_price_is_dollars_per_token_and_the_cost_is_the_product() {
    let price = Price {
        prompt: 0.15 / 1_000_000.0,
        completion: 0.60 / 1_000_000.0,
    };

    // 1000 × $0.15/M + 500 × $0.60/M = $0.00015 + $0.00030.
    let cost = price.cost(1_000, 500);

    assert!(
        (cost - 0.00045).abs() < 1e-12,
        "1000 in at $0.15/M and 500 out at $0.60/M is $0.00045, got {cost:?}"
    );
    assert_eq!(
        format!("{cost:.4}"),
        "0.0004",
        "four decimals is the precision money is shown at, and $0.00045 rounds there"
    );
    assert_eq!(
        Price::of(Some(0.0), Some(0.0)).map(|free| free.cost(2_000, 1_000)),
        Some(0.0),
        "a stated zero costs nothing, and it is the provider's claim rather than ours"
    );
    assert_eq!(
        Price::of(Some(0.00000015), None),
        None,
        "half a price cannot say what a turn cost"
    );
}
