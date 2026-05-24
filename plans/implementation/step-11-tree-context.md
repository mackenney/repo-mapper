# Step 11: TreeContext

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 7 — Rendering. This step implements scope-aware rendering.

### This Step
Implement TreeContext for extracting scope headers and lines of interest per SPEC §9.2.

## Prerequisites
- Step 04 complete (parser registry)
- Step 10 complete (RankedEntry type)

## Files to Read Before Starting
- `SPEC.md` §9.1, §9.2 — to_tree algorithm, render_tree, TreeContext parameters
- `src/render/tree_context.rs` — current stub

## Implementation

### Task 1: Implement TreeContext in src/render/tree_context.rs

```rust
//! TreeContext for scope-aware rendering (SPEC §9.2).
//!
//! With all SPEC parameters disabled (color=false, line_number=false, etc.),
//! TreeContext shows: scope header lines + lines of interest + ellipsis for gaps.

use crate::lang::detect_language;
use crate::parser::get_parser;
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Node, Tree};

/// TreeContext for scope-aware rendering.
///
/// Per SPEC §9.2, all formatting options are disabled:
/// - color = false
/// - line_number = false
/// - child_context = false
/// - last_line = false
/// - margin = 0
/// - mark_lois = false
/// - loi_pad = 0
/// - show_top_of_file_parent_scope = false
pub struct TreeContext {
    /// Source lines (split by newline)
    lines: Vec<String>,
    /// Parsed tree (if language is recognized)
    tree: Option<Tree>,
    /// Lines of interest (0-indexed internally)
    lois: HashSet<usize>,
    /// Context lines (scope headers, 0-indexed)
    context_lines: HashSet<usize>,
}

impl TreeContext {
    /// Create a new TreeContext from file content.
    pub fn new(content: &str, path: &Path) -> Self {
        let lines: Vec<String> = content.lines().map(String::from).collect();

        // Try to parse if language is recognized
        let tree = detect_language(path)
            .and_then(|lang| get_parser(lang))
            .and_then(|mut parser| parser.parse(content, None));

        TreeContext {
            lines,
            tree,
            lois: HashSet::new(),
            context_lines: HashSet::new(),
        }
    }

    /// Reset lines of interest.
    pub fn reset_lois(&mut self) {
        self.lois.clear();
        self.context_lines.clear();
    }

    /// Add lines of interest (1-indexed input, stored 0-indexed).
    ///
    /// Lines with -1 (fallback refs) are skipped.
    pub fn add_lines_of_interest(&mut self, lois: &[i32]) {
        for &line in lois {
            if line > 0 {
                self.lois.insert((line - 1) as usize);
            }
        }
    }

    /// Add context by finding scope headers for all lines of interest.
    pub fn add_context(&mut self) {
        if let Some(tree) = &self.tree {
            let root = tree.root_node();
            for &loi in &self.lois.clone() {
                self.add_scope_context(&root, loi);
            }
        }
    }

    /// Find scope headers for a line of interest.
    fn add_scope_context(&mut self, root: &Node, loi: usize) {
        // Find the deepest node containing this line
        let point = tree_sitter::Point::new(loi, 0);
        let mut node = root.descendant_for_point_range(point, point);

        // Walk up the tree to find scope nodes
        while let Some(n) = node {
            if is_scope_node(&n) {
                // Add the first line of the scope (the header)
                let start_line = n.start_position().row;
                self.context_lines.insert(start_line);
            }
            node = n.parent();
        }
    }

    /// Format the output with ellipsis for gaps.
    pub fn format(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }

        // Combine lois and context lines
        let mut show_lines: Vec<usize> = self
            .lois
            .iter()
            .chain(self.context_lines.iter())
            .copied()
            .filter(|&l| l < self.lines.len())
            .collect();
        show_lines.sort_unstable();
        show_lines.dedup();

        if show_lines.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        let mut last_line: Option<usize> = None;

        for line_idx in show_lines {
            // Add ellipsis for gaps
            if let Some(last) = last_line {
                if line_idx > last + 1 {
                    output.push_str("⋮\n");
                }
            }

            output.push_str(&self.lines[line_idx]);
            output.push('\n');
            last_line = Some(line_idx);
        }

        output
    }
}

/// Check if a node represents a scope (function, class, etc.).
fn is_scope_node(node: &Node) -> bool {
    let kind = node.kind();
    matches!(
        kind,
        // Functions
        "function_definition"
            | "function_declaration"
            | "function_item"
            | "method_definition"
            | "method_declaration"
            | "arrow_function"
            | "function_expression"
            | "generator_function"
            | "generator_function_declaration"
            // Classes and structs
            | "class_definition"
            | "class_declaration"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "interface_declaration"
            // Impl blocks
            | "impl_item"
            // Modules and namespaces
            | "module"
            | "mod_item"
            | "namespace_definition"
            // Control structures (optional, for more context)
            | "if_statement"
            | "if_expression"
            | "for_statement"
            | "for_expression"
            | "while_statement"
            | "while_expression"
            | "match_expression"
            | "try_statement"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn tree_context_basic() {
        let content = r#"fn main() {
    println!("hello");
}

fn helper() {
    something();
}
"#;
        let mut ctx = TreeContext::new(content, Path::new("test.rs"));
        ctx.add_lines_of_interest(&[2]); // println line
        ctx.add_context();

        let output = ctx.format();
        // Should include fn main() header and the println line
        assert!(output.contains("fn main()"));
        assert!(output.contains("println"));
    }

    #[test]
    fn tree_context_ellipsis() {
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let mut ctx = TreeContext::new(content, Path::new("test.txt"));
        // No tree for .txt, but we can still add lois manually
        ctx.lois.insert(0); // line1
        ctx.lois.insert(4); // line5

        let output = ctx.format();
        assert!(output.contains("line1"));
        assert!(output.contains("⋮")); // Gap between line1 and line5
        assert!(output.contains("line5"));
    }

    #[test]
    fn tree_context_negative_line() {
        let content = "fn foo() {}\n";
        let mut ctx = TreeContext::new(content, Path::new("test.rs"));
        ctx.add_lines_of_interest(&[-1, 1]); // -1 should be ignored
        ctx.add_context();

        // Should not panic, -1 is skipped
        let output = ctx.format();
        assert!(output.contains("fn foo()"));
    }

    #[test]
    fn tree_context_empty() {
        let ctx = TreeContext::new("", Path::new("test.rs"));
        let output = ctx.format();
        assert!(output.is_empty());
    }

    #[test]
    fn tree_context_reset() {
        let content = "fn foo() {}\n";
        let mut ctx = TreeContext::new(content, Path::new("test.rs"));
        ctx.add_lines_of_interest(&[1]);
        ctx.add_context();

        ctx.reset_lois();
        let output = ctx.format();
        assert!(output.is_empty());
    }
}
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test tree_context` exits 0
- [ ] TreeContext parses file with tree-sitter when language is recognized
- [ ] `add_lines_of_interest` handles -1 (fallback refs) by skipping
- [ ] `add_context` finds scope headers (function, class definitions)
- [ ] `format` produces output with `⋮` for non-contiguous line gaps
- [ ] Empty content produces empty output

## Reviewer Instructions

You are reviewing Step 11. Verify:

1. Run `cargo test render::tree_context::tests` — all tests must pass
2. Check `is_scope_node` includes function, class, struct, impl nodes
3. Verify ellipsis character is `⋮` (U+22EE)
4. Check that -1 lines are skipped in `add_lines_of_interest`
5. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/render/tree_context.rs
```
