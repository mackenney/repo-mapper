# Step 06: Graph Construction

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 4 — Graph Construction. This step builds the weighted directed graph.

### This Step
Implement TagIndex, edge weight calculation, and Graph with petgraph per SPEC §6.

## Prerequisites
- Step 02 complete (Tag type)
- Step 05 complete (tag extraction)

## Files to Read Before Starting
- `SPEC.md` §6.1–§6.5 — graph construction algorithm, edge weights
- `src/graph.rs` — current stub
- `src/weight.rs` — current stub

## Implementation

### Task 1: Implement edge weight calculation in src/weight.rs

```rust
//! Edge weight calculation (SPEC §6.4).

use std::collections::HashSet;

/// Compute the edge weight for a reference relationship.
///
/// Per SPEC §6.4, applies multipliers based on:
/// - Identifier in mentioned_idents: ×10
/// - Identifier length ≥8 AND (snake_case OR kebab-case OR camelCase): ×10
/// - Identifier starts with "_": ×0.1
/// - More than 5 distinct definers: ×0.1
/// - Referencer is a chat file: ×50
/// - Final weight = use_mul × sqrt(num_refs)
pub fn compute_edge_weight(
    ident: &str,
    referencer: &str,
    num_refs: usize,
    mentioned_idents: &HashSet<String>,
    chat_rel_fnames: &HashSet<String>,
    num_definers: usize,
) -> f64 {
    let mut mul = 1.0;

    // Condition 1: in mentioned_idents
    if mentioned_idents.contains(ident) {
        mul *= 10.0;
    }

    // Condition 2: length ≥8 AND meaningful identifier pattern
    if ident.len() >= 8 && is_meaningful_ident(ident) {
        mul *= 10.0;
    }

    // Condition 3: starts with "_"
    if ident.starts_with('_') {
        mul *= 0.1;
    }

    // Condition 4: more than 5 distinct definers
    if num_definers > 5 {
        mul *= 0.1;
    }

    // Condition 5: referencer is a chat file
    let use_mul = if chat_rel_fnames.contains(referencer) {
        mul * 50.0
    } else {
        mul
    };

    // Final weight
    use_mul * (num_refs as f64).sqrt()
}

/// Check if an identifier is "meaningful" per SPEC §6.4.
///
/// Returns true if the identifier contains:
/// - `_` with at least one alphabetic character (snake_case), OR
/// - `-` with at least one alphabetic character (kebab-case), OR
/// - Both upper and lower case letters (camelCase/PascalCase)
pub fn is_meaningful_ident(s: &str) -> bool {
    let has_alpha = s.chars().any(|c| c.is_alphabetic());
    if !has_alpha {
        return false;
    }

    // Check for snake_case
    if s.contains('_') {
        return true;
    }

    // Check for kebab-case
    if s.contains('-') {
        return true;
    }

    // Check for mixed case (camelCase/PascalCase)
    let has_upper = s.chars().any(|c| c.is_uppercase());
    let has_lower = s.chars().any(|c| c.is_lowercase());
    if has_upper && has_lower {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_meaningful_snake_case() {
        assert!(is_meaningful_ident("my_function"));
        assert!(is_meaningful_ident("CONST_VALUE"));
    }

    #[test]
    fn is_meaningful_kebab_case() {
        assert!(is_meaningful_ident("my-component"));
    }

    #[test]
    fn is_meaningful_camel_case() {
        assert!(is_meaningful_ident("myFunction"));
        assert!(is_meaningful_ident("MyClass"));
    }

    #[test]
    fn is_meaningful_short_or_simple() {
        assert!(!is_meaningful_ident("foo")); // No special pattern
        assert!(!is_meaningful_ident("FOO")); // All caps, no mixed case
        assert!(!is_meaningful_ident("123")); // No alpha
    }

    #[test]
    fn weight_basic() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("foo", "file.rs", 1, &mentioned, &chat_files, 1);
        assert!((weight - 1.0).abs() < 0.001);
    }

    #[test]
    fn weight_mentioned_ident() {
        let mut mentioned = HashSet::new();
        mentioned.insert("my_function".to_string());
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("my_function", "file.rs", 1, &mentioned, &chat_files, 1);
        // 10.0 (mentioned) * 10.0 (meaningful + len≥8) * sqrt(1) = 100.0
        assert!((weight - 100.0).abs() < 0.001);
    }

    #[test]
    fn weight_chat_file() {
        let mentioned = HashSet::new();
        let mut chat_files = HashSet::new();
        chat_files.insert("main.rs".to_string());
        let weight = compute_edge_weight("foo", "main.rs", 4, &mentioned, &chat_files, 1);
        // 1.0 * 50.0 (chat file) * sqrt(4) = 100.0
        assert!((weight - 100.0).abs() < 0.001);
    }

    #[test]
    fn weight_underscore_prefix() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("_private", "file.rs", 1, &mentioned, &chat_files, 1);
        // 0.1 (underscore) * sqrt(1) = 0.1
        assert!((weight - 0.1).abs() < 0.001);
    }

    #[test]
    fn weight_many_definers() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("foo", "file.rs", 1, &mentioned, &chat_files, 10);
        // 0.1 (>5 definers) * sqrt(1) = 0.1
        assert!((weight - 0.1).abs() < 0.001);
    }

    #[test]
    fn weight_multiple_refs() {
        let mentioned = HashSet::new();
        let chat_files = HashSet::new();
        let weight = compute_edge_weight("foo", "file.rs", 9, &mentioned, &chat_files, 1);
        // 1.0 * sqrt(9) = 3.0
        assert!((weight - 3.0).abs() < 0.001);
    }
}
```

### Task 2: Implement TagIndex in src/graph.rs

```rust
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
    graph
        .edges(node)
        .map(|e| e.weight().weight)
        .sum()
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
        let b_node = graph
            .node_indices()
            .find(|&n| graph[n] == "b.rs")
            .unwrap();

        let total = total_out_weight(&graph, b_node);
        // Two outgoing edges, each weight 1.0 (sqrt(1))
        assert!((total - 2.0).abs() < 0.001);
    }
}
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test weight` exits 0
- [ ] `cargo test graph` exits 0
- [ ] TagIndex correctly populates defines, references, definitions from tags
- [ ] No-reference fallback substitutes defines into references when empty
- [ ] Graph uses petgraph::DiGraph
- [ ] Edge weights calculated per SPEC §6.4 multiplier rules
- [ ] Self-edges added for unreferenced definitions per SPEC §6.5

## Reviewer Instructions

You are reviewing Step 06. Verify:

1. Run `cargo test weight::tests` — all tests must pass
2. Run `cargo test graph::tests` — all tests must pass
3. Check `src/graph.rs` uses `petgraph::graph::DiGraph`
4. Verify `compute_edge_weight` applies all 5 multiplier conditions from SPEC §6.4
5. Verify `build_graph` adds self-edges for identifiers NOT in references
6. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/graph.rs src/weight.rs
```
