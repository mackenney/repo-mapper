# Step 14: Public API

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 9 — Public API. This step integrates all components.

### This Step
Implement RepoMapConfig, RepoMap, get_repo_map, and edge case handling per SPEC §12–14.

## Prerequisites
- Step 08 complete (tag cache)
- Step 09 complete (map cache)
- Step 10 complete (PageRank)
- Step 12 complete (rendering)
- Step 13 complete (token budget)

## Files to Read Before Starting
- `SPEC.md` §12 (output format), §13 (edge cases), §14 (configuration)
- `src/config.rs` — current stub
- `src/repo_map.rs` — current stub
- `src/edge_cases.rs` — current stub

## Implementation

### Task 1: Complete RepoMapConfig in src/config.rs

```rust
//! Configuration types (SPEC §14).

use crate::cache::map_cache::RefreshMode;
use std::path::PathBuf;

pub use crate::cache::map_cache::RefreshMode;

/// Configuration for RepoMap (SPEC §14).
#[derive(Debug, Clone)]
pub struct RepoMapConfig {
    /// Maximum tokens in repo map output (default: 1024)
    pub map_tokens: usize,
    /// Repository root directory (default: cwd)
    pub root: PathBuf,
    /// Prefix prepended to map output
    pub repo_content_prefix: Option<String>,
    /// Emit diagnostic output
    pub verbose: bool,
    /// LLM context window size
    pub max_context_window: Option<usize>,
    /// Multiplier for no-chat mode (default: 8)
    pub map_mul_no_files: usize,
    /// Map cache refresh mode (default: Auto)
    pub refresh: RefreshMode,
    /// Force cache recomputation
    pub force_refresh: bool,
    /// Exclude files with PageRank ≤ 0.0001
    pub exclude_unranked: bool,
    /// Self-edge weight (default: 0.1)
    pub self_edge_weight: f64,
    /// Maximum line length (default: 100)
    pub max_line_length: usize,
    /// PageRank damping factor (default: 0.85)
    pub pagerank_damping: f64,
    /// PageRank convergence tolerance (default: 1e-6)
    pub pagerank_tol: f64,
    /// PageRank maximum iterations (default: 100)
    pub pagerank_max_iter: usize,
}

impl Default for RepoMapConfig {
    fn default() -> Self {
        Self {
            map_tokens: 1024,
            root: std::env::current_dir().unwrap_or_default(),
            repo_content_prefix: None,
            verbose: false,
            max_context_window: None,
            map_mul_no_files: 8,
            refresh: RefreshMode::Auto,
            force_refresh: false,
            exclude_unranked: false,
            self_edge_weight: 0.1,
            max_line_length: 100,
            pagerank_damping: 0.85,
            pagerank_tol: 1e-6,
            pagerank_max_iter: 100,
        }
    }
}

/// Builder for RepoMapConfig.
#[derive(Debug, Default)]
pub struct RepoMapConfigBuilder {
    config: RepoMapConfig,
}

impl RepoMapConfig {
    /// Create a new builder.
    pub fn builder() -> RepoMapConfigBuilder {
        RepoMapConfigBuilder::default()
    }
}

impl RepoMapConfigBuilder {
    pub fn map_tokens(mut self, n: usize) -> Self {
        self.config.map_tokens = n;
        self
    }

    pub fn root(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.root = path.into();
        self
    }

    pub fn repo_content_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.repo_content_prefix = Some(prefix.into());
        self
    }

    pub fn verbose(mut self, v: bool) -> Self {
        self.config.verbose = v;
        self
    }

    pub fn max_context_window(mut self, n: Option<usize>) -> Self {
        self.config.max_context_window = n;
        self
    }

    pub fn map_mul_no_files(mut self, n: usize) -> Self {
        self.config.map_mul_no_files = n;
        self
    }

    pub fn refresh(mut self, mode: RefreshMode) -> Self {
        self.config.refresh = mode;
        self
    }

    pub fn force_refresh(mut self, v: bool) -> Self {
        self.config.force_refresh = v;
        self
    }

    pub fn exclude_unranked(mut self, v: bool) -> Self {
        self.config.exclude_unranked = v;
        self
    }

    pub fn self_edge_weight(mut self, w: f64) -> Self {
        self.config.self_edge_weight = w;
        self
    }

    pub fn max_line_length(mut self, n: usize) -> Self {
        self.config.max_line_length = n;
        self
    }

    pub fn pagerank_damping(mut self, d: f64) -> Self {
        self.config.pagerank_damping = d;
        self
    }

    pub fn pagerank_tol(mut self, t: f64) -> Self {
        self.config.pagerank_tol = t;
        self
    }

    pub fn pagerank_max_iter(mut self, n: usize) -> Self {
        self.config.pagerank_max_iter = n;
        self
    }

    pub fn build(self) -> crate::repo_map::RepoMap {
        crate::repo_map::RepoMap::new(self.config)
    }
}
```

