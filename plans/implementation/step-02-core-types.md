# Step 02: Core Types

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 1 — Foundation. This step defines the fundamental data types.

### This Step
Implement `Tag`, `TagKind`, and path utilities per SPEC §2.

## Prerequisites
- Step 01 complete (Cargo.toml and module skeleton)

## Files to Read Before Starting
- `SPEC.md` §2.1, §2.2, §3.4 — Tag definition, path representations, line indexing
- `src/tag.rs` — current stub
- `src/path.rs` — current stub

## Implementation

### Task 1: Implement TagKind enum in src/tag.rs

```rust
//! Tag and TagKind types (SPEC §2.1).

use std::fmt;

/// The kind of tag: definition or reference (SPEC §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TagKind {
    /// A definition (function, class, etc.)
    Def,
    /// A reference to an identifier
    Ref,
}

impl fmt::Display for TagKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagKind::Def => write!(f, "def"),
            TagKind::Ref => write!(f, "ref"),
        }
    }
}

impl TagKind {
    /// Create from string, matching SPEC §2.1 values.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "def" => Some(TagKind::Def),
            "ref" => Some(TagKind::Ref),
            _ => None,
        }
    }
}
```

### Task 2: Implement Tag struct in src/tag.rs

Append to `src/tag.rs`:

```rust
/// A tag extracted from source code (SPEC §2.1).
///
/// Field order is significant for derived `PartialOrd`/`Ord`:
/// tags sort by (rel_fname, fname, line, name, kind).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag {
    /// Path relative to repository root (SPEC §2.2)
    pub rel_fname: String,
    /// Absolute path to file
    pub fname: String,
    /// 1-indexed line number, or -1 for fallback refs (SPEC §3.4)
    pub line: i32,
    /// The identifier text
    pub name: String,
    /// Definition or reference
    pub kind: TagKind,
}

impl Tag {
    /// Create a new tag.
    pub fn new(
        rel_fname: impl Into<String>,
        fname: impl Into<String>,
        line: i32,
        name: impl Into<String>,
        kind: TagKind,
    ) -> Self {
        Self {
            rel_fname: rel_fname.into(),
            fname: fname.into(),
            line,
            name: name.into(),
            kind,
        }
    }

    /// Create a definition tag.
    pub fn def(
        rel_fname: impl Into<String>,
        fname: impl Into<String>,
        line: i32,
        name: impl Into<String>,
    ) -> Self {
        Self::new(rel_fname, fname, line, name, TagKind::Def)
    }

    /// Create a reference tag.
    pub fn reference(
        rel_fname: impl Into<String>,
        fname: impl Into<String>,
        line: i32,
        name: impl Into<String>,
    ) -> Self {
        Self::new(rel_fname, fname, line, name, TagKind::Ref)
    }

    /// Check if this is a definition tag.
    pub fn is_def(&self) -> bool {
        self.kind == TagKind::Def
    }

    /// Check if this is a reference tag.
    pub fn is_ref(&self) -> bool {
        self.kind == TagKind::Ref
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.rel_fname, self.line, self.name, self.kind
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_kind_display() {
        assert_eq!(TagKind::Def.to_string(), "def");
        assert_eq!(TagKind::Ref.to_string(), "ref");
    }

    #[test]
    fn tag_kind_from_str() {
        assert_eq!(TagKind::from_str("def"), Some(TagKind::Def));
        assert_eq!(TagKind::from_str("ref"), Some(TagKind::Ref));
        assert_eq!(TagKind::from_str("other"), None);
    }

    #[test]
    fn tag_construction() {
        let tag = Tag::def("src/main.rs", "/home/user/proj/src/main.rs", 10, "main");
        assert_eq!(tag.rel_fname, "src/main.rs");
        assert_eq!(tag.fname, "/home/user/proj/src/main.rs");
        assert_eq!(tag.line, 10);
        assert_eq!(tag.name, "main");
        assert!(tag.is_def());
        assert!(!tag.is_ref());
    }

    #[test]
    fn tag_ordering() {
        // Tags sort by (rel_fname, fname, line, name, kind)
        let t1 = Tag::def("a.rs", "/a.rs", 1, "foo");
        let t2 = Tag::def("a.rs", "/a.rs", 2, "foo");
        let t3 = Tag::def("b.rs", "/b.rs", 1, "foo");

        assert!(t1 < t2); // same file, different line
        assert!(t2 < t3); // different file
    }

    #[test]
    fn tag_fallback_line() {
        // SPEC §3.2: fallback refs use line = -1
        let tag = Tag::reference("a.rs", "/a.rs", -1, "ident");
        assert_eq!(tag.line, -1);
    }
}
```

