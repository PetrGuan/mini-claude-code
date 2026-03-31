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

    pub fn add_usage(&mut self, usage: &Usage) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
        self.total_cache_read_tokens += usage.cache_read_input_tokens;
        self.total_cache_creation_tokens += usage.cache_creation_input_tokens;
    }

    pub fn add_turn(&mut self) {
        self.turns += 1;
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Estimated cost in USD based on model pricing.
    pub fn estimated_cost(&self) -> f64 {
        let (input_per_m, output_per_m) = model_pricing(&self.model);
        let input_cost = self.total_input_tokens as f64 * input_per_m / 1_000_000.0;
        let output_cost = self.total_output_tokens as f64 * output_per_m / 1_000_000.0;
        // Cache reads are cheaper (typically 10% of input price)
        let cache_read_cost =
            self.total_cache_read_tokens as f64 * (input_per_m * 0.1) / 1_000_000.0;
        input_cost + output_cost + cache_read_cost
    }

    /// Format a summary line for display.
    pub fn summary(&self) -> String {
        let total = self.total_tokens();
        let cost = self.estimated_cost();
        let tokens_str = format_tokens(total);
        format!(
            "{} turns · {} tokens · ${:.4}",
            self.turns, tokens_str, cost
        )
    }

    /// Format detailed cost breakdown for /cost command.
    pub fn detail(&self) -> String {
        let cost = self.estimated_cost();
        let mut lines = Vec::new();
        lines.push(format!("  \x1b[1mSession Cost\x1b[0m"));
        lines.push(format!(""));
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
        lines.push(format!(
            "  \x1b[2m─────────────────────\x1b[0m"
        ));
        lines.push(format!(
            "  Total:           {:>8}",
            format_tokens(self.total_tokens())
        ));
        lines.push(format!("  Turns:           {:>8}", self.turns));
        lines.push(format!("  Est. cost:       \x1b[1m${:.4}\x1b[0m", cost));
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

/// Returns (input_price_per_million, output_price_per_million) in USD.
fn model_pricing(model: &str) -> (f64, f64) {
    if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else if model.contains("haiku") {
        (0.80, 4.0)
    } else {
        // Unknown model — use Sonnet pricing as default
        (3.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker_basic() {
        let mut tracker = CostTracker::new("claude-haiku-4-5-20251001");
        tracker.add_usage(&Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });
        tracker.add_turn();
        assert_eq!(tracker.total_tokens(), 1500);
        assert_eq!(tracker.turns, 1);
        assert!(tracker.estimated_cost() > 0.0);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_model_pricing() {
        let (i, o) = model_pricing("claude-haiku-4-5-20251001");
        assert_eq!(i, 0.80);
        assert_eq!(o, 4.0);
    }

    #[test]
    fn test_summary_format() {
        let mut tracker = CostTracker::new("claude-haiku-4-5-20251001");
        tracker.add_usage(&Usage {
            input_tokens: 10000,
            output_tokens: 5000,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });
        tracker.add_turn();
        let summary = tracker.summary();
        assert!(summary.contains("1 turns"));
        assert!(summary.contains("15.0K"));
        assert!(summary.contains("$"));
    }
}
