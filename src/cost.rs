use crate::api::types::Usage;

/// Tracks token usage and estimated cost across a session.
pub struct CostTracker {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub turns: usize,
    model: String,
}

impl CostTracker {
    pub fn new(model: &str) -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_read_tokens: 0,
            total_cache_creation_tokens: 0,
            turns: 0,
            model: model.to_string(),
        }
    }

    /// Add usage from a streaming event.
    /// Anthropic API reports usage across two events:
    /// - message_start.message.usage: input_tokens, cache tokens (output_tokens = 0)
    /// - message_delta.usage: output_tokens (input_tokens = 0)
    /// Fields default to 0 via serde, so adding both is safe without double-counting.
    pub fn add_usage(&mut self, usage: &Usage) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
        self.total_cache_read_tokens += usage.cache_read_input_tokens;
        self.total_cache_creation_tokens += usage.cache_creation_input_tokens;
    }

    pub fn add_turn(&mut self) {
        self.turns += 1;
    }

    /// Total tokens including cache tokens.
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens
            + self.total_output_tokens
            + self.total_cache_read_tokens
            + self.total_cache_creation_tokens
    }

    /// Estimated cost in USD based on model pricing.
    /// Pricing as of 2025-03: https://docs.anthropic.com/en/docs/about-claude/pricing
    pub fn estimated_cost(&self) -> f64 {
        let pricing = model_pricing(&self.model);
        let input_cost = self.total_input_tokens as f64 * pricing.input / 1_000_000.0;
        let output_cost = self.total_output_tokens as f64 * pricing.output / 1_000_000.0;
        let cache_read_cost =
            self.total_cache_read_tokens as f64 * pricing.cache_read / 1_000_000.0;
        let cache_creation_cost =
            self.total_cache_creation_tokens as f64 * pricing.cache_write / 1_000_000.0;
        input_cost + output_cost + cache_read_cost + cache_creation_cost
    }

    /// Format a summary line for display.
    pub fn summary(&self) -> String {
        let total = self.total_tokens();
        let cost = self.estimated_cost();
        let turn_label = if self.turns == 1 { "turn" } else { "turns" };
        format!(
            "{} {} · {} tokens · ${:.4}",
            self.turns,
            turn_label,
            format_tokens(total),
            cost
        )
    }

    /// Format detailed cost breakdown for /cost command.
    pub fn detail(&self) -> String {
        let cost = self.estimated_cost();
        let mut lines = Vec::new();
        lines.push("  \x1b[1mSession Cost\x1b[0m".to_string());
        lines.push(String::new());
        lines.push(format!(
            "  Input tokens:    {:>8}",
            format_tokens(self.total_input_tokens)
        ));
        lines.push(format!(
            "  Output tokens:   {:>8}",
            format_tokens(self.total_output_tokens)
        ));
        if self.total_cache_read_tokens > 0 {
            lines.push(format!(
                "  Cache read:      {:>8}",
                format_tokens(self.total_cache_read_tokens)
            ));
        }
        if self.total_cache_creation_tokens > 0 {
            lines.push(format!(
                "  Cache created:   {:>8}",
                format_tokens(self.total_cache_creation_tokens)
            ));
        }
        lines.push("  \x1b[2m─────────────────────\x1b[0m".to_string());
        lines.push(format!(
            "  Total:           {:>8}",
            format_tokens(self.total_tokens())
        ));
        lines.push(format!("  Turns:           {:>8}", self.turns));
        lines.push(format!(
            "  Est. cost:       \x1b[1m${:.4}\x1b[0m",
            cost
        ));
        lines.push(format!("  Model:           {}", self.model));
        lines.join("\n")
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

/// Per-model pricing in USD per million tokens.
/// Source: https://docs.anthropic.com/en/docs/about-claude/pricing (as of 2025-03)
struct ModelPricing {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

fn model_pricing(model: &str) -> ModelPricing {
    if model.contains("opus") {
        ModelPricing {
            input: 15.0,
            output: 75.0,
            cache_read: 1.50,
            cache_write: 18.75,
        }
    } else if model.contains("sonnet") {
        ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input: 0.80,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        }
    } else {
        // Unknown model — use Sonnet pricing as default
        ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_haiku_exact() {
        let mut tracker = CostTracker::new("claude-haiku-4-5-20251001");
        tracker.add_usage(&Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });
        // Haiku: $0.80/M input + $4.0/M output = $4.80
        let cost = tracker.estimated_cost();
        assert!((cost - 4.80).abs() < 0.001, "Expected $4.80, got ${}", cost);
    }

    #[test]
    fn test_cost_with_cache() {
        let mut tracker = CostTracker::new("claude-sonnet-4-20250514");
        tracker.add_usage(&Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_input_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
        });
        // Sonnet: cache_read $0.30/M + cache_write $3.75/M = $4.05
        let cost = tracker.estimated_cost();
        assert!(
            (cost - 4.05).abs() < 0.001,
            "Expected $4.05, got ${}",
            cost
        );
    }

    #[test]
    fn test_total_tokens_includes_cache() {
        let mut tracker = CostTracker::new("claude-haiku-4-5-20251001");
        tracker.add_usage(&Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 200,
            cache_creation_input_tokens: 30,
        });
        assert_eq!(tracker.total_tokens(), 380);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_summary_singular_turn() {
        let mut tracker = CostTracker::new("claude-haiku-4-5-20251001");
        tracker.add_turn();
        let summary = tracker.summary();
        assert!(summary.contains("1 turn "));
    }

    #[test]
    fn test_summary_plural_turns() {
        let mut tracker = CostTracker::new("claude-haiku-4-5-20251001");
        tracker.add_turn();
        tracker.add_turn();
        let summary = tracker.summary();
        assert!(summary.contains("2 turns"));
    }
}
