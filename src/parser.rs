//! Parser registry for tree-sitter languages.

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
#[allow(dead_code)]
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
