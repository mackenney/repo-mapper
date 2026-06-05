//! Core tag extraction (SPEC §3).

use crate::lang::detect_language;
use crate::parser::get_parser;
use crate::queries::get_queries;
use crate::tag::{Tag, TagKind};
use std::path::Path;
use tree_sitter::{Query, QueryCursor, StreamingIterator};

/// Extract tags from a source file.
///
/// Returns an empty vector if:
/// - Language is unrecognized (SPEC §3.1 step 2)
/// - Parser unavailable (SPEC §3.1 step 3)
/// - No query file exists (SPEC §3.1 step 4)
/// - File is empty
pub fn extract_tags(fname: &Path, rel_fname: &str, content: &str) -> Vec<Tag> {
    if content.is_empty() {
        return Vec::new();
    }

    // Step 1: Detect language
    let lang = match detect_language(fname) {
        Some(l) => l,
        None => return Vec::new(),
    };

    // Step 2-3: Get parser
    let mut parser = match get_parser(lang) {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Step 4: Get query
    let query_pair = match get_queries(lang) {
        Some(q) => q,
        None => return Vec::new(),
    };

    // Step 5: Parse
    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Get the language for query compilation
    let ts_language = match crate::parser::get_language(lang) {
        Some(l) => l,
        None => return Vec::new(),
    };

    // Compile the query
    let query = match Query::new(&ts_language, query_pair.tags) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let source_bytes = content.as_bytes();
    let root_node = tree.root_node();

    // Step 6-9: Execute query and collect tags
    let mut tags = Vec::new();
    let mut saw_def = false;
    let mut saw_ref = false;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, source_bytes);
    let capture_names = query.capture_names();

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let capture_name = &capture_names[capture.index as usize];
            let node = capture.node;

            // Get identifier text
            let name = match node.utf8_text(source_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            // Line number: 1-indexed (SPEC §3.4)
            let line = (node.start_position().row + 1) as i32;

            // Determine tag kind from capture name prefix
            let kind = if capture_name.starts_with("name.definition.") {
                saw_def = true;
                TagKind::Def
            } else if capture_name.starts_with("name.reference.") {
                saw_ref = true;
                TagKind::Ref
            } else {
                // SPEC §3.1 step 9: ignore other capture names
                continue;
            };

            tags.push(Tag::new(
                rel_fname.to_string(),
                fname.to_string_lossy().to_string(),
                line,
                name,
                kind,
            ));
        }
    }

    // Step 8: Apply identifier fallback if needed (SPEC §3.2)
    if saw_def
        && !saw_ref
        && let Some(idents_query_src) = query_pair.idents
    {
        let fallback_tags = extract_idents_fallback(
            &ts_language,
            idents_query_src,
            &root_node,
            source_bytes,
            rel_fname,
            &fname.to_string_lossy(),
        );
        tags.extend(fallback_tags);
    }

    tags
}

/// Extract identifier references as fallback (SPEC §3.2).
///
/// Used when a file has definitions but no references.
fn extract_idents_fallback(
    language: &tree_sitter::Language,
    query_src: &str,
    root_node: &tree_sitter::Node,
    source_bytes: &[u8],
    rel_fname: &str,
    fname: &str,
) -> Vec<Tag> {
    let query = match Query::new(language, query_src) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let mut tags = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, *root_node, source_bytes);
    let capture_names = query.capture_names();

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let capture_name = &capture_names[capture.index as usize];

            // Only process name.reference.* captures (SPEC §3.2)
            if !capture_name.starts_with("name.reference.") {
                continue;
            }

            let node = capture.node;
            let name = match node.utf8_text(source_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            // Fallback tags use line = -1 (SPEC §3.2 step 3)
            tags.push(Tag::new(
                rel_fname.to_string(),
                fname.to_string(),
                -1,
                name,
                TagKind::Ref,
            ));
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn extract_rust_tags() {
        let content = r#"
fn main() {
    println!("hello");
}

struct Foo {
    bar: i32,
}

impl Foo {
    fn new() -> Self {
        Self { bar: 0 }
    }
}
"#;
        let tags = extract_tags(Path::new("test.rs"), "test.rs", content);

        // Should have definitions for main, Foo, new
        let defs: Vec<_> = tags.iter().filter(|t| t.is_def()).collect();
        assert!(defs.iter().any(|t| t.name == "main"));
        assert!(defs.iter().any(|t| t.name == "Foo"));
        assert!(defs.iter().any(|t| t.name == "new"));

        // Should have references for println
        let refs: Vec<_> = tags.iter().filter(|t| t.is_ref()).collect();
        assert!(refs.iter().any(|t| t.name == "println"));
    }

    #[test]
    fn extract_python_tags() {
        let content = r#"
class MyClass:
    def method(self):
        pass

def my_function():
    helper()

CONSTANT = 42
"#;
        let tags = extract_tags(Path::new("test.py"), "test.py", content);

        let defs: Vec<_> = tags.iter().filter(|t| t.is_def()).collect();
        assert!(defs.iter().any(|t| t.name == "MyClass"));
        assert!(defs.iter().any(|t| t.name == "method"));
        assert!(defs.iter().any(|t| t.name == "my_function"));

        let refs: Vec<_> = tags.iter().filter(|t| t.is_ref()).collect();
        assert!(refs.iter().any(|t| t.name == "helper"));
    }

    #[test]
    fn extract_typescript_fallback() {
        // TypeScript file with class but no type annotations or new expressions
        // Should trigger identifier fallback
        let content = r#"
class MyClass {
    constructor() {}
    doStuff() {
        helper();
    }
}
"#;
        let tags = extract_tags(Path::new("test.ts"), "test.ts", content);

        let defs: Vec<_> = tags.iter().filter(|t| t.is_def()).collect();
        // Should have MyClass definition
        assert!(defs.iter().any(|t| t.name == "MyClass"));

        let refs: Vec<_> = tags.iter().filter(|t| t.is_ref()).collect();
        // Should have fallback identifier refs with line = -1
        assert!(!refs.is_empty());
        // Fallback refs have line = -1
        assert!(refs.iter().all(|t| t.line == -1));
    }

    #[test]
    fn extract_unknown_language() {
        let tags = extract_tags(Path::new("data.csv"), "data.csv", "a,b,c\n1,2,3");
        assert!(tags.is_empty());
    }

    #[test]
    fn extract_empty_file() {
        let tags = extract_tags(Path::new("empty.rs"), "empty.rs", "");
        assert!(tags.is_empty());
    }

    #[test]
    fn line_numbers_are_1_indexed() {
        let content = "fn foo() {}\n";
        let tags = extract_tags(Path::new("test.rs"), "test.rs", content);
        let foo = tags.iter().find(|t| t.name == "foo").unwrap();
        assert_eq!(foo.line, 1); // First line is 1, not 0
    }
}