### Task 2: Implement edge case handling in src/edge_cases.rs

```rust
//! Edge case handling (SPEC §13).

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Per-instance warning deduplication (SPEC §13.5).
pub struct WarnedFiles {
    warned: RefCell<HashSet<PathBuf>>,
}

impl WarnedFiles {
    pub fn new() -> Self {
        Self {
            warned: RefCell::new(HashSet::new()),
        }
    }

    /// Warn about a missing file once per path per instance.
    pub fn warn_missing(&self, path: &Path) {
        let mut warned = self.warned.borrow_mut();
        if !warned.contains(path) {
            warn!("File not found, skipping: {}", path.display());
            warned.insert(path.to_path_buf());
        }
    }
}

impl Default for WarnedFiles {
    fn default() -> Self {
        Self::new()
    }
}

/// Substitute {other} placeholder in prefix (SPEC §12.1).
pub fn substitute_prefix(prefix: Option<&str>, has_chat_files: bool) -> String {
    match prefix {
        None => String::new(),
        Some(p) => {
            let other = if has_chat_files { "other " } else { "" };
            p.replace("{other}", other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_with_chat() {
        let result = substitute_prefix(Some("Here are the {other}files:"), true);
        assert_eq!(result, "Here are the other files:");
    }

    #[test]
    fn substitute_without_chat() {
        let result = substitute_prefix(Some("Here are the {other}files:"), false);
        assert_eq!(result, "Here are the files:");
    }

    #[test]
    fn substitute_none() {
        let result = substitute_prefix(None, true);
        assert_eq!(result, "");
    }
}
```

### Task 3: Implement RepoMap in src/repo_map.rs

