# Step 12: Render Tree

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 7 — Rendering. This step runs in parallel with TreeContext.

### This Step
Implement render_tree, to_tree, and render caching per SPEC §9.1, §9.2.

## Prerequisites
- Step 11 complete (TreeContext)

## Files to Read Before Starting
- `SPEC.md` §9.1, §9.2 — to_tree algorithm, render caching
- `src/render/tree_cache.rs` — current stub
- `src/render/mod.rs` — current stub

## Implementation

### Task 1: Implement tree cache in src/render/tree_cache.rs

```rust
//! Render result caching (SPEC §9.2).

use crate::render::tree_context::TreeContext;
use std::collections::HashMap;

/// Cache key for rendered tree results: (rel_fname, sorted_lois, mtime_bits).
pub type TreeCacheKey = (String, Vec<i32>, u64);

/// Cache for rendered tree strings.
#[derive(Debug, Default)]
pub struct TreeCache {
    cache: HashMap<TreeCacheKey, String>,
}

impl TreeCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the cache (called at start of each uncached computation).
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get a cached render result.
    pub fn get(&self, key: &TreeCacheKey) -> Option<&String> {
        self.cache.get(key)
    }

    /// Store a render result.
    pub fn set(&mut self, key: TreeCacheKey, value: String) {
        self.cache.insert(key, value);
    }

    /// Create a cache key from components.
    pub fn make_key(rel_fname: &str, lois: &[i32], mtime: f64) -> TreeCacheKey {
        let mut sorted_lois: Vec<i32> = lois.to_vec();
        sorted_lois.sort_unstable();
        (rel_fname.to_string(), sorted_lois, mtime.to_bits())
    }
}

/// Cache for TreeContext objects, keyed by rel_fname.
///
/// Per SPEC §9.2: stored with mtime, replaced on mismatch.
#[derive(Debug, Default)]
pub struct TreeContextCache {
    cache: HashMap<String, (TreeContext, f64)>,
}

impl TreeContextCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a TreeContext for a file.
    ///
    /// Returns the cached context if mtime matches, otherwise creates new.
    pub fn get_or_create(
        &mut self,
        rel_fname: &str,
        abs_fname: &std::path::Path,
        content: &str,
        current_mtime: f64,
    ) -> &mut TreeContext {
        // Check if we have a valid cached entry
        let needs_new = self
            .cache
            .get(rel_fname)
            .map(|(_, mtime)| (*mtime - current_mtime).abs() > 0.001)
            .unwrap_or(true);

        if needs_new {
            let ctx = TreeContext::new(content, abs_fname);
            self.cache.insert(rel_fname.to_string(), (ctx, current_mtime));
        }

        &mut self.cache.get_mut(rel_fname).unwrap().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_cache_key_sorting() {
        let key1 = TreeCache::make_key("test.rs", &[3, 1, 2], 1000.0);
        let key2 = TreeCache::make_key("test.rs", &[1, 2, 3], 1000.0);
        // Keys should be equal (lois sorted)
        assert_eq!(key1, key2);
    }

    #[test]
    fn tree_cache_set_get() {
        let mut cache = TreeCache::new();
        let key = TreeCache::make_key("test.rs", &[1, 2], 1000.0);
        cache.set(key.clone(), "rendered content".to_string());

        let result = cache.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "rendered content");
    }

    #[test]
    fn tree_cache_clear() {
        let mut cache = TreeCache::new();
        let key = TreeCache::make_key("test.rs", &[1], 1000.0);
        cache.set(key.clone(), "content".to_string());
        cache.clear();

        assert!(cache.get(&key).is_none());
    }
}
```

### Task 2: Implement render functions in src/render/mod.rs

```rust
//! Map rendering (SPEC §9).

mod tree_context;
mod tree_cache;

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

    // Step 2: Append sentinel
    // (handled implicitly by checking for file boundary at end)

    let mut output = String::new();
    let mut cur_fname: Option<&str> = None;
    let mut cur_abs_fname: Option<&str> = None;
    let mut lois: Option<Vec<i32>> = None;

    // Step 3: Iterate with sentinel handling
    for entry in sorted.iter().chain(std::iter::once(&sentinel_entry())) {
        let entry_fname = entry.rel_fname();

        // Step 4: Skip chat files
        if chat_rel_fnames.contains(entry_fname) {
            continue;
        }

        // Step 5: File boundary check
        if Some(entry_fname) != cur_fname || is_sentinel(entry) {
            // Flush previous file
            if let Some(fname) = cur_fname {
                if let Some(ref loi_list) = lois {
                    // 5a: File with tags
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
                    // 5b: Bare file entry
                    output.push('\n');
                    output.push_str(fname);
                    output.push('\n');
                }
            }

            // 5c-d: Start new file if not sentinel
            if !is_sentinel(entry) {
                cur_fname = Some(entry_fname);
                match entry {
                    RankedEntry::Tagged { tags, .. } => {
                        lois = Some(Vec::new());
                        // Get abs_fname from first tag
                        cur_abs_fname = tags.first().map(|t| t.fname.as_str());
                    }
                    RankedEntry::Bare { .. } => {
                        lois = None;
                        cur_abs_fname = None;
                    }
                }
            }
        }

        // Step 6: Append line to lois if not sentinel
        if !is_sentinel(entry) {
            if let (Some(ref mut loi_list), RankedEntry::Tagged { tags, .. }) = (&mut lois, entry) {
                for tag in tags {
                    loi_list.push(tag.line);
                }
            }
        }
    }

    // Step 7: Truncate lines
    let output = truncate_lines(&output, max_line_length);

    // Step 8: Ensure trailing newline
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
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test render` exits 0
- [ ] `render_tree` caches by (rel_fname, sorted_lois, mtime)
- [ ] `to_tree` returns empty string for empty input
- [ ] `to_tree` excludes chat_rel_fnames
- [ ] File entries with tags produce `\n{fname}:\n{rendered}`
- [ ] Bare file entries produce `\n{fname}\n`
- [ ] Lines truncated to max_line_length
- [ ] Output ends with newline

## Reviewer Instructions

You are reviewing Step 12. Verify:

1. Run `cargo test render::tests` — all tests must pass
2. Check `render_tree` uses cache key `(rel_fname, sorted_lois, mtime_bits)`
3. Verify `to_tree` uses sentinel pattern for flushing last file
4. Check chat file exclusion happens in step 4 of iteration
5. Verify `truncate_lines` handles Unicode correctly (uses `.chars().count()`)
6. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/render/
```
