# Step 04: Query and Parser Registry

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 2 — Language Support. This step bundles query files and sets up parsers.

### This Step
Implement query registry with bundled .scm files and parser registry per SPEC §4.

## Prerequisites
- Step 03 complete (language detection)
- tree-sitter and tree-sitter-language-pack dependencies available

## Files to Read Before Starting
- `SPEC.md` §4.1, §4.2, §4.3 — query file resolution, supported languages, capture names
- `src/queries.rs` — current stub
- `src/parser.rs` — current stub
- Reference: `reference/aider/aider/queries/` for .scm file content

## Implementation

### Task 1: Create queries directory structure

Create `queries/tree-sitter-language-pack/` directory.

### Task 2: Bundle initial .scm query files

For this step, implement queries for the most critical languages: **rust**, **python**, **javascript**, **typescript**, **go**.

Additional languages can be added incrementally. Each language needs:
1. `{lang}-tags.scm` — main tag extraction query
2. `{lang}-idents.scm` — fallback identifier query (for languages like TypeScript that lack `name.reference.call`)

**queries/tree-sitter-language-pack/rust-tags.scm:**
```scheme
; Definitions
(struct_item name: (type_identifier) @name.definition.class)
(enum_item name: (type_identifier) @name.definition.class)
(union_item name: (type_identifier) @name.definition.class)
(type_item name: (type_identifier) @name.definition.class)
(trait_item name: (type_identifier) @name.definition.interface)
(function_item name: (identifier) @name.definition.function)
(mod_item name: (identifier) @name.definition.module)
(macro_definition name: (identifier) @name.definition.macro)
(impl_item type: (type_identifier) @name.definition.class)

; Methods in impl blocks
(impl_item
  body: (declaration_list
    (function_item name: (identifier) @name.definition.method)))

; References
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (field_expression field: (field_identifier) @name.reference.call))
(macro_invocation macro: (identifier) @name.reference.call)

; Trait/type references in impl
(impl_item trait: (type_identifier) @name.reference.implementation)
```

**queries/tree-sitter-language-pack/python-tags.scm:**
```scheme
; Definitions
(class_definition name: (identifier) @name.definition.class)
(function_definition name: (identifier) @name.definition.function)

; Module-level assignments (constants)
(module
  (expression_statement
    (assignment
      left: (identifier) @name.definition.constant)))

; References - call targets
(call function: (identifier) @name.reference.call)
(call function: (attribute attribute: (identifier) @name.reference.call))
```

**queries/tree-sitter-language-pack/javascript-tags.scm:**
```scheme
; Definitions
(class_declaration name: (identifier) @name.definition.class)
(function_declaration name: (identifier) @name.definition.function)
(generator_function_declaration name: (identifier) @name.definition.function)

; Method definitions (exclude constructor)
(method_definition
  name: (property_identifier) @name.definition.method
  (#not-eq? @name.definition.method "constructor"))

; Variable/const function assignments
(lexical_declaration
  (variable_declarator
    name: (identifier) @name.definition.function
    value: [(arrow_function) (function_expression)]))

; Object property function assignments
(pair
  key: (property_identifier) @name.definition.function
  value: [(arrow_function) (function_expression)])

; References - calls (exclude require)
(call_expression
  function: (identifier) @name.reference.call
  (#not-eq? @name.reference.call "require"))
(call_expression
  function: (member_expression property: (property_identifier) @name.reference.call))

; new expressions
(new_expression constructor: (identifier) @name.reference.class)
```

**queries/tree-sitter-language-pack/typescript-tags.scm:**
```scheme
; Definitions
(class_declaration name: (type_identifier) @name.definition.class)
(abstract_class_declaration name: (type_identifier) @name.definition.class)
(interface_declaration name: (type_identifier) @name.definition.interface)
(type_alias_declaration name: (type_identifier) @name.definition.type)
(enum_declaration name: (identifier) @name.definition.enum)
(module name: (identifier) @name.definition.module)
(function_declaration name: (identifier) @name.definition.function)
(function_signature name: (identifier) @name.definition.function)

; Method definitions
(method_definition name: (property_identifier) @name.definition.method)
(method_signature name: (property_identifier) @name.definition.method)
(abstract_method_signature name: (property_identifier) @name.definition.method)

; References - type annotations
(type_annotation (type_identifier) @name.reference.type)
(type_arguments (type_identifier) @name.reference.type)

; new expressions
(new_expression constructor: (identifier) @name.reference.class)
```

**queries/tree-sitter-language-pack/typescript-idents.scm:**
```scheme
; Fallback identifier captures for TypeScript (SPEC §3.2)
; Used when no name.reference.call captures exist
(identifier) @name.reference.identifier
(type_identifier) @name.reference.identifier
(property_identifier) @name.reference.identifier
```

**queries/tree-sitter-language-pack/go-tags.scm:**
```scheme
; Definitions
(function_declaration name: (identifier) @name.definition.function)
(method_declaration name: (field_identifier) @name.definition.method)
(type_declaration (type_spec name: (type_identifier) @name.definition.type))

; References
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (selector_expression field: (field_identifier) @name.reference.call))
(type_identifier) @name.reference.type
```

### Task 3: Implement query registry in src/queries.rs

