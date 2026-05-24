//! Tag and TagKind types (SPEC §2.1).

use std::fmt;

use serde::{Deserialize, Serialize};

/// The kind of tag: definition or reference (SPEC §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

impl std::str::FromStr for TagKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "def" => Ok(TagKind::Def),
            "ref" => Ok(TagKind::Ref),
            _ => Err(()),
        }
    }
}

/// A tag extracted from source code (SPEC §2.1).
///
/// Field order is significant for derived `PartialOrd`/`Ord`:
/// tags sort by (rel_fname, fname, line, name, kind).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
        assert_eq!("def".parse::<TagKind>(), Ok(TagKind::Def));
        assert_eq!("ref".parse::<TagKind>(), Ok(TagKind::Ref));
        assert!("other".parse::<TagKind>().is_err());
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
