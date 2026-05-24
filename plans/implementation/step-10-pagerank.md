# Step 10: PageRank

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 6 — Ranking. This step implements the core ranking algorithm.

### This Step
Implement personalization, PageRank, rank distribution, and ranked tag list per SPEC §7.

## Prerequisites
- Step 06 complete (Graph with petgraph)
- Step 07 complete (important files)

## Files to Read Before Starting
- `SPEC.md` §7.1–§7.5 — personalization, PageRank, distribution, ranked tags
- `src/rank/mod.rs` — current stub
- `src/rank/personalization.rs` — current stub
- `src/rank/pagerank.rs` — current stub
- `src/rank/distribute.rs` — current stub

## Implementation

### Task 1: Implement personalization in src/rank/personalization.rs

```rust
//! Personalization vector computation (SPEC §7.1).

use crate::path::path_components;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Compute the personalization vector for PageRank.
///
/// Per SPEC §7.1:
/// - Base = 100.0 / N
/// - Chat files: add base
/// - Mentioned files: max(current, base)
/// - Path components in mentioned_idents: add base
pub fn compute_personalization(
    total_files: usize,
    chat_fnames: &[PathBuf],
    rel_fnames: &[String],
    mentioned_fnames: &HashSet<String>,
    mentioned_idents: &HashSet<String>,
) -> HashMap<String, f64> {
    if total_files == 0 {
        return HashMap::new();
    }

    let personalize = 100.0 / total_files as f64;
    let mut result: HashMap<String, f64> = HashMap::new();

    // Build chat_rel_fnames set for lookup
    let chat_rel: HashSet<&str> = chat_fnames
        .iter()
        .filter_map(|p| p.to_str())
        .collect();

    for rel_fname in rel_fnames {
        let mut current_pers = 0.0;

        // Step 2: Chat files add personalize
        if chat_rel.contains(rel_fname.as_str()) {
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

        // Only include files with positive personalization
        if current_pers > 0.0 {
            result.insert(rel_fname.clone(), current_pers);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personalization_empty() {
        let result = compute_personalization(0, &[], &[], &HashSet::new(), &HashSet::new());
        assert!(result.is_empty());
    }

    #[test]
    fn personalization_chat_files() {
        let chat = vec![PathBuf::from("main.rs")];
        let files = vec!["main.rs".to_string(), "lib.rs".to_string()];

        let result = compute_personalization(
            2,
            &chat,
            &files,
            &HashSet::new(),
            &HashSet::new(),
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

        let result = compute_personalization(2, &[], &files, &mentioned, &HashSet::new());

        assert!(result.contains_key("lib.rs"));
        assert!((result["lib.rs"] - 50.0).abs() < 0.001);
    }

    #[test]
    fn personalization_mentioned_idents() {
        let mut idents = HashSet::new();
        idents.insert("utils".to_string());
        let files = vec!["src/utils/mod.rs".to_string()];

        let result = compute_personalization(1, &[], &files, &HashSet::new(), &idents);

        // "utils" is in the path, so file gets personalization
        assert!(result.contains_key("src/utils/mod.rs"));
    }

    #[test]
    fn personalization_no_double_count() {
        let chat = vec![PathBuf::from("main.rs")];
        let mut mentioned = HashSet::new();
        mentioned.insert("main.rs".to_string());
        let files = vec!["main.rs".to_string()];

        let result = compute_personalization(1, &chat, &files, &mentioned, &HashSet::new());

        // Should be max(100, 100) = 100, not 200
        assert!((result["main.rs"] - 100.0).abs() < 0.001);
    }
}
```

### Task 2: Implement PageRank in src/rank/pagerank.rs

