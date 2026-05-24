# Step 13: Token Budget

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 8 — Token Budget. This step implements budget enforcement.

### This Step
Implement token counting and binary search per SPEC §10.

## Prerequisites
- Step 12 complete (to_tree function)

## Files to Read Before Starting
- `SPEC.md` §10.1, §10.2, §10.3 — token counting, binary search, no-chat mode
- `src/tokens.rs` — current stub
- `src/budget.rs` — current stub

## Implementation

### Task 1: Implement token counting in src/tokens.rs

```rust
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
```

### Task 2: Implement binary search in src/budget.rs

```rust
//! Binary search for token budget (SPEC §10.2).

use crate::rank::RankedEntry;
use crate::render::{to_tree, TreeCache, TreeContextCache};
use crate::tokens::TokenCounter;
use std::collections::HashSet;

/// Binary search for the largest tree that fits within the token budget.
///
/// Returns the rendered tree string, or None if no valid tree found.
pub fn binary_search_budget(
    ranked_tags: &[RankedEntry],
    max_map_tokens: usize,
    chat_rel_fnames: &HashSet<String>,
    max_line_length: usize,
    token_counter: &TokenCounter,
    tree_cache: &mut TreeCache,
    tree_context_cache: &mut TreeContextCache,
) -> Option<String> {
    let n = ranked_tags.len();
    if n == 0 {
        return None;
    }

    // Step 1: Initialize
    let mut lower_bound = 0usize;
    let mut upper_bound = n;
    let mut best_tree: Option<String> = None;
    let mut best_tree_tokens = 0usize;

    // Initial middle: min(max_map_tokens / 25, N)
    let mut middle = (max_map_tokens / 25).min(n);

    // Step 2: Binary search loop
    while lower_bound <= upper_bound {
        // 2a: Render tree for ranked_tags[..middle]
        let current_tree = to_tree(
            &ranked_tags[..middle],
            chat_rel_fnames,
            max_line_length,
            tree_cache,
            tree_context_cache,
        );

        // 2b: Count tokens
        let num_tokens = token_counter.count(&current_tree);

        // 2c: Compute percentage error
        let pct_err = if max_map_tokens > 0 {
            (num_tokens as f64 - max_map_tokens as f64).abs() / max_map_tokens as f64
        } else {
            1.0
        };

        // 2d: Update best if conditions met
        let is_under_budget = num_tokens <= max_map_tokens;
        let is_better = num_tokens > best_tree_tokens;
        let is_close_enough = pct_err < 0.15;

        if (is_under_budget && is_better) || is_close_enough {
            best_tree = Some(current_tree);
            best_tree_tokens = num_tokens;

            // Early exit if close enough
            if is_close_enough {
                break;
            }
        }

        // 2e: Binary search step
        if num_tokens < max_map_tokens {
            lower_bound = middle + 1;
        } else {
            if middle == 0 {
                break;
            }
            upper_bound = middle - 1;
        }

        middle = (lower_bound + upper_bound) / 2;

        // Prevent infinite loop
        if middle == 0 && lower_bound > upper_bound {
            break;
        }
    }

    best_tree
}

/// Compute effective max_map_tokens for no-chat mode (SPEC §10.3).
///
/// When chat_fnames is empty and max_context_window is set:
/// effective_max = min(max_map_tokens * map_mul_no_files, max_context_window - 4096)
pub fn compute_effective_max(
    max_map_tokens: usize,
    chat_fnames_empty: bool,
    max_context_window: Option<usize>,
    map_mul_no_files: usize,
) -> usize {
    if !chat_fnames_empty {
        return max_map_tokens;
    }

    match max_context_window {
        Some(window) if window > 4096 => {
            let expanded = max_map_tokens.saturating_mul(map_mul_no_files);
            let capped = window.saturating_sub(4096);
            expanded.min(capped)
        }
        _ => max_map_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::Tag;

    fn make_bare(rel: &str) -> RankedEntry {
        RankedEntry::Bare {
            rel_fname: rel.to_string(),
            score: 0.0,
        }
    }

    #[test]
    fn binary_search_empty() {
        let counter = TokenCounter::new();
        let mut tc = TreeCache::new();
        let mut tcc = TreeContextCache::new();

        let result = binary_search_budget(
            &[],
            1000,
            &HashSet::new(),
            100,
            &counter,
            &mut tc,
            &mut tcc,
        );

        assert!(result.is_none());
    }

    #[test]
    fn binary_search_small_budget() {
        let entries: Vec<RankedEntry> = (0..100)
            .map(|i| make_bare(&format!("file{}.rs", i)))
            .collect();

        let counter = TokenCounter::new();
        let mut tc = TreeCache::new();
        let mut tcc = TreeContextCache::new();

        let result = binary_search_budget(
            &entries,
            50, // Very small budget
            &HashSet::new(),
            100,
            &counter,
            &mut tc,
            &mut tcc,
        );

        // Should return something, even if small
        if let Some(tree) = result {
            let tokens = counter.count(&tree);
            // Allow 15% tolerance
            assert!(tokens <= 50 || (tokens as f64 / 50.0) < 1.15);
        }
    }

    #[test]
    fn effective_max_with_chat() {
        // With chat files, no expansion
        let result = compute_effective_max(1024, false, Some(100000), 8);
        assert_eq!(result, 1024);
    }

    #[test]
    fn effective_max_no_chat() {
        // No chat files, expand up to window - 4096
        let result = compute_effective_max(1024, true, Some(100000), 8);
        assert_eq!(result, 1024 * 8); // 8192
    }

    #[test]
    fn effective_max_capped_by_window() {
        // Expansion capped by max_context_window - 4096
        let result = compute_effective_max(10000, true, Some(10000), 8);
        assert_eq!(result, 10000 - 4096); // 5904
    }

    #[test]
    fn effective_max_no_window() {
        // No max_context_window, no expansion
        let result = compute_effective_max(1024, true, None, 8);
        assert_eq!(result, 1024);
    }
}
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test tokens` exits 0
- [ ] `cargo test budget` exits 0
- [ ] TokenCounter uses cl100k_base encoding
- [ ] Token counting uses sampling for texts ≥ 200 chars
- [ ] Binary search uses initial middle = min(max_tokens/25, N)
- [ ] Binary search allows 15% overrun tolerance
- [ ] No-chat mode expands budget per SPEC §10.3 formula

## Reviewer Instructions

You are reviewing Step 13. Verify:

1. Run `cargo test tokens::tests` — all tests must pass
2. Run `cargo test budget::tests` — all tests must pass
3. Check `TokenCounter::count` uses sampling for len >= 200
4. Check sampling uses step = num_lines / 100 (min 1)
5. Verify binary search early exits when pct_err < 0.15
6. Verify `compute_effective_max` formula matches SPEC §10.3
7. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/tokens.rs src/budget.rs
```
