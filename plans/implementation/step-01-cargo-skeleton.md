# Step 01: Cargo Skeleton

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 1 — Foundation. This step establishes the project structure and dependencies.

### This Step
Set up Cargo.toml with all required dependencies and create the module skeleton in lib.rs.

## Prerequisites
- None (first step)

## Files to Read Before Starting
- `Cargo.toml` — current state (likely minimal)
- `src/lib.rs` — current state (stub)
- `SPEC.md` §14, §17 — configuration and CLI requirements inform dependency choices

## Implementation

### Task 1: Update Cargo.toml

Replace the entire `[dependencies]` section (and add `[dev-dependencies]`) with:

```toml
[package]
name = "repo-mapper"
version = "0.1.0"
edition = "2021"
description = "Token-budget-respecting repository map generator"
license = "MIT"

[dependencies]
# Tree-sitter parsing
tree-sitter = "0.26"
tree-sitter-language-pack = "1.8"  # requires tree-sitter 0.26

# Compile-time maps for language detection
phf = { version = "0.11", features = ["macros"] }

# Graph algorithms
petgraph = "0.6"

# Serialization for caches
serde = { version = "1", features = ["derive"] }
bincode = "1"

# Persistent tag cache
rusqlite = { version = "0.32", features = ["bundled"] }

# Token counting
tiktoken-rs = "0.6"

# Path handling
pathdiff = "0.2"

# Logging/diagnostics
tracing = "0.1"

# CLI (only needed by binary, but simplest to include)
clap = { version = "4", features = ["derive"] }
ignore = "0.4"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
thiserror = "2"

[dev-dependencies]
tempfile = "3"

[[bin]]
name = "repo-mapper"
path = "src/bin/repo-mapper.rs"
```

### Task 2: Create module skeleton in src/lib.rs

Replace entire contents with:

```rust
//! repo-mapper: Token-budget-respecting repository map generator.
//!
//! This library produces a compact textual summary of a source code repository
//! by extracting tags with tree-sitter, building a weighted directed graph,
//! running PageRank, and rendering ranked definitions.

// Core types
mod tag;
mod path;

// Language support
mod lang;
mod queries;
mod parser;

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
mod tokens;
mod budget;

// Public API
mod config;
mod repo_map;
mod edge_cases;

// Re-exports
pub use tag::{Tag, TagKind};
pub use config::{RepoMapConfig, RefreshMode};
pub use repo_map::RepoMap;
```

### Task 3: Create stub modules

Create minimal stub files for each module declared above. Each stub should contain just enough to compile:

**src/tag.rs:**
```rust
//! Tag and TagKind types (SPEC §2.1).
```

**src/path.rs:**
```rust
//! Path utilities for relative path computation.
```

**src/lang.rs:**
```rust
//! Language detection from filenames (SPEC §3.1, §4).
```

**src/queries.rs:**
```rust
//! Query registry with bundled .scm files (SPEC §4.1).
```

**src/parser.rs:**
```rust
//! Parser registry for tree-sitter languages.
```

**src/extract.rs:**
```rust
//! Core tag extraction (SPEC §3).
```

**src/file.rs:**
```rust
//! File reading utilities (SPEC §13.5, §13.6).
```

**src/graph.rs:**
```rust
//! Graph construction (SPEC §6).
```

**src/weight.rs:**
```rust
//! Edge weight calculation (SPEC §6.4).
```

**src/important.rs:**
```rust
//! Important files list (SPEC §8).
```

**src/cache/mod.rs:**
```rust
//! Caching modules.

pub mod tag_cache;
pub mod map_cache;
```

**src/cache/tag_cache.rs:**
```rust
//! Persistent tag cache (SPEC §5).
```

**src/cache/map_cache.rs:**
```rust
//! In-memory map cache (SPEC §11).
```

**src/rank/mod.rs:**
```rust
//! PageRank ranking (SPEC §7).

mod personalization;
mod pagerank;
mod distribute;
```

**src/rank/personalization.rs:**
```rust
//! Personalization vector computation (SPEC §7.1).
```

**src/rank/pagerank.rs:**
```rust
//! PageRank algorithm (SPEC §7.2).
```

**src/rank/distribute.rs:**
```rust
//! Rank distribution to definitions (SPEC §7.4).
```

**src/render/mod.rs:**
```rust
//! Map rendering (SPEC §9).

mod tree_context;
mod tree_cache;
```

**src/render/tree_context.rs:**
```rust
//! TreeContext for scope-aware rendering (SPEC §9.2).
```

**src/render/tree_cache.rs:**
```rust
//! Render result caching (SPEC §9.2).
```

**src/tokens.rs:**
```rust
//! Token counting with tiktoken (SPEC §10.1).
```

**src/budget.rs:**
```rust
//! Binary search for token budget (SPEC §10.2).
```

**src/config.rs:**
```rust
//! Configuration types (SPEC §14).

/// Map cache refresh mode (SPEC §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefreshMode {
    Manual,
    Always,
    Files,
    #[default]
    Auto,
}

/// Configuration for RepoMap.
#[derive(Debug, Clone)]
pub struct RepoMapConfig {
    _placeholder: (),
}
```

**src/repo_map.rs:**
```rust
//! RepoMap public API.

/// Main entry point for repo map generation.
pub struct RepoMap {
    _placeholder: (),
}
```

**src/edge_cases.rs:**
```rust
//! Edge case handling (SPEC §13).
```

## Acceptance Criteria

- [ ] `cargo check` exits 0 (all modules resolve)
- [ ] `cargo build` exits 0 (dependencies compile)
- [ ] `cargo build --bin repo-mapper` exits 0
- [ ] All module files exist under `src/`
- [ ] `src/cache/` and `src/rank/` and `src/render/` directories exist with mod.rs

## Reviewer Instructions

You are reviewing Step 01. Verify:

1. Run `cargo check` — must exit 0 with no errors
2. Run `cargo build` — must exit 0 (first build may take time)
3. Check `ls src/` shows: tag.rs, path.rs, lang.rs, queries.rs, parser.rs, extract.rs, file.rs, graph.rs, weight.rs, important.rs, tokens.rs, budget.rs, config.rs, repo_map.rs, edge_cases.rs, lib.rs, cache/, rank/, render/, bin/
4. Check `ls src/cache/` shows: mod.rs, tag_cache.rs, map_cache.rs
5. Check `ls src/rank/` shows: mod.rs, personalization.rs, pagerank.rs, distribute.rs
6. Check `ls src/render/` shows: mod.rs, tree_context.rs, tree_cache.rs
7. Verify Cargo.toml has all dependencies listed in Task 1

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- Cargo.toml src/
```
