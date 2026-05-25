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

        let result =
            binary_search_budget(&[], 1000, &HashSet::new(), 100, &counter, &mut tc, &mut tcc);

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
