//! What a listed model charges: the dollars the provider states per token.
//!
//! The numbers are the provider's, never a table of ours. OpenRouter and its imitators put them on
//! the listing as `pricing: {"prompt": "0.00000015", "completion": "0.0000006"}`, strings in
//! dollars per token, and a gateway that states none leaves the price unknown — which is not zero,
//! because zero is the claim that a model is free.

use serde::Deserialize;

/// Dollars per token, for the two kinds of token a turn reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    /// Dollars per input token.
    pub prompt: f64,
    /// Dollars per output token.
    pub completion: f64,
}

impl Price {
    /// The price when the provider stated both halves.
    ///
    /// Half a price is no price: a completion with no prompt rate cannot be turned into what a
    /// turn cost, and filling the gap with zero would claim the missing half is free.
    pub fn of(prompt: Option<f64>, completion: Option<f64>) -> Option<Price> {
        Some(Price {
            prompt: prompt?,
            completion: completion?,
        })
    }

    /// The dollars a turn of `input` and `output` tokens cost at this price.
    pub fn cost(&self, input: u32, output: u32) -> f64 {
        f64::from(input) * self.prompt + f64::from(output) * self.completion
    }
}

/// The `pricing` object a listing may carry.
#[derive(Debug, Deserialize)]
pub(crate) struct WirePricing {
    #[serde(default)]
    prompt: Option<serde_json::Value>,
    #[serde(default)]
    completion: Option<serde_json::Value>,
}

impl WirePricing {
    /// The two rates, when both are stated as finite non-negative numbers.
    pub(crate) fn values(&self) -> Option<(f64, f64)> {
        Some((
            money(self.prompt.as_ref()?)?,
            money(self.completion.as_ref()?)?,
        ))
    }
}

/// One stated rate, read as a string or a number, refusing anything that is not money.
///
/// `"0"` and `0` are both money — a provider saying its model is free. A negative, infinite or
/// unparseable value is not, and leaves the whole price unknown rather than half-believed.
fn money(value: &serde_json::Value) -> Option<f64> {
    let dollars = match value {
        serde_json::Value::String(words) => words.trim().parse::<f64>().ok()?,
        serde_json::Value::Number(number) => number.as_f64()?,
        _ => return None,
    };
    (dollars.is_finite() && dollars >= 0.0).then_some(dollars)
}
