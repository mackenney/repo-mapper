//! Personalization vector computation (SPEC §7.1).

use crate::path::path_components;
use std::collections::{HashMap, HashSet};

/// Compute the personalization vector for PageRank.
///
/// Per SPEC §7.1:
/// - Base = 100.0 / N
/// - Chat files: add base
/// - Mentioned files: max(current, base)
/// - Path components in mentioned_idents: add base
/// - Anchor files (SPEC §7.1 step 5): add base * anchor_weight_multiplier
pub fn compute_personalization(
    total_files: usize,
    chat_rel_fnames: &HashSet<String>,
    rel_fnames: &[String],
    mentioned_fnames: &HashSet<String>,
    mentioned_idents: &HashSet<String>,
    anchor_rel_fnames: &HashSet<String>,
    anchor_weight_multiplier: f64,
) -> HashMap<String, f64> {
    if total_files == 0 {
        return HashMap::new();
    }

    let personalize = 100.0 / total_files as f64;
    let mut result: HashMap<String, f64> = HashMap::new();

    for rel_fname in rel_fnames {
        let mut current_pers = 0.0;

        // Step 2: Chat files add personalize
        if chat_rel_fnames.contains(rel_fname) {
            current_pers += personalize;
        }

        // Step 3: Mentioned files take max (avoids double-counting)
        if mentioned_fnames.contains(rel_fname) {
            current_pers = current_pers.max(personalize);
        }

        // Step 4: Path components in mentioned_idents (SPEC §7.1 step 4).
        // Checks: directory parts, basename WITH extension, basename WITHOUT extension.
        // path_components() in src/path.rs MUST return all Path::components() entries
        // (directory parts + filename) as &str slices, INCLUDING the full basename.
        // The stem check below adds basename-without-extension as a fourth check.
        // All checks are independent; at most one +personalize is added total for step 4.
        let mut path_matched = false;
        for component in path_components(rel_fname) {
            // covers: directory parts AND basename with extension
            if mentioned_idents.contains(component) {
                path_matched = true;
                break;
            }
        }
        // Also check basename without extension (file stem)
        if !path_matched {
            if let Some(stem) = std::path::Path::new(rel_fname)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                if mentioned_idents.contains(stem) {
                    path_matched = true;
                }
            }
        }
        // NOTE: personalize added once regardless of whether steps 2/3 already fired.
        // Steps 2, 3, and 4 are independent per SPEC §7.1.
        if path_matched {
            current_pers += personalize;
        }

        // Step 5: Anchor files (SPEC §7.1 step 5) — additive, independent of prior steps.
        if anchor_rel_fnames.contains(rel_fname) {
            current_pers += personalize * anchor_weight_multiplier;
        }

        // Only include files with positive personalization
        if current_pers > 0.0 {
            result.insert(rel_fname.clone(), current_pers);
        }
    }

    // Anchor files may not be in rel_fnames (e.g. resolved from anchor_idents pointing to
    // a file outside other_fnames). Add them directly so they always appear in the vector.
    for anchor in anchor_rel_fnames {
        if !result.contains_key(anchor) {
            result.insert(anchor.clone(), personalize * anchor_weight_multiplier);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personalization_empty() {
        let result = compute_personalization(
            0,
            &HashSet::new(),
            &[],
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            10.0,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn personalization_chat_files() {
        let mut chat = HashSet::new();
        chat.insert("main.rs".to_string());
        let files = vec!["main.rs".to_string(), "lib.rs".to_string()];

        let result = compute_personalization(
            2,
            &chat,
            &files,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            10.0,
        );

        assert!(result.contains_key("main.rs"));
        assert!(!result.contains_key("lib.rs")); // Not a chat file
        assert!((result["main.rs"] - 50.0).abs() < 0.001); // 100/2 = 50
    }

    #[test]
    fn personalization_mentioned_files() {
        let mut mentioned = HashSet::new();
        mentioned.insert("lib.rs".to_string());
        let files = vec!["main.rs".to_string(), "lib.rs".to_string()];

        let result = compute_personalization(
            2,
            &HashSet::new(),
            &files,
            &mentioned,
            &HashSet::new(),
            &HashSet::new(),
            10.0,
        );

        assert!(result.contains_key("lib.rs"));
        assert!((result["lib.rs"] - 50.0).abs() < 0.001);
    }

    #[test]
    fn personalization_mentioned_idents() {
        let mut idents = HashSet::new();
        idents.insert("utils".to_string());
        let files = vec!["src/utils/mod.rs".to_string()];

        let result = compute_personalization(
            1,
            &HashSet::new(),
            &files,
            &HashSet::new(),
            &idents,
            &HashSet::new(),
            10.0,
        );

        // "utils" is in the path, so file gets personalization
        assert!(result.contains_key("src/utils/mod.rs"));
    }

    #[test]
    fn personalization_no_double_count() {
        let mut chat = HashSet::new();
        chat.insert("main.rs".to_string());
        let mut mentioned = HashSet::new();
        mentioned.insert("main.rs".to_string());
        let files = vec!["main.rs".to_string()];

        let result = compute_personalization(
            1,
            &chat,
            &files,
            &mentioned,
            &HashSet::new(),
            &HashSet::new(),
            10.0,
        );

        // Should be max(100, 100) = 100, not 200
        assert!((result["main.rs"] - 100.0).abs() < 0.001);
    }

    #[test]
    fn personalization_anchor_files() {
        let mut anchors = HashSet::new();
        anchors.insert("entry.rs".to_string());
        let files = vec!["entry.rs".to_string(), "lib.rs".to_string()];

        let result = compute_personalization(
            2,
            &HashSet::new(),
            &files,
            &HashSet::new(),
            &HashSet::new(),
            &anchors,
            10.0,
        );

        // entry.rs gets 10x boost: 10 * (100/2) = 500
        assert!(result.contains_key("entry.rs"));
        assert!((result["entry.rs"] - 500.0).abs() < 0.001);
        assert!(!result.contains_key("lib.rs"));
    }

    #[test]
    fn personalization_anchor_not_in_rel_fnames() {
        // Anchor resolved from ident may not be in other_fnames
        let mut anchors = HashSet::new();
        anchors.insert("external.rs".to_string());
        let files = vec!["main.rs".to_string()];

        let result = compute_personalization(
            1,
            &HashSet::new(),
            &files,
            &HashSet::new(),
            &HashSet::new(),
            &anchors,
            10.0,
        );

        // external.rs gets inserted directly
        assert!(result.contains_key("external.rs"));
    }
}
