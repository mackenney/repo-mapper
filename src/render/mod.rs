//! Map rendering (SPEC §9).

pub mod tree_cache;
pub mod tree_context;

pub use tree_cache::{TreeCache, TreeCacheKey, TreeContextCache};
pub use tree_context::TreeContext;

use crate::file::{get_mtime, read_file_utf8};
use crate::rank::RankedEntry;
use std::collections::HashSet;
use std::path::Path;

/// Render a single file's tree (SPEC §9.2).
pub fn render_tree(
    abs_fname: &Path,
    rel_fname: &str,
    lois: &[i32],
    tree_cache: &mut TreeCache,
    tree_context_cache: &mut TreeContextCache,
) -> String {
    // Get current mtime
    let mtime = get_mtime(abs_fname).unwrap_or(0.0);

    // Check tree_cache first
    let cache_key = TreeCache::make_key(rel_fname, lois, mtime);
    if let Some(cached) = tree_cache.get(&cache_key) {
        return cached.clone();
    }

    // Read file content
    let content = match read_file_utf8(abs_fname) {
        Some(c) => c,
        None => return String::new(),
    };

    // Ensure trailing newline (SPEC §9.2 step 2)
    let content = if content.ends_with('\n') {
        content
    } else {
        format!("{}\n", content)
    };

    // Get or create TreeContext
    let ctx = tree_context_cache.get_or_create(rel_fname, abs_fname, &content, mtime);

    // Reset and populate lois
    ctx.reset_lois();
    ctx.add_lines_of_interest(lois);
    ctx.add_context();

    // Format result
    let result = ctx.format();

    // Cache the result
    tree_cache.set(cache_key, result.clone());

    result
}

/// Convert ranked entries to tree output (SPEC §9.1).
pub fn to_tree(
    ranked_tags: &[RankedEntry],
    chat_rel_fnames: &HashSet<String>,
    max_line_length: usize,
    tree_cache: &mut TreeCache,
    tree_context_cache: &mut TreeContextCache,
) -> String {
    // Empty input → empty string (SPEC §9.1)
    if ranked_tags.is_empty() {
        return String::new();
    }

    // Step 1: Sort lexicographically
    let mut sorted: Vec<&RankedEntry> = ranked_tags.iter().collect();
    sorted.sort_by_key(|e| e.rel_fname());

    // Append sentinel for flushing the last file
    let sentinel = sentinel_entry();
    sorted.push(&sentinel);

    let mut output = String::new();
    let mut cur_fname: Option<&str> = None;
    let mut cur_abs_fname: Option<&str> = None;
    let mut lois: Option<Vec<i32>> = None;

    // Iterate with sentinel handling
    for entry in &sorted {
        let entry_fname = entry.rel_fname();

        // Skip chat files
        if chat_rel_fnames.contains(entry_fname) {
            continue;
        }

        // File boundary check
        if Some(entry_fname) != cur_fname || is_sentinel(entry) {
            // Flush previous file
            if let Some(fname) = cur_fname {
                if let Some(ref loi_list) = lois {
                    // File with tags
                    output.push('\n');
                    output.push_str(fname);
                    output.push_str(":\n");

                    if let Some(abs) = cur_abs_fname {
                        let rendered = render_tree(
                            Path::new(abs),
                            fname,
                            loi_list,
                            tree_cache,
                            tree_context_cache,
                        );
                        output.push_str(&rendered);
                    }
                } else {
                    // Bare file entry
                    output.push('\n');
                    output.push_str(fname);
                    output.push('\n');
                }
            }

            // Start new file if not sentinel
            if !is_sentinel(entry) {
                cur_fname = Some(entry_fname);
                match entry {
                    RankedEntry::Tagged { tags, .. } => {
                        lois = Some(Vec::new());
                        cur_abs_fname = tags.first().map(|t| t.fname.as_str());
                    }
                    RankedEntry::Bare { .. } => {
                        lois = None;
                        cur_abs_fname = None;
                    }
                }
            }
        }

        // Append line to lois if not sentinel
        if !is_sentinel(entry) {
            if let (Some(ref mut loi_list), RankedEntry::Tagged { tags, .. }) = (&mut lois, *entry)
            {
                for tag in tags {
                    loi_list.push(tag.line);
                }
            }
        }
    }

    // Truncate lines
    let output = truncate_lines(&output, max_line_length);

    // Ensure trailing newline
    if output.is_empty() || output.ends_with('\n') {
        output
    } else {
        format!("{}\n", output)
    }
}

/// Sentinel entry for flushing the last file.
fn sentinel_entry() -> RankedEntry {
    RankedEntry::Bare {
        rel_fname: "\x00SENTINEL\x00".to_string(),
        score: 0.0,
    }
}

/// Check if an entry is the sentinel.
fn is_sentinel(entry: &RankedEntry) -> bool {
    entry.rel_fname() == "\x00SENTINEL\x00"
}

/// Truncate each line to max_length characters.
fn truncate_lines(text: &str, max_length: usize) -> String {
    text.lines()
        .map(|line| {
            if line.chars().count() > max_length {
                line.chars().take(max_length).collect::<String>()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::Tag;

    fn make_tagged(rel: &str, abs: &str, lines: &[i32]) -> RankedEntry {
        let tags: Vec<Tag> = lines
            .iter()
            .map(|&l| Tag::def(rel, abs, l, "test"))
            .collect();
        RankedEntry::Tagged {
            rel_fname: rel.to_string(),
            ident: "test".to_string(),
            tags,
            score: 1.0,
        }
    }

    fn make_bare(rel: &str) -> RankedEntry {
        RankedEntry::Bare {
            rel_fname: rel.to_string(),
            score: 0.0,
        }
    }

    #[test]
    fn to_tree_empty() {
        let mut tc = TreeCache::new();
        let mut tcc = TreeContextCache::new();
        let result = to_tree(&[], &HashSet::new(), 100, &mut tc, &mut tcc);
        assert!(result.is_empty());
    }

    #[test]
    fn to_tree_bare_entry() {
        let entries = vec![make_bare("test.rs")];
        let mut tc = TreeCache::new();
        let mut tcc = TreeContextCache::new();
        let result = to_tree(&entries, &HashSet::new(), 100, &mut tc, &mut tcc);

        // Bare entry should produce "\ntest.rs\n"
        assert!(result.contains("test.rs"));
        assert!(!result.contains(":")); // No colon for bare entries
    }

    #[test]
    fn to_tree_excludes_chat() {
        let entries = vec![make_bare("chat.rs"), make_bare("other.rs")];
        let mut chat = HashSet::new();
        chat.insert("chat.rs".to_string());
        let mut tc = TreeCache::new();
        let mut tcc = TreeContextCache::new();

        let result = to_tree(&entries, &chat, 100, &mut tc, &mut tcc);

        assert!(!result.contains("chat.rs"));
        assert!(result.contains("other.rs"));
    }

    #[test]
    fn truncate_lines_basic() {
        let text = "short\nthis line is quite long\n";
        let result = truncate_lines(text, 10);
        assert!(result.lines().all(|l| l.len() <= 10));
    }
}
