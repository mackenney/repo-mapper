//! RepoMap public API.

use crate::budget::{binary_search_budget, compute_effective_max};
use crate::cache::map_cache::{MapCache, MapCacheKey, RefreshMode};
use crate::cache::tag_cache::TagCache;
use crate::config::RepoMapConfig;
use crate::edge_cases::{WarnedFiles, substitute_prefix};
use crate::extract::extract_tags;
use crate::file::{get_mtime, is_regular_file, read_file_utf8};
use crate::graph::{TagIndex, build_graph};
use crate::important::filter_important_files;
use crate::path::rel_path;
use crate::rank::{
    PageRankParams, RankedEntry, build_ranked_tags, compute_personalization, distribute_rank,
    pagerank,
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
                crate::cache::map_cache::AnchorCacheParams {
                    anchor_fnames: &self.config.anchor_fnames,
                    anchor_idents: &self.config.anchor_idents,
                    anchor_scoped: &self.config.anchor_scoped,
                },
            )
        } else {
            MapCacheKey::files(chat_fnames, other_fnames, effective_max)
        };

        // Check map cache
        if let Some(cached) =
            self.map_cache
                .get(&cache_key, self.config.refresh, self.config.force_refresh)
        {
            debug!("Map cache hit");
            return Some(
                substitute_prefix(
                    self.config.repo_content_prefix.as_deref(),
                    !chat_fnames.is_empty(),
                ) + cached,
            );
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

        // Resolve anchor inputs to per-file personalization contributions (SPEC §7.1a).
        // Must happen after tag index is built so we can look up ident → defining files.
        //
        // anchor_fnames  : full weight each.
        // anchor_idents  : look up in tag_index.defines; divide weight when ambiguous; warn.
        // anchor_scoped  : full weight to the specified file; ident gets edge-weight boost.
        let n_files = rel_fnames.len().max(1);
        let personalize = 100.0 / n_files as f64;
        let anchor_w = self.config.anchor_weight_multiplier;

        let mut anchor_contributions: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        let mut scoped_idents: HashSet<String> = HashSet::new();

        for p in &self.config.anchor_fnames {
            let rel = rel_path(p, &self.config.root);
            *anchor_contributions.entry(rel).or_default() += personalize * anchor_w;
        }

        for ident in &self.config.anchor_idents {
            match tag_index.defines.get(ident) {
                None => {
                    debug!("anchor ident '{}' not found in tag index; ignoring", ident);
                }
                Some(files) => {
                    let n_matches = files.len() as f64;
                    if n_matches > 1.0 {
                        tracing::warn!(
                            "anchor '{}' is defined in {} files; distributing weight equally. \
                            Use -a file.py:ident to target one specific definition.",
                            ident,
                            files.len()
                        );
                    }
                    let weight = personalize * anchor_w / n_matches;
                    for f in files {
                        *anchor_contributions.entry(f.clone()).or_default() += weight;
                    }
                }
            }
        }

        for (p, ident) in &self.config.anchor_scoped {
            let rel = rel_path(p, &self.config.root);
            *anchor_contributions.entry(rel).or_default() += personalize * anchor_w;
            scoped_idents.insert(ident.clone());
        }

        if !anchor_contributions.is_empty() {
            debug!(
                "Anchor contributions: {:?}",
                anchor_contributions.keys().collect::<Vec<_>>()
            );
        }

        // Merge scoped idents into mentioned_idents for edge-weight boosting.
        // Borrow the original when no scoped anchors to avoid an allocation.
        let merged_idents: HashSet<String>;
        let effective_mentioned_idents: &HashSet<String> = if scoped_idents.is_empty() {
            mentioned_idents
        } else {
            merged_idents = mentioned_idents
                .iter()
                .chain(scoped_idents.iter())
                .cloned()
                .collect();
            &merged_idents
        };

        // Build graph
        let graph = build_graph(
            &tag_index,
            effective_mentioned_idents,
            &chat_rel_fnames,
            self.config.self_edge_weight,
        );

        // Compute personalization (includes anchor step, SPEC §7.1 step 5)
        let personalization = compute_personalization(
            rel_fnames.len(),
            &chat_rel_fnames,
            &rel_fnames,
            mentioned_fnames,
            effective_mentioned_idents,
            &anchor_contributions,
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
                score: f64::MAX,
            })
            .collect();
        important_entries.append(&mut ranked_tags);
        ranked_tags = important_entries;

        // Prepend anchor files (SPEC §7.1b) — guaranteed inclusion regardless of budget.
        // Extract ALL existing entries for anchor files from their natural (low) rank position
        // and move them to the front, preserving any tagged definitions they carry.
        // If an anchor file has no entries at all (not in graph), add a bare entry.
        if !anchor_contributions.is_empty() {
            let anchor_files: std::collections::HashSet<&str> =
                anchor_contributions.keys().map(|s| s.as_str()).collect();

            let mut anchor_entries: Vec<RankedEntry> = Vec::new();
            let mut rest: Vec<RankedEntry> = Vec::new();
            for entry in ranked_tags {
                if anchor_files.contains(entry.rel_fname()) {
                    anchor_entries.push(entry);
                } else {
                    rest.push(entry);
                }
            }

            // Add a bare entry for any anchor file that has no existing entries.
            let covered: std::collections::HashSet<String> = anchor_entries
                .iter()
                .map(|e| e.rel_fname().to_string())
                .collect();
            for f in &anchor_files {
                if !covered.contains(*f) {
                    anchor_entries.push(RankedEntry::Bare {
                        rel_fname: f.to_string(),
                        score: f64::MAX,
                    });
                }
            }

            anchor_entries.append(&mut rest);
            ranked_tags = anchor_entries;
        }

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
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() { helper(); }\n").unwrap();
        fs::write(dir.path().join("lib.rs"), "fn helper() {}\n").unwrap();
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
