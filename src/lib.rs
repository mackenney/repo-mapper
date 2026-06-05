#![warn(missing_docs)]
//! Token-budget-respecting repository map generator.
//!
//! Produces a compact textual summary of a source code repository by:
//! - Extracting symbol tags (definitions and references) with tree-sitter
//! - Building a weighted directed file-dependency graph
//! - Running Personalized PageRank to rank files by structural relevance
//! - Rendering ranked definitions within a token budget
//!
//! # Quick start
//!
//! ```no_run
//! use repo_mapper::RepoMapConfig;
//!
//! let mut repo_map = RepoMapConfig::builder()
//!     .root("/path/to/repo")
//!     .map_tokens(1024)
//!     .build();
//!
//! let files = vec![std::path::PathBuf::from("/path/to/repo/src/main.rs")];
//! let map = repo_map.get_repo_map(&[], &files, &Default::default(), &Default::default());
//! if let Some(text) = map {
//!     print!("{text}");
//! }
//! ```

pub mod path;
mod tag;

pub mod lang;
pub mod parser;
pub mod queries;

pub mod extract;
pub mod file;

pub mod graph;
pub mod weight;

pub mod important;

pub mod cache;

pub mod rank;

pub mod render;

pub mod budget;
pub mod tokens;

pub mod config;
pub mod edge_cases;
pub mod repo_map;

pub use config::{RefreshMode, RepoMapConfig, RepoMapConfigBuilder};
pub use repo_map::RepoMap;
pub use tag::{Tag, TagKind};
