//! Edge weight calculation (SPEC §6.4).

use std::collections::HashSet;

/// Compute the edge weight for a reference relationship.
///
/// Per SPEC §6.4, applies multipliers based on:
/// - Identifier in mentioned_idents: ×10
/// - Identifier length ≥8 AND (snake_case OR kebab-case OR camelCase): ×10
/// - Identifier starts with "_": ×0.1
/// - More than 5 distinct definers: ×0.1
/// - Referencer is a chat file: ×50
/// - Final weight = use_mul × sqrt(num_refs)
pub fn compute_edge_weight(
    ident: &str,
    referencer: &str,
    num_refs: usize,
    mentioned_idents: &HashSet<String>,
    chat_rel_fnames: &HashSet<String>,
    num_definers: usize,
) -> f64 {
    let mut mul = 1.0;

    // Condition 1: in mentioned_idents
    if mentioned_idents.contains(ident) {
        mul *= 10.0;
    }

    // Condition 2: length ≥8 AND meaningful identifier pattern (skip if starts with "_")
    if ident.len() >= 8 && !ident.starts_with('_') && is_meaningful_ident(ident) {
        mul *= 10.0;
    }

    // Condition 3: starts with "_"
    if ident.starts_with('_') {
        mul *= 0.1;
    }

    // Condition 4: more than 5 distinct definers
    if num_definers > 5 {
        mul *= 0.1;
    }

    // Condition 5: referencer is a chat file
    let use_mul = if chat_rel_fnames.contains(referencer) {
        mul * 50.0
    } else {
        mul
    };

    // Final weight
    use_mul * (num_refs as f64).sqrt()
}

/// Check if an identifier is "meaningful" per SPEC §6.4.
///
/// Returns true if the identifier contains:
/// - `_` with at least one alphabetic character (snake_case), OR
/// - `-` with at least one alphabetic character (kebab-case), OR
/// - Both upper and lower case letters (camelCase/PascalCase)
pub fn is_meaningful_ident(s: &str) -> bool {
    let has_alpha = s.chars().any(|c| c.is_alphabetic());
    if !has_alpha {
        return false;
    }

    // Check for snake_case
    if s.contains('_') {
        return true;
    }

    // Check for kebab-case
    if s.contains('-') {
        return true;
    }

    // Check for mixed case (camelCase/PascalCase)
    let has_upper = s.chars().any(|c| c.is_uppercase());
    let has_lower = s.chars().any(|c| c.is_lowercase());
    if has_upper && has_lower {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_meaningful_snake_case() {
        assert!(is_meaningful_ident("my_function"));
        assert!(is_meaningful_ident("CONST_VALUE"));
    }

    #[test]
    fn is_meaningful_kebab_case() {
        assert!(is_meaningful_ident("my-component"));
    }

    #[test]
    fn is_meaningful_camel_case() {
        assert!(is_meaningful_ident("myFunction"));
        assert!(is_meaningful_ident("MyClass"));
    }

    #[test]
    fn is_meaningful_short_or_simple() {
        assert!(!is_meaningful_ident("foo")); // No special pattern
        assert!(!is_meaningful_ident("FOO")); // All caps, no mixed case
        assert!(!is_meaningful_ident("123")); // No alpha
    }

    #[test]
    fn weight_basic() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("foo", "file.rs", 1, &mentioned, &chat_files, 1);
        assert!((weight - 1.0).abs() < 0.001);
    }

    #[test]
    fn weight_mentioned_ident() {
        let mut mentioned = HashSet::new();
        mentioned.insert("my_function".to_string());
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("my_function", "file.rs", 1, &mentioned, &chat_files, 1);
        // 10.0 (mentioned) * 10.0 (meaningful + len≥8) * sqrt(1) = 100.0
        assert!((weight - 100.0).abs() < 0.001);
    }

    #[test]
    fn weight_chat_file() {
        let mentioned = HashSet::new();
        let mut chat_files = HashSet::new();
        chat_files.insert("main.rs".to_string());
        let weight = compute_edge_weight("foo", "main.rs", 4, &mentioned, &chat_files, 1);
        // 1.0 * 50.0 (chat file) * sqrt(4) = 100.0
        assert!((weight - 100.0).abs() < 0.001);
    }

    #[test]
    fn weight_underscore_prefix() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("_private", "file.rs", 1, &mentioned, &chat_files, 1);
        // 0.1 (underscore) * sqrt(1) = 0.1
        assert!((weight - 0.1).abs() < 0.001);
    }

    #[test]
    fn weight_many_definers() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("foo", "file.rs", 1, &mentioned, &chat_files, 10);
        // 0.1 (>5 definers) * sqrt(1) = 0.1
        assert!((weight - 0.1).abs() < 0.001);
    }

    #[test]
    fn weight_multiple_refs() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("foo", "file.rs", 9, &mentioned, &chat_files, 1);
        // 1.0 * sqrt(9) = 3.0
        assert!((weight - 3.0).abs() < 0.001);
    }
}
