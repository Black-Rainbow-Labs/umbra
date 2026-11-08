//! Dollar cost of a model call. Prices are dollars per million tokens, from Anthropic's published
//! first party rates (checked 2026-11-08). A provider's config can set its own prices, and any
//! model not listed costs nothing, which is right for local models.

use crate::models::config::ProviderConfig;
use crate::models::model_response::Usage;

/// Input, output, cache read and cache write prices for one model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// Builds a price from input and output rates, with cache reads at 10% of input and cache
/// writes at 125% of input.
/// @param input - dollars per million input tokens
/// @param output - dollars per million output tokens
const fn standard(input: f64, output: f64) -> Price {
    Price { input, output, cache_read: input * 0.1, cache_write: input * 1.25 }
}

/// Model id prefixes and their prices. The first prefix that matches wins.
const PRICE_TABLE: &[(&str, Price)] = &[
    ("claude-fable-5-1", Price { input: 10.0, output: 50.0, cache_read: 0.25, cache_write: 12.5 }),
    ("claude-mythos-5-1", Price { input: 10.0, output: 50.0, cache_read: 0.25, cache_write: 12.5 }),
    ("claude-fable-5", standard(10.0, 50.0)),
    ("claude-opus-5-5", Price { input: 4.0, output: 20.0, cache_read: 0.20, cache_write: 5.0 }),
    ("claude-opus-5", standard(5.0, 25.0)),
    ("claude-opus-4", standard(5.0, 25.0)),
    ("claude-sonnet-5-5", Price { input: 2.0, output: 10.0, cache_read: 0.20, cache_write: 2.5 }),
    ("claude-sonnet-5", standard(2.0, 10.0)),
    ("claude-sonnet-4", standard(3.0, 15.0)),
    ("claude-haiku-5-5", standard(0.10, 0.50)),
    ("claude-haiku-4-5", standard(1.0, 5.0)),
];

/// Finds the price for a model, preferring prices set in the provider config.
/// @param model - the model id
/// @param config - the provider settings
pub fn price_for(model: &str, config: &ProviderConfig) -> Option<Price> {
    if let (Some(input), Some(output)) = (config.input_cost_per_mtok, config.output_cost_per_mtok) {
        return Some(standard(input, output));
    }
    PRICE_TABLE.iter().find(|(prefix, _)| model.starts_with(prefix)).map(|(_, price)| *price)
}

/// Computes the dollar cost of one call.
/// @param usage - the token counts the provider reported
/// @param price - the model's price, or None for a free model
pub fn cost_of(usage: &Usage, price: Option<Price>) -> f64 {
    let Some(p) = price else { return 0.0 };
    let per = 1_000_000.0;
    usage.input_tokens as f64 * p.input / per + usage.output_tokens as f64 * p.output / per + usage.cache_read_tokens as f64 * p.cache_read / per + usage.cache_write_tokens as f64 * p.cache_write / per
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_haiku_and_leaves_local_models_free() {
        let config = ProviderConfig::default();
        let usage = Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, cache_read_tokens: 0, cache_write_tokens: 0 };
        assert!((cost_of(&usage, price_for("claude-haiku-5-5", &config)) - 0.60).abs() < 1e-9);
        assert_eq!(cost_of(&usage, price_for("qwen3.8-27b", &config)), 0.0);
    }
}
