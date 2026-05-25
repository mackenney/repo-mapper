//! Token counting with tiktoken (SPEC §10.1).

use tiktoken_rs::cl100k_base;

/// Token counter using cl100k_base encoding.
pub struct TokenCounter {
    bpe: tiktoken_rs::CoreBPE,
}

impl TokenCounter {
    /// Create a new token counter.
    pub fn new() -> Self {
        let bpe = cl100k_base().expect("Failed to load cl100k_base tokenizer");
        Self { bpe }
    }

    /// Count tokens in text (SPEC §10.1).
    ///
    /// For texts < 200 chars: direct count.
    /// For longer texts: sample-based estimation.
    pub fn count(&self, text: &str) -> usize {
        if text.len() < 200 {
            return self.bpe.encode_ordinary(text).len();
        }

        // Sampling for longer texts
        let lines: Vec<&str> = text.lines().collect();
        let num_lines = lines.len();
        if num_lines == 0 {
            return 0;
        }

        // Step = num_lines / 100, minimum 1
        let step = (num_lines / 100).max(1);

        // Sample every step-th line
        let mut sample_text = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i % step == 0 {
                sample_text.push_str(line);
                sample_text.push('\n');
            }
        }

        if sample_text.is_empty() {
            return 0;
        }

        // Count tokens in sample
        let sample_tokens = self.bpe.encode_ordinary(&sample_text).len();

        // Estimate: (sample_tokens / sample_len) * total_len
        let estimate = (sample_tokens as f64 / sample_text.len() as f64) * text.len() as f64;
        estimate.round() as usize
    }

    /// Direct token count without sampling (for testing).
    pub fn count_exact(&self, text: &str) -> usize {
        self.bpe.encode_ordinary(text).len()
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_short_text() {
        let counter = TokenCounter::new();
        let text = "Hello, world!";
        let count = counter.count(text);
        // Short text should be counted exactly
        let exact = counter.count_exact(text);
        assert_eq!(count, exact);
    }

    #[test]
    fn count_long_text() {
        let counter = TokenCounter::new();
        // Create text with 300 lines
        let text: String = (0..300)
            .map(|i| format!("This is line number {}\n", i))
            .collect();

        let estimate = counter.count(&text);
        let exact = counter.count_exact(&text);

        // Estimate should be within reasonable range of exact
        let ratio = estimate as f64 / exact as f64;
        assert!(ratio > 0.7 && ratio < 1.5, "ratio: {}", ratio);
    }

    #[test]
    fn count_empty() {
        let counter = TokenCounter::new();
        assert_eq!(counter.count(""), 0);
    }
}
