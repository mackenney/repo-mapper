//! Graph construction (SPEC §6).

use crate::tag::{Tag, TagKind};
use crate::weight::compute_edge_weight;
use petgraph::graph::DiGraph;
use std::collections::{HashMap, HashSet};

/// Intermediate structures built from tags (SPEC §6.1).
#[derive(Debug, Default)]
pub struct TagIndex {
    /// identifier_name → set<rel_fname> (files that define it)
    pub defines: HashMap<String, HashSet<String>>,
    /// identifier_name → list<rel_fname> (files that reference it, with multiplicity)
    pub references: HashMap<String, Vec<String>>,
    /// (rel_fname, identifier_name) → set<Tag> (definition tags)
    pub definitions: HashMap<(String, String), HashSet<Tag>>,
}

impl TagIndex {
    /// Build a TagIndex from an iterator of tags.
    pub fn from_tags(tags: impl Iterator<Item = Tag>) -> Self {
        let mut index = TagIndex::default();

        for tag in tags {
            match tag.kind {
                TagKind::Def => {
                    // Add to defines
                    index
                        .defines
                        .entry(tag.name.clone())
                        .or_default()
                        .insert(tag.rel_fname.clone());

                    // Add to definitions
                    index
                        .definitions
                        .entry((tag.rel_fname.clone(), tag.name.clone()))
                        .or_default()
                        .insert(tag);
                }
                TagKind::Ref => {
                    // Add to references (with multiplicity)
                    index
                        .references
                        .entry(tag.name.clone())
                        .or_default()
                        .push(tag.rel_fname.clone());
                }
            }
        }

        index
    }

    /// Apply no-reference fallback (SPEC §6.2).
    ///
    /// If references is empty, substitute with defines.
    pub fn apply_no_reference_fallback(&mut self) {
        if self.references.is_empty() {
            for (name, fnames) in &self.defines {
                self.references
                    .insert(name.clone(), fnames.iter().cloned().collect());
            }
        }
    }
}

/// Edge in the repo graph.
#[derive(Debug, Clone)]
pub struct Edge {
    /// The identifier that creates this relationship
    pub ident: String,
    /// Edge weight
    pub weight: f64,
}

/// A weighted directed multi-graph of file relationships (SPEC §6.3).
pub type Graph = DiGraph<String, Edge>;

/// Build the graph from a TagIndex (SPEC §6.4, §6.5).
pub fn build_graph(
    index: &TagIndex,
    mentioned_idents: &HashSet<String>,
    chat_rel_fnames: &HashSet<String>,
    self_edge_weight: f64,
) -> Graph {
    let mut graph = Graph::new();
    let mut node_indices: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    // Helper to get or create a node
    let mut get_node = |graph: &mut Graph, fname: &str| -> petgraph::graph::NodeIndex {
        if let Some(&idx) = node_indices.get(fname) {
            idx
        } else {
            let idx = graph.add_node(fname.to_string());
            node_indices.insert(fname.to_string(), idx);
            idx
        }
    };

    // Track which identifiers have references
    let idents_with_refs: HashSet<&String> = index.references.keys().collect();

    // Step 1: Build edges for identifiers in both defines and references
    for ident in index.defines.keys() {
        if !index.references.contains_key(ident) {
            continue;
        }

        let definers = &index.defines[ident];
        let num_definers = definers.len();

        // Count references per referencer
        let mut ref_counts: HashMap<&str, usize> = HashMap::new();
        for referencer in &index.references[ident] {
            *ref_counts.entry(referencer.as_str()).or_default() += 1;
        }

        // Create edges
        for (referencer, num_refs) in ref_counts {
            for definer in definers {
                let weight = compute_edge_weight(
                    ident,
                    referencer,
                    num_refs,
                    mentioned_idents,
                    chat_rel_fnames,
                    num_definers,
                );

                let src = get_node(&mut graph, referencer);
                let dst = get_node(&mut graph, definer);
                graph.add_edge(
                    src,
                    dst,
                    Edge {
                        ident: ident.clone(),
                        weight,
                    },
                );
            }
        }
    }

    // Step 2: Add self-edges for unreferenced definitions (SPEC §6.5)
    for (ident, definers) in &index.defines {
        if idents_with_refs.contains(ident) {
            continue;
        }

        for definer in definers {
            let node = get_node(&mut graph, definer);
            graph.add_edge(
                node,
                node,
                Edge {
                    ident: ident.clone(),
                    weight: self_edge_weight,
                },
            );
        }
    }

    graph
}