```rust
//! PageRank algorithm (SPEC §7.2).

use crate::graph::{Graph, Edge};
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use thiserror::Error;

/// PageRank algorithm parameters.
#[derive(Debug, Clone)]
pub struct PageRankParams {
    /// Damping factor (default: 0.85)
    pub damping: f64,
    /// Convergence tolerance (default: 1e-6)
    pub tol: f64,
    /// Maximum iterations (default: 100)
    pub max_iter: usize,
}

impl Default for PageRankParams {
    fn default() -> Self {
        Self {
            damping: 0.85,
            tol: 1e-6,
            max_iter: 100,
        }
    }
}

/// PageRank errors.
#[derive(Debug, Error)]
pub enum PageRankError {
    #[error("Zero division: personalization vector sums to zero")]
    ZeroDivision,
}

/// Run PageRank on the graph.
///
/// Returns a map from rel_fname to PageRank score.
pub fn pagerank(
    graph: &Graph,
    personalization: Option<&HashMap<String, f64>>,
    params: &PageRankParams,
) -> Result<HashMap<String, f64>, PageRankError> {
    let n = graph.node_count();
    if n == 0 {
        return Ok(HashMap::new());
    }

    // Build node index to name mapping
    let node_names: HashMap<NodeIndex, &str> = graph
        .node_indices()
        .map(|idx| (idx, graph[idx].as_str()))
        .collect();

    let name_to_idx: HashMap<&str, NodeIndex> = node_names
        .iter()
        .map(|(&idx, &name)| (name, idx))
        .collect();

    // Initialize personalization vector
    let pers = match personalization {
        Some(p) if !p.is_empty() => {
            let total: f64 = p.values().sum();
            if total == 0.0 {
                return Err(PageRankError::ZeroDivision);
            }
            // Normalize to sum to 1.0
            p.iter()
                .filter_map(|(name, &val)| {
                    name_to_idx.get(name.as_str()).map(|&idx| (idx, val / total))
                })
                .collect::<HashMap<_, _>>()
        }
        _ => {
            // Uniform distribution
            let uniform = 1.0 / n as f64;
            graph.node_indices().map(|idx| (idx, uniform)).collect()
        }
    };

    // Dangling nodes: nodes with no outgoing edges
    let dangling: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|&idx| graph.edges(idx).count() == 0)
        .collect();

    // Initialize rank vector
    let mut rank: HashMap<NodeIndex, f64> = pers.clone();
    // Ensure all nodes have a rank
    for idx in graph.node_indices() {
        rank.entry(idx).or_insert(1.0 / n as f64);
    }

    // Power iteration
    let d = params.damping;
    for _ in 0..params.max_iter {
        let mut new_rank: HashMap<NodeIndex, f64> = HashMap::new();

        // Dangling node contribution
        let dangling_sum: f64 = dangling.iter().map(|&idx| rank[&idx]).sum();

        for idx in graph.node_indices() {
            // Teleport + dangling redistribution
            let pers_val = pers.get(&idx).copied().unwrap_or(1.0 / n as f64);
            let mut r = (1.0 - d) * pers_val + d * dangling_sum * pers_val;

            // Sum contributions from incoming edges
            for edge in graph.edges_directed(idx, petgraph::Direction::Incoming) {
                let src = edge.source();
                let src_rank = rank[&src];
                let edge_weight = edge.weight().weight;

                // Compute out-weight of source
                let out_weight: f64 = graph.edges(src).map(|e| e.weight().weight).sum();
                if out_weight > 0.0 {
                    r += d * src_rank * edge_weight / out_weight;
                }
            }

            new_rank.insert(idx, r);
        }

        // Check convergence
        let diff: f64 = graph
            .node_indices()
            .map(|idx| (rank[&idx] - new_rank[&idx]).abs())
            .sum();

        rank = new_rank;

        if diff < params.tol {
            break;
        }
    }

    // Convert back to names
    Ok(rank
        .into_iter()
        .map(|(idx, r)| (node_names[&idx].to_string(), r))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;
    use petgraph::graph::DiGraph;

    fn simple_graph() -> Graph {
        let mut g = DiGraph::new();
        let a = g.add_node("a.rs".to_string());
        let b = g.add_node("b.rs".to_string());
        g.add_edge(a, b, Edge { ident: "foo".to_string(), weight: 1.0 });
        g.add_edge(b, a, Edge { ident: "bar".to_string(), weight: 1.0 });
        g
    }

    #[test]
    fn pagerank_empty_graph() {
        let g: Graph = DiGraph::new();
        let result = pagerank(&g, None, &PageRankParams::default()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn pagerank_simple() {
        let g = simple_graph();
        let result = pagerank(&g, None, &PageRankParams::default()).unwrap();

        assert!(result.contains_key("a.rs"));
        assert!(result.contains_key("b.rs"));
        // Symmetric graph should have equal ranks
        let diff = (result["a.rs"] - result["b.rs"]).abs();
        assert!(diff < 0.01);
    }

    #[test]
    fn pagerank_with_personalization() {
        let g = simple_graph();
        let mut pers = HashMap::new();
        pers.insert("a.rs".to_string(), 100.0);

        let result = pagerank(&g, Some(&pers), &PageRankParams::default()).unwrap();

        // a.rs should have higher rank due to personalization
        assert!(result["a.rs"] > result["b.rs"]);
    }

    #[test]
    fn pagerank_zero_division() {
        let g = simple_graph();
        let mut pers = HashMap::new();
        pers.insert("a.rs".to_string(), 0.0);

        let result = pagerank(&g, Some(&pers), &PageRankParams::default());
        assert!(matches!(result, Err(PageRankError::ZeroDivision)));
    }

    #[test]
    fn pagerank_sum_approximately_one() {
        let g = simple_graph();
        let result = pagerank(&g, None, &PageRankParams::default()).unwrap();
        let sum: f64 = result.values().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
```

