//! Tag and TagKind types (SPEC §2.1).

/// Kind of a tag extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagKind {
    Definition,
    Reference,
}

/// A tag extracted from a source file.
#[derive(Debug, Clone)]
pub struct Tag {
    _placeholder: (),
}
