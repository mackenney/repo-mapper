//! repo-mapper: Token-budget-respecting repository map generator.
//!
//! This library produces a compact textual summary of a source code repository
//! by extracting tags with tree-sitter, building a weighted directed graph,
//! running PageRank, and rendering ranked definitions.

// Core types
pub mod path;
mod tag;

// Language support
mod lang;
mod parser;
mod queries;

// Tag extraction
mod extract;
mod file;

// Graph construction
mod graph;
mod weight;

// Important files
mod important;

// Caching
mod cache;

// Ranking
mod rank;

// Rendering
mod render;

// Token budget
mod budget;
mod tokens;

// Public API
mod config;
mod edge_cases;
mod repo_map;

// Re-exports
pub use config::{RefreshMode, RepoMapConfig};
pub use repo_map::RepoMap;
pub use tag::{Tag, TagKind};