### Task 3: Implement path utilities in src/path.rs

```rust
//! Path utilities for relative path computation (SPEC §2.2).

use std::path::{Path, PathBuf};

/// Compute the relative path from `root` to `fname`.
///
/// Falls back to the absolute path if relative computation fails
/// (e.g., Windows cross-drive paths per SPEC §2.2).
pub fn rel_path(fname: &Path, root: &Path) -> String {
    pathdiff::diff_paths(fname, root)
        .map(|p| normalize_slashes(&p))
        .unwrap_or_else(|| fname.to_string_lossy().into_owned())
}

/// Normalize path separators to forward slashes.
///
/// Ensures consistent path representation across platforms.
fn normalize_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Canonicalize a path, resolving symlinks and relative components.
///
/// Returns the path unchanged if canonicalization fails.
pub fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Extract the file stem (basename without extension).
pub fn file_stem(path: &Path) -> Option<&str> {
    path.file_stem().and_then(|s| s.to_str())
}

/// Extract the file name (basename with extension).
pub fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|s| s.to_str())
}

/// Extract the file extension.
pub fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|s| s.to_str())
}

/// Get all path components as strings (for mentioned_idents matching in §7.1).
pub fn path_components(rel_fname: &str) -> impl Iterator<Item = &str> {
    Path::new(rel_fname)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rel_path_basic() {
        let root = Path::new("/home/user/project");
        let fname = Path::new("/home/user/project/src/main.rs");
        assert_eq!(rel_path(fname, root), "src/main.rs");
    }

    #[test]
    fn rel_path_same_dir() {
        let root = Path::new("/home/user/project");
        let fname = Path::new("/home/user/project/file.rs");
        assert_eq!(rel_path(fname, root), "file.rs");
    }

    #[test]
    fn rel_path_fallback_to_absolute() {
        // When diff_paths returns None, fall back to absolute
        // This happens on Windows with cross-drive paths
        // We can't easily test this on Unix, but verify the function runs
        let root = Path::new("/root");
        let fname = Path::new("/other/path/file.rs");
        let result = rel_path(fname, root);
        // On Unix this will be a relative path like "../other/path/file.rs"
        // or fall back to absolute; either is acceptable
        assert!(!result.is_empty());
    }

    #[test]
    fn path_components_basic() {
        let components: Vec<_> = path_components("src/lib/mod.rs").collect();
        assert_eq!(components, vec!["src", "lib", "mod.rs"]);
    }

    #[test]
    fn file_stem_basic() {
        assert_eq!(file_stem(Path::new("main.rs")), Some("main"));
        assert_eq!(file_stem(Path::new("lib.tar.gz")), Some("lib.tar"));
    }

    #[test]
    fn extension_basic() {
        assert_eq!(extension(Path::new("main.rs")), Some("rs"));
        assert_eq!(extension(Path::new("Makefile")), None);
    }
}
```

### Task 4: Add serde derives to Tag for cache serialization

Update `src/tag.rs` to add serde derives. At the top of the file, add:

```rust
use serde::{Deserialize, Serialize};
```

Update `TagKind` derive:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
```

Update `Tag` derive:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo test tag` exits 0 (all tag tests pass)
- [ ] `cargo test path` exits 0 (all path tests pass)
- [ ] `Tag` struct has all 5 fields per SPEC §2.1
- [ ] `Tag` derives `Serialize, Deserialize` for cache compatibility
- [ ] `rel_path` falls back to absolute path on failure (SPEC §2.2)

## Reviewer Instructions

You are reviewing Step 02. Verify:

1. Run `cargo test tag::tests` — all tests must pass
2. Run `cargo test path::tests` — all tests must pass
3. Check `src/tag.rs` contains: `TagKind` enum with `Def`/`Ref`, `Tag` struct with 5 fields in correct order
4. Verify `Tag` has `#[derive(..., Serialize, Deserialize)]`
5. Check `src/path.rs` contains: `rel_path`, `normalize_path`, `path_components` functions
6. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/tag.rs src/path.rs
```
