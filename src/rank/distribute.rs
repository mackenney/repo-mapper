//! Rank distribution to definitions (SPEC §7.4).

use crate::graph::Graph;
use crate::tag::Tag;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
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
            *ranked_definitions.entry((dst_name, ident)).or_default() += contribution;
        }
    }

    ranked_definitions
}

/// A ranked entry in the final output.
#[derive(Debug, Clone)]
pub enum RankedEntry {
    /// Full entry: file contains one or more tagged definitions for `ident`.
    Tagged {
        /// Relative file path.
        rel_fname: String,
        /// Identifier (function, class, etc.) defined in this file.
        ident: String,
        /// All definition tags for this identifier in this file.
        tags: Vec<Tag>,
        /// Aggregated PageRank score for this (file, ident) pair.
        score: f64,
    },
    /// Bare file entry with no associated definitions.
    Bare {
        /// Relative file path.
        rel_fname: String,
        /// Score (usually `f64::MAX` for forced-include files).
        score: f64,
    },
}

impl RankedEntry {
    /// Returns the relative file path for this entry.
    pub fn rel_fname(&self) -> &str {
        match self {
            RankedEntry::Tagged { rel_fname, .. } => rel_fname,
            RankedEntry::Bare { rel_fname, .. } => rel_fname,
        }
    }

    /// Returns the aggregated PageRank score for this entry.
    pub fn score(&self) -> f64 {
        match self {
            RankedEntry::Tagged { score, .. } => *score,
            RankedEntry::Bare { score, .. } => *score,
        }
    }

    /// Returns `true` if this is a bare (tag-free) file entry.
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

    #[test]
    fn distribute_rank_basic() {
        use crate::graph::Edge;
        use petgraph::graph::DiGraph;

        let mut g: Graph = DiGraph::new();
        let a = g.add_node("a.rs".to_string());
        let b = g.add_node("b.rs".to_string());
        g.add_edge(
            a,
            b,
            Edge {
                ident: "foo".to_string(),
                weight: 1.0,
            },
        );

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
