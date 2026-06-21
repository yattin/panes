use crate::domain::{conversation::AgentMessage, telemetry::TokenUsage};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TokenBudget {
    pub max_turns: Option<u32>,
    pub max_input_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub max_total_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BudgetUsage {
    pub turn_count: u32,
    pub tokens: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    Continue,
    Stop(String),
}

impl TokenBudget {
    pub fn before_model_call(
        &self,
        messages: &[AgentMessage],
        system_prompt: &str,
    ) -> BudgetDecision {
        let estimated_input = estimate_tokens(system_prompt)
            .saturating_add(messages.iter().map(estimate_message_tokens).sum::<u64>());

        if exceeds(self.max_input_tokens, estimated_input) {
            return BudgetDecision::Stop(format!(
                "input token budget exceeded: estimated {estimated_input} tokens"
            ));
        }
        if exceeds(self.max_total_tokens, estimated_input) {
            return BudgetDecision::Stop(format!(
                "total token budget exceeded before model call: estimated {estimated_input} tokens"
            ));
        }
        BudgetDecision::Continue
    }

    pub fn after_model_call(&self, usage: &BudgetUsage) -> BudgetDecision {
        if exceeds(self.max_turns.map(u64::from), u64::from(usage.turn_count)) {
            return BudgetDecision::Stop(format!(
                "turn budget exceeded: {} turns",
                usage.turn_count
            ));
        }
        if exceeds(self.max_input_tokens, usage.tokens.input) {
            return BudgetDecision::Stop(format!(
                "input token budget exceeded: {} input tokens",
                usage.tokens.input
            ));
        }
        if exceeds(self.max_output_tokens, usage.tokens.output) {
            return BudgetDecision::Stop(format!(
                "output token budget exceeded: {} output tokens",
                usage.tokens.output
            ));
        }
        let total = usage
            .tokens
            .input
            .saturating_add(usage.tokens.output)
            .saturating_add(usage.tokens.reasoning.unwrap_or(0));
        if exceeds(self.max_total_tokens, total) {
            return BudgetDecision::Stop(format!("total token budget exceeded: {total} tokens"));
        }
        if let (Some(limit), Some(cost)) = (self.max_cost_usd, usage.tokens.cost_usd) {
            if cost > limit {
                return BudgetDecision::Stop(format!("cost budget exceeded: ${cost:.6}"));
            }
        }
        BudgetDecision::Continue
    }
}

pub fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.saturating_add(3) / 4
}

fn estimate_message_tokens(message: &AgentMessage) -> u64 {
    message
        .content
        .iter()
        .map(|content| estimate_tokens(&format!("{content:?}")))
        .sum()
}

fn exceeds(limit: Option<u64>, value: u64) -> bool {
    limit.map(|limit| value > limit).unwrap_or(false)
}