/// Get the total outgoing weight for a node.
pub fn total_out_weight(graph: &Graph, node: petgraph::graph::NodeIndex) -> f64 {
    graph.edges(node).map(|e| e.weight().weight).sum()
}

/// Get all graph nodes as rel_fname strings.
pub fn graph_nodes(graph: &Graph) -> Vec<&str> {
    graph.node_weights().map(|s| s.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::Tag;

    #[test]
    fn tag_index_from_tags() {
        let tags = vec![
            Tag::def("a.rs", "/a.rs", 1, "Foo"),
            Tag::def("a.rs", "/a.rs", 5, "Bar"),
            Tag::reference("b.rs", "/b.rs", 10, "Foo"),
            Tag::reference("b.rs", "/b.rs", 15, "Foo"),
        ];

        let index = TagIndex::from_tags(tags.into_iter());

        assert!(index.defines["Foo"].contains("a.rs"));
        assert!(index.defines["Bar"].contains("a.rs"));
        assert_eq!(index.references["Foo"].len(), 2); // Two refs to Foo
        assert!(!index.references.contains_key("Bar")); // No refs to Bar
    }

    #[test]
    fn no_reference_fallback() {
        let tags = vec![
            Tag::def("a.rs", "/a.rs", 1, "Foo"),
            Tag::def("b.rs", "/b.rs", 1, "Bar"),
        ];

        let mut index = TagIndex::from_tags(tags.into_iter());
        assert!(index.references.is_empty());

        index.apply_no_reference_fallback();
        assert!(index.references.contains_key("Foo"));
        assert!(index.references.contains_key("Bar"));
    }

    #[test]
    fn build_graph_basic() {
        let tags = vec![
            Tag::def("a.rs", "/a.rs", 1, "Foo"),
            Tag::reference("b.rs", "/b.rs", 10, "Foo"),
        ];

        let index = TagIndex::from_tags(tags.into_iter());
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();

        let graph = build_graph(&index, &mentioned, &chat_files, 0.1);

        // Should have 2 nodes: a.rs and b.rs
        assert_eq!(graph.node_count(), 2);
        // Should have 1 edge: b.rs -> a.rs
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn build_graph_self_edges() {
        let tags = vec![
            Tag::def("a.rs", "/a.rs", 1, "Foo"),
            // No references to Foo
        ];

        let mut index = TagIndex::from_tags(tags.into_iter());
        // Don't apply fallback, so Foo has no references
        let _ = &mut index; // suppress unused warning
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();

        let graph = build_graph(&index, &mentioned, &chat_files, 0.5);

        // Should have self-edge for a.rs
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 1);

        // Check edge weight is self_edge_weight
        let edge = graph.edge_weights().next().unwrap();
        assert!((edge.weight - 0.5).abs() < 0.001);
    }

    #[test]
    fn total_out_weight_calculation() {
        let tags = vec![
            Tag::def("a.rs", "/a.rs", 1, "Foo"),
            Tag::def("a.rs", "/a.rs", 2, "Bar"),
            Tag::reference("b.rs", "/b.rs", 10, "Foo"),
            Tag::reference("b.rs", "/b.rs", 20, "Bar"),
        ];

        let index = TagIndex::from_tags(tags.into_iter());
        let graph = build_graph(&index, &HashSet::new(), &HashSet::new(), 0.1);

        // Find b.rs node
        let b_node = graph.node_indices().find(|&n| graph[n] == "b.rs").unwrap();

        let total = total_out_weight(&graph, b_node);
        // Two outgoing edges, each weight 1.0 (sqrt(1))
        assert!((total - 2.0).abs() < 0.001);
    }
}
