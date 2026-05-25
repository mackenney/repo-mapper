//! PageRank algorithm (SPEC §7.2).

use crate::graph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
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

    let name_to_idx: HashMap<&str, NodeIndex> =
        node_names.iter().map(|(&idx, &name)| (name, idx)).collect();

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
                    name_to_idx
                        .get(name.as_str())
                        .map(|&idx| (idx, val / total))
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
        g.add_edge(
            a,
            b,
            Edge {
                ident: "foo".to_string(),
                weight: 1.0,
            },
        );
        g.add_edge(
            b,
            a,
            Edge {
                ident: "bar".to_string(),
                weight: 1.0,
            },
        );
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