```rust
//! RepoMap public API.

use crate::budget::{binary_search_budget, compute_effective_max};
use crate::cache::map_cache::{MapCache, MapCacheKey, RefreshMode};
use crate::cache::tag_cache::TagCache;
use crate::config::RepoMapConfig;
use crate::edge_cases::{substitute_prefix, WarnedFiles};
use crate::extract::extract_tags;
use crate::file::{get_mtime, is_regular_file, read_file_utf8};
use crate::graph::{build_graph, TagIndex};
use crate::important::filter_important_files;
use crate::path::rel_path;
use crate::rank::{
    build_ranked_tags, compute_personalization, distribute_rank, pagerank, PageRankParams,
    RankedEntry,
};
use crate::render::{TreeCache, TreeContextCache};
use crate::tokens::TokenCounter;
use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, error};

/// Main entry point for repo map generation.
pub struct RepoMap {
    config: RepoMapConfig,
    /// Mutable internal copy of map_tokens (SPEC §14)
    max_map_tokens: Cell<usize>,
    /// Per-instance warning deduplication (SPEC §13.5)
    warned_files: WarnedFiles,
    /// Persistent tag cache
    tag_cache: TagCache,
    /// In-memory map cache
    map_cache: MapCache,
    /// Token counter
    token_counter: TokenCounter,
    /// Tree render cache (cleared each computation)
    tree_cache: TreeCache,
    /// TreeContext cache (persists across computations)
    tree_context_cache: TreeContextCache,
}

impl RepoMap {
    /// Create a new RepoMap with the given configuration.
    pub fn new(config: RepoMapConfig) -> Self {
        let tag_cache = TagCache::new(&config.root);
        let max_tokens = config.map_tokens;

        Self {
            config,
            max_map_tokens: Cell::new(max_tokens),
            warned_files: WarnedFiles::new(),
            tag_cache,
            map_cache: MapCache::new(),
            token_counter: TokenCounter::new(),
            tree_cache: TreeCache::new(),
            tree_context_cache: TreeContextCache::new(),
        }
    }

    /// Generate a repo map.
    ///
    /// Returns `None` when:
    /// - max_map_tokens <= 0 (SPEC §13.1)
    /// - other_fnames is empty (SPEC §13.2)
    /// - No files listing produced (SPEC §12.1)
    pub fn get_repo_map(
        &mut self,
        chat_fnames: &[PathBuf],
        other_fnames: &[PathBuf],
        mentioned_fnames: &HashSet<String>,
        mentioned_idents: &HashSet<String>,
    ) -> Option<String> {
        // §13.1: Zero budget
        if self.max_map_tokens.get() == 0 {
            return None;
        }

        // §13.2: No other files
        if other_fnames.is_empty() {
            return None;
        }

        // Compute effective max for no-chat mode (§10.3)
        let effective_max = compute_effective_max(
            self.max_map_tokens.get(),
            chat_fnames.is_empty(),
            self.config.max_context_window,
            self.config.map_mul_no_files,
        );

        // Build cache key
        let cache_key = if self.config.refresh == RefreshMode::Auto {
            MapCacheKey::auto(
                chat_fnames,
                other_fnames,
                effective_max,
                mentioned_fnames,
                mentioned_idents,
            )
        } else {
            MapCacheKey::files(chat_fnames, other_fnames, effective_max)
        };

        // Check map cache
        if let Some(cached) = self
            .map_cache
            .get(&cache_key, self.config.refresh, self.config.force_refresh)
        {
            debug!("Map cache hit");
            return Some(substitute_prefix(
                self.config.repo_content_prefix.as_deref(),
                !chat_fnames.is_empty(),
            ) + cached);
        }

        // Clear tree cache for new computation (SPEC §9.2)
        self.tree_cache.clear();

        let start = Instant::now();
        let result = self.compute_map(
            chat_fnames,
            other_fnames,
            mentioned_fnames,
            mentioned_idents,
            effective_max,
        );
        let duration = start.elapsed();

        // Store in cache regardless of mode (SPEC §11)
        if let Some(ref files_listing) = result {
            self.map_cache
                .set(cache_key, files_listing.clone(), duration);
        }

        result.map(|files_listing| {
            substitute_prefix(
                self.config.repo_content_prefix.as_deref(),
                !chat_fnames.is_empty(),
            ) + &files_listing
        })
    }

    fn compute_map(
        &mut self,
        chat_fnames: &[PathBuf],
        other_fnames: &[PathBuf],
        mentioned_fnames: &HashSet<String>,
        mentioned_idents: &HashSet<String>,
        effective_max: usize,
    ) -> Option<String> {
        // Deduplicate and sort files (SPEC §2.2)
        let mut all_fnames: Vec<PathBuf> = chat_fnames
            .iter()
            .chain(other_fnames.iter())
            .cloned()
            .collect();
        all_fnames.sort();
        all_fnames.dedup();

        // Build rel_fname mappings
        let chat_rel_fnames: HashSet<String> = chat_fnames
            .iter()
            .map(|p| rel_path(p, &self.config.root))
            .collect();

        let other_rel_fnames: HashSet<String> = other_fnames
            .iter()
            .map(|p| rel_path(p, &self.config.root))
            .collect();

        // Extract tags
        let mut all_tags = Vec::new();
        let mut rel_fnames = Vec::new();

        for fname in &all_fnames {
            // Check file exists (SPEC §13.5)
            if !is_regular_file(fname) {
                self.warned_files.warn_missing(fname);
                continue;
            }

            let rel_fname = rel_path(fname, &self.config.root);
            rel_fnames.push(rel_fname.clone());

            // Get mtime
            let mtime = match get_mtime(fname) {
                Some(m) => m,
                None => continue, // TOCTOU: silently skip
            };

            // Check tag cache
            let fname_str = fname.to_string_lossy().to_string();
            if let Some(cached_tags) = self.tag_cache.get(&fname_str, mtime) {
                all_tags.extend(cached_tags);
                continue;
            }

            // Extract tags
            let content = match read_file_utf8(fname) {
                Some(c) => c,
                None => continue,
            };

            let tags = extract_tags(fname, &rel_fname, &content);
            self.tag_cache.set(&fname_str, mtime, tags.clone());
            all_tags.extend(tags);
        }

        // Build TagIndex
        let mut tag_index = TagIndex::from_tags(all_tags.into_iter());
        tag_index.apply_no_reference_fallback();

        // Build graph
        let graph = build_graph(
            &tag_index,
            mentioned_idents,
            &chat_rel_fnames,
            self.config.self_edge_weight,
        );

        // Compute personalization
        let personalization = compute_personalization(
            rel_fnames.len(),
            chat_fnames,
            &rel_fnames,
            mentioned_fnames,
            mentioned_idents,
        );

        // Run PageRank
        let params = PageRankParams {
            damping: self.config.pagerank_damping,
            tol: self.config.pagerank_tol,
            max_iter: self.config.pagerank_max_iter,
        };

        let pr_result = pagerank(&graph, Some(&personalization), &params);
        let pagerank_scores = match pr_result {
            Ok(scores) => scores,
            Err(e) => {
                // §7.3: Retry without personalization
                debug!("PageRank failed with personalization: {}, retrying", e);
                match pagerank(&graph, None, &params) {
                    Ok(scores) => scores,
                    Err(e2) => {
                        error!("PageRank failed: {}", e2);
                        // §13.4: Disable map on failure
                        self.max_map_tokens.set(0);
                        return None;
                    }
                }
            }
        };

        // Distribute rank
        let ranked_defs = distribute_rank(&graph, &pagerank_scores);

        // Build ranked tags
        let mut ranked_tags = build_ranked_tags(
            ranked_defs,
            &tag_index.definitions,
            &chat_rel_fnames,
            &other_rel_fnames,
            &pagerank_scores,
            self.config.exclude_unranked,
        );

        // Prepend important files (§8.2)
        let other_rel_vec: Vec<&str> = other_rel_fnames.iter().map(|s| s.as_str()).collect();
        let important = filter_important_files(&other_rel_vec);
        let included: HashSet<&str> = ranked_tags.iter().map(|e| e.rel_fname()).collect();

        let mut important_entries: Vec<RankedEntry> = important
            .into_iter()
            .filter(|f| !included.contains(f))
            .map(|f| RankedEntry::Bare {
                rel_fname: f.to_string(),
                score: f64::MAX, // Sort to top
            })
            .collect();
        important_entries.append(&mut ranked_tags);
        ranked_tags = important_entries;

        // Binary search for budget
        binary_search_budget(
            &ranked_tags,
            effective_max,
            &chat_rel_fnames,
            self.config.max_line_length,
            &self.token_counter,
            &mut self.tree_cache,
            &mut self.tree_context_cache,
        )
    }

    /// Get the current max_map_tokens value.
    pub fn max_map_tokens(&self) -> usize {
        self.max_map_tokens.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn setup_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("main.rs"),
            "fn main() { helper(); }\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("lib.rs"),
            "fn helper() {}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn get_repo_map_empty_other() {
        let dir = setup_test_repo();
        let config = RepoMapConfig {
            root: dir.path().to_path_buf(),
            ..Default::default()
        };
        let mut rm = RepoMap::new(config);

        let result = rm.get_repo_map(&[], &[], &HashSet::new(), &HashSet::new());
        assert!(result.is_none()); // §13.2
    }

    #[test]
    fn get_repo_map_zero_budget() {
        let dir = setup_test_repo();
        let config = RepoMapConfig {
            root: dir.path().to_path_buf(),
            map_tokens: 0,
            ..Default::default()
        };
        let mut rm = RepoMap::new(config);

        let other = vec![dir.path().join("main.rs")];
        let result = rm.get_repo_map(&[], &other, &HashSet::new(), &HashSet::new());
        assert!(result.is_none()); // §13.1
    }

    #[test]
    fn get_repo_map_basic() {
        let dir = setup_test_repo();
        let config = RepoMapConfig {
            root: dir.path().to_path_buf(),
            map_tokens: 1000,
            ..Default::default()
        };
        let mut rm = RepoMap::new(config);

        let other = vec![dir.path().join("main.rs"), dir.path().join("lib.rs")];
        let result = rm.get_repo_map(&[], &other, &HashSet::new(), &HashSet::new());

        assert!(result.is_some());
        let map = result.unwrap();
        assert!(map.contains("main.rs") || map.contains("lib.rs"));
    }
}
```

### Task 4: Update lib.rs exports

Update `src/lib.rs` to export the public API:

```rust
// Add at the end of existing exports:
pub use repo_map::RepoMap;
pub use config::{RepoMapConfig, RepoMapConfigBuilder};
pub use cache::map_cache::RefreshMode;
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test config` exits 0
- [ ] `cargo test edge_cases` exits 0
- [ ] `cargo test repo_map` exits 0
- [ ] RepoMapConfig has all 13 parameters from SPEC §14
- [ ] Builder pattern works with fluent setters
- [ ] get_repo_map returns None for zero budget (§13.1)
- [ ] get_repo_map returns None for empty other_fnames (§13.2)
- [ ] Missing files warn once per path per instance (§13.5)
- [ ] {other} placeholder substituted correctly (§12.1)

## Reviewer Instructions

You are reviewing Step 14. Verify:

1. Run `cargo test config` — all tests must pass
2. Run `cargo test edge_cases` — all tests must pass
3. Run `cargo test repo_map` — all tests must pass
4. Check RepoMapConfig default values match SPEC §14
5. Verify get_repo_map checks §13.1 and §13.2 early returns
6. Check important files prepended before binary search
7. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/config.rs src/edge_cases.rs src/repo_map.rs src/lib.rs
```
