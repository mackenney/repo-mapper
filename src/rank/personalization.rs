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
/// - Anchor contributions (SPEC §7.1 step 5): add pre-computed per-file weight.
///   Weights already incorporate the multiplier and any ambiguity division; see §7.1a.
pub fn compute_personalization(
    total_files: usize,
    chat_rel_fnames: &HashSet<String>,
    rel_fnames: &[String],
    mentioned_fnames: &HashSet<String>,
    mentioned_idents: &HashSet<String>,
    anchor_contributions: &HashMap<String, f64>,
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
            if mentioned_idents.contains(component) {
                path_matched = true;
                break;
            }
        }
        if !path_matched
            && let Some(stem) = std::path::Path::new(rel_fname)
                .file_stem()
                .and_then(|s| s.to_str())
            && mentioned_idents.contains(stem)
        {
            path_matched = true;
        }
        if path_matched {
            current_pers += personalize;
        }

        // Step 5: Anchor contributions (SPEC §7.1 step 5).
        // Pre-computed in compute_map; already incorporates multiplier and ambiguity division.
        if let Some(&contrib) = anchor_contributions.get(rel_fname) {
            current_pers += contrib;
        }

        if current_pers > 0.0 {
            result.insert(rel_fname.clone(), current_pers);
        }
    }

    // Anchor files resolved from idents may not be in rel_fnames (e.g. not in other_fnames).
    // Insert them directly so they always participate in the restart vector.
    for (anchor, &contrib) in anchor_contributions {
        if !result.contains_key(anchor) {
            result.insert(anchor.clone(), contrib);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_anchors() -> HashMap<String, f64> {
        HashMap::new()
    }

    #[test]
    fn personalization_empty() {
        let result = compute_personalization(
            0,
            &HashSet::new(),
            &[],
            &HashSet::new(),
            &HashSet::new(),
            &no_anchors(),
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
            &no_anchors(),
        );

        assert!(result.contains_key("main.rs"));
        assert!(!result.contains_key("lib.rs"));
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
            &no_anchors(),
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
            &no_anchors(),
        );

        assert!(result.contains_key("src/utils/mod.rs"));
    }

    #[test]
    fn personalization_no_double_count() {
        let mut chat = HashSet::new();
        chat.insert("main.rs".to_string());
        let mut mentioned = HashSet::new();
        mentioned.insert("main.rs".to_string());
        let files = vec!["main.rs".to_string()];

        let result =
            compute_personalization(1, &chat, &files, &mentioned, &HashSet::new(), &no_anchors());

        // max(100, 100) = 100, not 200
        assert!((result["main.rs"] - 100.0).abs() < 0.001);
    }

    #[test]
    fn personalization_anchor_contributions() {
        // Pre-computed: 10x multiplier * (100/2 base) = 500
        let anchors = HashMap::from([("entry.rs".to_string(), 500.0)]);
        let files = vec!["entry.rs".to_string(), "lib.rs".to_string()];

        let result = compute_personalization(
            2,
            &HashSet::new(),
            &files,
            &HashSet::new(),
            &HashSet::new(),
            &anchors,
        );

        assert!(result.contains_key("entry.rs"));
        assert!((result["entry.rs"] - 500.0).abs() < 0.001);
        assert!(!result.contains_key("lib.rs"));
    }

    #[test]
    fn personalization_anchor_ambiguous_divided_weight() {
        // Two files share the ident: each gets half the multiplier weight.
        // Pre-computed: 10 * (100/4) / 2 = 125 each (4 total files, 2 matches, div by 2)
        let anchors = HashMap::from([
            ("app_a/tasks.py".to_string(), 125.0),
            ("app_b/tasks.py".to_string(), 125.0),
        ]);
        let files = vec![
            "app_a/tasks.py".to_string(),
            "app_b/tasks.py".to_string(),
            "lib.rs".to_string(),
            "util.rs".to_string(),
        ];

        let result = compute_personalization(
            4,
            &HashSet::new(),
            &files,
            &HashSet::new(),
            &HashSet::new(),
            &anchors,
        );

        assert!((result["app_a/tasks.py"] - 125.0).abs() < 0.001);
        assert!((result["app_b/tasks.py"] - 125.0).abs() < 0.001);
        assert!(!result.contains_key("lib.rs"));
    }

    #[test]
    fn personalization_anchor_not_in_rel_fnames() {
        // Anchor from ident may not be in other_fnames; inserted via fallback.
        let anchors = HashMap::from([("external.rs".to_string(), 1000.0)]);
        let files = vec!["main.rs".to_string()];

        let result = compute_personalization(
            1,
            &HashSet::new(),
            &files,
            &HashSet::new(),
            &HashSet::new(),
            &anchors,
        );

        assert!(result.contains_key("external.rs"));
    }
}
