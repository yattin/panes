use crate::domain::telemetry::TokenUsage;

const LONG_CONTEXT_THRESHOLD_TOKENS: u64 = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LongContextPricing {
    StandardAcrossFullWindow,
    StandardTier,
}

#[derive(Debug, Clone, Copy)]
struct AnthropicPricing {
    input_per_mtok: f64,
    output_per_mtok: f64,
    cache_write_per_mtok: f64,
    cache_read_per_mtok: f64,
    long_context: LongContextPricing,
}

pub(super) fn estimate_anthropic_usage_cost_usd(model: &str, usage: &TokenUsage) -> Option<f64> {
    let pricing = pricing_for_model(model)?;
    let input_per_mtok = match pricing.long_context {
        LongContextPricing::StandardAcrossFullWindow
            if usage.input > LONG_CONTEXT_THRESHOLD_TOKENS =>
        {
            pricing.input_per_mtok
        }
        _ => pricing.input_per_mtok,
    };
    let per_mtok = 1_000_000.0;
    let cost = usage.input as f64 * input_per_mtok / per_mtok
        + usage.output as f64 * pricing.output_per_mtok / per_mtok
        + usage.cache_write.unwrap_or(0) as f64 * pricing.cache_write_per_mtok / per_mtok
        + usage.cache_read.unwrap_or(0) as f64 * pricing.cache_read_per_mtok / per_mtok;

    (cost > 0.0).then_some(round_cost_usd(cost))
}

fn pricing_for_model(model: &str) -> Option<AnthropicPricing> {
    let normalized = model.to_ascii_lowercase();
    if normalized.contains("fable") || normalized.contains("mythos") {
        return Some(pricing_from_input_output(
            10.0,
            50.0,
            LongContextPricing::StandardAcrossFullWindow,
        ));
    }
    if normalized.contains("haiku-3-5") || normalized.contains("haiku-3.5") {
        return Some(pricing_from_input_output(
            0.8,
            4.0,
            LongContextPricing::StandardTier,
        ));
    }
    if normalized.contains("haiku") {
        return Some(pricing_from_input_output(
            1.0,
            5.0,
            LongContextPricing::StandardTier,
        ));
    }
    if normalized.contains("sonnet") {
        return Some(pricing_from_input_output(
            3.0,
            15.0,
            long_context_pricing(&normalized),
        ));
    }
    if normalized.contains("opus-4-1")
        || normalized.contains("opus-4-202")
        || normalized.contains("opus-4.1")
    {
        return Some(pricing_from_input_output(
            15.0,
            75.0,
            LongContextPricing::StandardTier,
        ));
    }
    if normalized.contains("opus") {
        return Some(pricing_from_input_output(
            5.0,
            25.0,
            long_context_pricing(&normalized),
        ));
    }
    None
}

fn long_context_pricing(normalized_model: &str) -> LongContextPricing {
    if normalized_model.contains("4-6")
        || normalized_model.contains("4-7")
        || normalized_model.contains("4-8")
        || normalized_model.contains("4.6")
        || normalized_model.contains("4.7")
        || normalized_model.contains("4.8")
    {
        LongContextPricing::StandardAcrossFullWindow
    } else {
        LongContextPricing::StandardTier
    }
}

fn pricing_from_input_output(
    input_per_mtok: f64,
    output_per_mtok: f64,
    long_context: LongContextPricing,
) -> AnthropicPricing {
    AnthropicPricing {
        input_per_mtok,
        output_per_mtok,
        cache_write_per_mtok: input_per_mtok * 1.25,
        cache_read_per_mtok: input_per_mtok * 0.1,
        long_context,
    }
}

fn round_cost_usd(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_cost_is_model_tier_aware() {
        let sonnet = estimate_anthropic_usage_cost_usd(
            "claude-sonnet-4-6",
            &TokenUsage {
                input: 1_000_000,
                output: 1_000_000,
                reasoning: None,
                cache_read: Some(1_000_000),
                cache_write: Some(1_000_000),
                cost_usd: None,
            },
        )
        .expect("sonnet pricing should be known");
        assert_close(sonnet, 22.05);

        let haiku = estimate_anthropic_usage_cost_usd(
            "claude-haiku-4-5-20251001",
            &TokenUsage {
                input: 1_000_000,
                output: 1_000_000,
                reasoning: None,
                cache_read: None,
                cache_write: None,
                cost_usd: None,
            },
        )
        .expect("haiku pricing should be known");
        assert_close(haiku, 6.0);

        assert!(estimate_anthropic_usage_cost_usd(
            "custom-model",
            &TokenUsage {
                input: 1_000_000,
                output: 0,
                reasoning: None,
                cache_read: None,
                cache_write: None,
                cost_usd: None,
            },
        )
        .is_none());
    }

    #[test]
    fn long_context_models_keep_standard_pricing_across_full_window() {
        let pricing = pricing_for_model("claude-sonnet-4-6").expect("sonnet 4.6 should price");
        assert_eq!(
            pricing.long_context,
            LongContextPricing::StandardAcrossFullWindow
        );

        let cost = estimate_anthropic_usage_cost_usd(
            "claude-sonnet-4-6",
            &TokenUsage {
                input: LONG_CONTEXT_THRESHOLD_TOKENS + 700_000,
                output: 100_000,
                reasoning: None,
                cache_read: None,
                cache_write: None,
                cost_usd: None,
            },
        )
        .expect("long context cost should be estimated");

        assert_close(cost, 4.2);
    }

    #[test]
    fn legacy_opus_4_1_uses_legacy_pricing() {
        let cost = estimate_anthropic_usage_cost_usd(
            "claude-opus-4-1",
            &TokenUsage {
                input: 1_000_000,
                output: 1_000_000,
                reasoning: None,
                cache_read: None,
                cache_write: None,
                cost_usd: None,
            },
        )
        .expect("legacy opus pricing should be known");

        assert_close(cost, 90.0);
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.0000001,
            "expected {actual} to be close to {expected}"
        );
    }
}