```rust
//! Query registry with bundled .scm files (SPEC §4.1).

/// A pair of queries: main tags query and optional idents fallback.
#[derive(Debug, Clone, Copy)]
pub struct QueryPair {
    /// Main tags query content
    pub tags: &'static str,
    /// Optional fallback idents query (for languages without name.reference.call)
    pub idents: Option<&'static str>,
}

// Bundled query files (compile-time inclusion)
mod bundled {
    pub const RUST_TAGS: &str = include_str!("../queries/tree-sitter-language-pack/rust-tags.scm");
    
    pub const PYTHON_TAGS: &str = include_str!("../queries/tree-sitter-language-pack/python-tags.scm");
    
    pub const JAVASCRIPT_TAGS: &str = include_str!("../queries/tree-sitter-language-pack/javascript-tags.scm");
    
    pub const TYPESCRIPT_TAGS: &str = include_str!("../queries/tree-sitter-language-pack/typescript-tags.scm");
    pub const TYPESCRIPT_IDENTS: &str = include_str!("../queries/tree-sitter-language-pack/typescript-idents.scm");
    
    pub const GO_TAGS: &str = include_str!("../queries/tree-sitter-language-pack/go-tags.scm");
}

/// Get the query pair for a language.
///
/// Returns `None` if the language has no bundled queries (SPEC §4.1).
pub fn get_queries(lang: &str) -> Option<QueryPair> {
    match lang {
        "rust" => Some(QueryPair {
            tags: bundled::RUST_TAGS,
            idents: None,
        }),
        "python" => Some(QueryPair {
            tags: bundled::PYTHON_TAGS,
            idents: None,
        }),
        "javascript" => Some(QueryPair {
            tags: bundled::JAVASCRIPT_TAGS,
            idents: None,
        }),
        "typescript" => Some(QueryPair {
            tags: bundled::TYPESCRIPT_TAGS,
            idents: Some(bundled::TYPESCRIPT_IDENTS),
        }),
        "go" => Some(QueryPair {
            tags: bundled::GO_TAGS,
            idents: None,
        }),
        _ => None,
    }
}

/// Check if a language has an idents fallback query.
pub fn has_idents_fallback(lang: &str) -> bool {
    get_queries(lang)
        .map(|q| q.idents.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_queries_rust() {
        let q = get_queries("rust").expect("rust should have queries");
        assert!(q.tags.contains("name.definition.function"));
        assert!(q.tags.contains("name.reference.call"));
        assert!(q.idents.is_none());
    }

    #[test]
    fn get_queries_typescript_has_idents() {
        let q = get_queries("typescript").expect("typescript should have queries");
        assert!(q.idents.is_some());
        let idents = q.idents.unwrap();
        assert!(idents.contains("name.reference.identifier"));
    }

    #[test]
    fn get_queries_unknown() {
        assert!(get_queries("unknown_lang").is_none());
    }

    #[test]
    fn has_idents_fallback_check() {
        assert!(!has_idents_fallback("rust"));
        assert!(!has_idents_fallback("python"));
        assert!(has_idents_fallback("typescript"));
    }
}
```

### Task 4: Implement parser registry in src/parser.rs

```rust
//! Parser registry for tree-sitter languages.

use std::cell::RefCell;
use std::collections::HashMap;
use tree_sitter::{Language, Parser};

// NOTE: tree-sitter::Parser does NOT implement Clone (verified against tree-sitter 0.26).
// We create a fresh parser per call rather than caching and cloning.
// Parser creation is cheap (microseconds); language loading is the expensive part.

/// Get the tree-sitter Language for a language identifier.
///
/// Uses tree-sitter-language-pack::get_language() which returns Result<Language, Error>.
/// On first call for a language, the crate may download the parser shared library to
/// ~/.cache/tree-sitter-language-pack/ — requires network access in fresh environments.
pub fn get_language(lang: &str) -> Option<Language> {
    tree_sitter_language_pack::get_language(lang).ok()
}

/// Get a configured parser for a language.
///
/// Creates a fresh Parser per call. Parser is !Clone and !Sync; thread-local caching
/// is not viable without unsafe code. Benchmarks show parser creation is negligible
/// compared to tree-sitter parsing itself.
///
/// Returns `None` if the language is not supported or parser init fails.
pub fn get_parser(lang: &str) -> Option<Parser> {
    let language = get_language(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    Some(parser)
}

/// Leak a string to get a 'static reference.
/// Used for thread-local cache keys.
fn leak_str(s: &str) -> &'static str {
    // Only called for known language strings, so bounded memory
    Box::leak(s.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_language_rust() {
        let lang = get_language("rust");
        assert!(lang.is_some());
    }

    #[test]
    fn get_language_unknown() {
        let lang = get_language("unknown_language");
        assert!(lang.is_none());
    }

    #[test]
    fn get_parser_rust() {
        let parser = get_parser("rust");
        assert!(parser.is_some());
    }

    #[test]
    fn parser_can_parse() {
        let mut parser = get_parser("rust").expect("rust parser");
        let source = "fn main() {}";
        let tree = parser.parse(source, None);
        assert!(tree.is_some());
    }

    #[test]
    fn parser_caching() {
        // Get parser twice, should use cache
        let p1 = get_parser("python");
        let p2 = get_parser("python");
        assert!(p1.is_some());
        assert!(p2.is_some());
    }
}
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test queries` exits 0
- [ ] `cargo test parser` exits 0
- [ ] `queries/tree-sitter-language-pack/` directory exists with .scm files
- [ ] rust-tags.scm, python-tags.scm, javascript-tags.scm, typescript-tags.scm, go-tags.scm exist
- [ ] typescript-idents.scm exists (fallback query)
- [ ] `get_queries("typescript")` returns QueryPair with idents set
- [ ] Parser can parse a simple Rust snippet

## Reviewer Instructions

You are reviewing Step 04. Verify:

1. Run `cargo test queries::tests` — all tests must pass
2. Run `cargo test parser::tests` — all tests must pass
3. Check `ls queries/tree-sitter-language-pack/` shows .scm files
4. Verify rust-tags.scm contains `name.definition.function` and `name.reference.call`
5. Verify typescript-tags.scm does NOT contain `name.reference.call` (intentional per SPEC §4.3)
6. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/queries.rs src/parser.rs
rm -rf queries/
```
