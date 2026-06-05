//! Language detection from filenames (SPEC §3.1, §4).

use phf::phf_map;
use std::path::Path;

/// Map of file extensions to language identifiers.
///
/// Based on grep-ast's filename_to_lang and tree-sitter-language-pack support.
static EXTENSION_TO_LANG: phf::Map<&'static str, &'static str> = phf_map! {
    // Rust
    "rs" => "rust",

    // Python
    "py" => "python",
    "pyw" => "python",
    "pyi" => "python",

    // JavaScript/TypeScript
    "js" => "javascript",
    "mjs" => "javascript",
    "cjs" => "javascript",
    "jsx" => "javascript",
    "ts" => "typescript",
    "tsx" => "typescript",
    "mts" => "typescript",
    "cts" => "typescript",

    // Go
    "go" => "go",

    // C/C++
    "c" => "c",
    "h" => "c",
    "cc" => "cpp",
    "cpp" => "cpp",
    "cxx" => "cpp",
    "hpp" => "cpp",
    "hxx" => "cpp",
    "hh" => "cpp",

    // Java
    "java" => "java",

    // C#
    "cs" => "csharp",

    // Ruby
    "rb" => "ruby",
    "rake" => "ruby",
    "gemspec" => "ruby",

    // PHP
    "php" => "php",
    "php3" => "php",
    "php4" => "php",
    "php5" => "php",
    "phtml" => "php",

    // Swift
    "swift" => "swift",

    // Kotlin
    "kt" => "kotlin",
    "kts" => "kotlin",

    // Scala
    "scala" => "scala",
    "sc" => "scala",

    // Elixir
    "ex" => "elixir",
    "exs" => "elixir",

    // Lua
    "lua" => "lua",

    // R
    "r" => "r",
    "R" => "r",

    // Dart
    "dart" => "dart",

    // Solidity
    "sol" => "solidity",

    // Elm
    "elm" => "elm",

    // OCaml
    "ml" => "ocaml",
    "mli" => "ocaml_interface",

    // D
    "d" => "d",

    // Gleam
    "gleam" => "gleam",

    // HCL/Terraform
    "hcl" => "hcl",
    "tf" => "hcl",

    // Racket
    "rkt" => "racket",

    // Pony
    "pony" => "pony",

    // QL
    "ql" => "ql",

    // Common Lisp
    "lisp" => "commonlisp",
    "cl" => "commonlisp",

    // Emacs Lisp
    "el" => "elisp",

    // Arduino
    "ino" => "arduino",

    // Properties
    "properties" => "properties",
};

/// Map of full filenames to language identifiers.
static FILENAME_TO_LANG: phf::Map<&'static str, &'static str> = phf_map! {
    // Build files
    "Makefile" => "make",
    "makefile" => "make",
    "GNUmakefile" => "make",
    "Dockerfile" => "dockerfile",
    "Containerfile" => "dockerfile",

    // Ruby
    "Gemfile" => "ruby",
    "Rakefile" => "ruby",

    // Config files that might have language-specific queries
    ".gitignore" => "gitignore",
    ".dockerignore" => "gitignore",
};

/// Detect the language of a file from its path.
///
/// Returns `None` if the language is unrecognized (SPEC §3.1 step 2).
pub fn detect_language(path: &Path) -> Option<&'static str> {
    // First try full filename match
    if let Some(filename) = path.file_name().and_then(|s| s.to_str())
        && let Some(&lang) = FILENAME_TO_LANG.get(filename)
    {
        return Some(lang);
    }

    // Then try extension match
    if let Some(ext) = path.extension().and_then(|s| s.to_str())
        && let Some(&lang) = EXTENSION_TO_LANG.get(ext)
    {
        return Some(lang);
    }

    None
}

/// Check if a language identifier has query support.
///
/// This is a subset of detected languages — only those with bundled .scm files.
/// The actual check is done in the queries module; this is a fast pre-filter.
pub fn has_query_support(lang: &str) -> bool {
    matches!(
        lang,
        "rust"
            | "python"
            | "javascript"
            | "typescript"
            | "go"
            | "c"
            | "cpp"
            | "java"
            | "csharp"
            | "ruby"
            | "php"
            | "swift"
            | "kotlin"
            | "scala"
            | "elixir"
            | "lua"
            | "r"
            | "dart"
            | "solidity"
            | "elm"
            | "ocaml"
            | "ocaml_interface"
            | "d"
            | "gleam"
            | "hcl"
            | "racket"
            | "pony"
            | "ql"
            | "commonlisp"
            | "elisp"
            | "arduino"
            | "properties"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detect_by_extension() {
        assert_eq!(detect_language(Path::new("main.rs")), Some("rust"));
        assert_eq!(detect_language(Path::new("lib.py")), Some("python"));
        assert_eq!(detect_language(Path::new("app.ts")), Some("typescript"));
        assert_eq!(detect_language(Path::new("main.go")), Some("go"));
        assert_eq!(detect_language(Path::new("App.java")), Some("java"));
    }

    #[test]
    fn detect_by_full_name() {
        assert_eq!(detect_language(Path::new("Makefile")), Some("make"));
        assert_eq!(detect_language(Path::new("Dockerfile")), Some("dockerfile"));
        assert_eq!(detect_language(Path::new("Gemfile")), Some("ruby"));
    }

    #[test]
    fn detect_unknown_returns_none() {
        assert_eq!(detect_language(Path::new("data.csv")), None);
        assert_eq!(detect_language(Path::new("image.png")), None);
        assert_eq!(detect_language(Path::new("README")), None);
    }

    #[test]
    fn detect_with_path() {
        assert_eq!(detect_language(Path::new("src/lib/main.rs")), Some("rust"));
        assert_eq!(
            detect_language(Path::new("/absolute/path/to/file.py")),
            Some("python")
        );
    }

    #[test]
    fn has_query_support_basic() {
        assert!(has_query_support("rust"));
        assert!(has_query_support("python"));
        assert!(has_query_support("typescript"));
        // make and dockerfile have detection but may not have queries
        assert!(!has_query_support("make"));
        assert!(!has_query_support("dockerfile"));
    }
}