### Task 3: Implement rank distribution in src/rank/distribute.rs

```rust
//! Rank distribution to definitions (SPEC §7.4).

use crate::graph::{Graph, TagIndex};
use crate::tag::Tag;
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};

/// Distribute PageRank scores to (file, identifier) pairs.
///
/// Per SPEC §7.4: for each src node, distribute its rank proportionally
/// across outgoing edges to (dst, ident) pairs.
pub fn distribute_rank(
    graph: &Graph,
    pagerank: &HashMap<String, f64>,
) -> HashMap<(String, String), f64> {
    let mut ranked_definitions: HashMap<(String, String), f64> = HashMap::new();

    // Build name to index mapping
    let name_to_idx: HashMap<&str, NodeIndex> = graph
        .node_indices()
        .map(|idx| (graph[idx].as_str(), idx))
        .collect();

    for (src_name, &src_rank) in pagerank {
        let src_idx = match name_to_idx.get(src_name.as_str()) {
            Some(&idx) => idx,
            None => continue,
        };

        // Compute total outgoing weight
        let total_weight: f64 = graph.edges(src_idx).map(|e| e.weight().weight).sum();
        if total_weight == 0.0 {
            continue;
        }

        // Distribute to each outgoing edge
        for edge in graph.edges(src_idx) {
            let dst_idx = edge.target();
            let dst_name = graph[dst_idx].clone();
            let ident = edge.weight().ident.clone();
            let weight = edge.weight().weight;

            let contribution = src_rank * weight / total_weight;
            *ranked_definitions
                .entry((dst_name, ident))
                .or_default() += contribution;
        }
    }

    ranked_definitions
}

/// A ranked entry in the final output.
#[derive(Debug, Clone)]
pub enum RankedEntry {
    /// Full entry with definition tags
    Tagged {
        rel_fname: String,
        ident: String,
        tags: Vec<Tag>,
        score: f64,
    },
    /// Bare file entry (no tags)
    Bare {
        rel_fname: String,
        score: f64,
    },
}

impl RankedEntry {
    pub fn rel_fname(&self) -> &str {
        match self {
            RankedEntry::Tagged { rel_fname, .. } => rel_fname,
            RankedEntry::Bare { rel_fname, .. } => rel_fname,
        }
    }

    pub fn score(&self) -> f64 {
        match self {
            RankedEntry::Tagged { score, .. } => *score,
            RankedEntry::Bare { score, .. } => *score,
        }
    }

    pub fn is_bare(&self) -> bool {
        matches!(self, RankedEntry::Bare { .. })
    }
}

/// Build the ranked tag list (SPEC §7.5).
pub fn build_ranked_tags(
    ranked_definitions: HashMap<(String, String), f64>,
    definitions: &HashMap<(String, String), HashSet<Tag>>,
    chat_rel_fnames: &HashSet<String>,
    other_rel_fnames: &HashSet<String>,
    pagerank: &HashMap<String, f64>,
    exclude_unranked: bool,
) -> Vec<RankedEntry> {
    let mut result = Vec::new();
    let mut included_files: HashSet<String> = HashSet::new();

    // Step 1: Sort ranked_definitions descending by score
    let mut sorted: Vec<_> = ranked_definitions.into_iter().collect();
    sorted.sort_by(|a, b| {
        // Primary: descending score
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            // Secondary: reverse-lexicographic on (fname, ident)
            .then_with(|| b.0.cmp(&a.0))
    });

    // Step 2-3: Add tagged entries (excluding chat files)
    for ((fname, ident), score) in sorted {
        if chat_rel_fnames.contains(&fname) {
            continue;
        }

        if let Some(tags) = definitions.get(&(fname.clone(), ident.clone())) {
            result.push(RankedEntry::Tagged {
                rel_fname: fname.clone(),
                ident,
                tags: tags.iter().cloned().collect(),
                score,
            });
            included_files.insert(fname);
        }
    }

    // Step 4: Add bare entries for graph nodes not yet included
    let mut graph_files: Vec<_> = pagerank
        .iter()
        .filter(|(name, _)| !included_files.contains(*name))
        .filter(|(name, _)| !chat_rel_fnames.contains(*name))
        .collect();
    graph_files.sort_by(|a, b| {
        b.1.partial_cmp(a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.0.cmp(a.0))
    });

    for (fname, &score) in graph_files {
        result.push(RankedEntry::Bare {
            rel_fname: fname.clone(),
            score,
        });
        included_files.insert(fname.clone());
    }

    // Step 5: Add bare entries for files never in graph
    let mut remaining: Vec<_> = other_rel_fnames
        .iter()
        .filter(|name| !included_files.contains(*name))
        .filter(|name| !chat_rel_fnames.contains(*name))
        .collect();
    remaining.sort();

    for fname in remaining {
        result.push(RankedEntry::Bare {
            rel_fname: fname.clone(),
            score: 0.0,
        });
    }

    // Step 6: Filter unranked if requested
    if exclude_unranked {
        result.retain(|entry| {
            if entry.is_bare() {
                entry.score() > 0.0001
            } else {
                true // Never remove tagged entries
            }
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::{Tag, TagKind};

    #[test]
    fn distribute_rank_basic() {
        use crate::graph::Edge;
        use petgraph::graph::DiGraph;

        let mut g: Graph = DiGraph::new();
        let a = g.add_node("a.rs".to_string());
        let b = g.add_node("b.rs".to_string());
        g.add_edge(a, b, Edge { ident: "foo".to_string(), weight: 1.0 });

        let mut pr = HashMap::new();
        pr.insert("a.rs".to_string(), 0.5);
        pr.insert("b.rs".to_string(), 0.5);

        let result = distribute_rank(&g, &pr);

        // a.rs distributes 0.5 to (b.rs, foo)
        assert!((result[&("b.rs".to_string(), "foo".to_string())] - 0.5).abs() < 0.01);
    }

    #[test]
    fn build_ranked_tags_excludes_chat() {
        let ranked_defs = HashMap::new();
        let definitions = HashMap::new();
        let mut chat = HashSet::new();
        chat.insert("chat.rs".to_string());
        let mut other = HashSet::new();
        other.insert("chat.rs".to_string());
        other.insert("other.rs".to_string());
        let pr = HashMap::new();

        let result = build_ranked_tags(ranked_defs, &definitions, &chat, &other, &pr, false);

        // chat.rs should be excluded
        assert!(result.iter().all(|e| e.rel_fname() != "chat.rs"));
        assert!(result.iter().any(|e| e.rel_fname() == "other.rs"));
    }

    #[test]
    fn build_ranked_tags_exclude_unranked() {
        let ranked_defs = HashMap::new();
        let definitions = HashMap::new();
        let chat = HashSet::new();
        let mut other = HashSet::new();
        other.insert("low.rs".to_string());

        let mut pr = HashMap::new();
        pr.insert("low.rs".to_string(), 0.00001); // Below threshold

        let result = build_ranked_tags(ranked_defs, &definitions, &chat, &other, &pr, true);

        // low.rs should be excluded due to low rank
        assert!(result.iter().all(|e| e.rel_fname() != "low.rs"));
    }
}
```

### Task 4: Update src/rank/mod.rs

```rust
//! PageRank ranking (SPEC §7).

mod personalization;
mod pagerank;
mod distribute;

pub use personalization::compute_personalization;
pub use pagerank::{pagerank, PageRankError, PageRankParams};
pub use distribute::{build_ranked_tags, distribute_rank, RankedEntry};
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test rank` exits 0
- [ ] Personalization applies chat file, mentioned file, and mentioned_idents rules
- [ ] Personalization uses max() for chat+mentioned overlap (no double-count)
- [ ] PageRank uses power iteration with damping, tolerance, max_iter
- [ ] PageRank handles empty personalization and zero-division
- [ ] Rank distribution distributes proportionally by edge weight
- [ ] Ranked tags excludes chat files
- [ ] exclude_unranked filters bare entries with score ≤ 0.0001

## Reviewer Instructions

You are reviewing Step 10. Verify:

1. Run `cargo test rank` — all tests must pass
2. Check personalization implements SPEC §7.1 steps 1-4
3. Check PageRank returns ZeroDivision error for zero personalization sum
4. Verify distribute_rank computes `src_rank * weight / total_weight`
5. Verify build_ranked_tags sorts descending by score, then reverse-lex
6. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/rank/
```
