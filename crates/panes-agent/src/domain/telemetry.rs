#[derive(Debug, Clone, PartialEq, Default)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub reasoning: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub cost_usd: Option<f64>,
}

impl TokenUsage {
    pub fn combine_latest(&mut self, update: TokenUsage) {
        if update.input > 0 {
            self.input = update.input;
        }
        if update.output > 0 {
            self.output = update.output;
        }
        if update.reasoning.is_some() {
            self.reasoning = update.reasoning;
        }
        if update.cache_read.is_some() {
            self.cache_read = update.cache_read;
        }
        if update.cache_write.is_some() {
            self.cache_write = update.cache_write;
        }
        self.cost_usd = add_optional_f64(self.cost_usd, update.cost_usd);
    }

    pub fn add_turn(&mut self, usage: &TokenUsage) {
        self.input = self.input.saturating_add(usage.input);
        self.output = self.output.saturating_add(usage.output);
        self.reasoning = add_optional(self.reasoning, usage.reasoning);
        self.cache_read = add_optional(self.cache_read, usage.cache_read);
        self.cache_write = add_optional(self.cache_write, usage.cache_write);
        self.cost_usd = add_optional_f64(self.cost_usd, usage.cost_usd);
    }

    pub fn is_empty(&self) -> bool {
        self.input == 0
            && self.output == 0
            && self.reasoning.unwrap_or(0) == 0
            && self.cache_read.unwrap_or(0) == 0
            && self.cache_write.unwrap_or(0) == 0
            && self.cost_usd.unwrap_or(0.0) == 0.0
    }
}

fn add_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn add_optional_f64(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(round_cost_usd(left + right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn round_cost_usd(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}
