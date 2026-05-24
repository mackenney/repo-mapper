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

    pub const PYTHON_TAGS: &str =
        include_str!("../queries/tree-sitter-language-pack/python-tags.scm");

    pub const JAVASCRIPT_TAGS: &str =
        include_str!("../queries/tree-sitter-language-pack/javascript-tags.scm");

    pub const TYPESCRIPT_TAGS: &str =
        include_str!("../queries/tree-sitter-language-pack/typescript-tags.scm");
    pub const TYPESCRIPT_IDENTS: &str =
        include_str!("../queries/tree-sitter-language-pack/typescript-idents.scm");

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
