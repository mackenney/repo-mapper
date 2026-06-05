# repo-mapper

A Rust implementation of [aider's repo map](https://github.com/Aider-AI/aider/blob/main/aider/repomap.py) — a token-budget-respecting textual summary of a source code repository.

Given a set of "chat" files (the ones you're actively editing) and the rest of the repository, repo-mapper uses tree-sitter tag extraction and Personalized PageRank to identify the most structurally relevant files and definitions, then renders them as a compact text map sized to fit within a token budget.

## Installation

```
cargo install repo-mapper
```

Or build from source:

```
cargo build --release
cargo install --path .
```

## CLI

repo-mapper has two subcommands with distinct mental models.

### `map` — aider-style ranked map

Ranks the entire repository by structural relevance to the files you are actively editing:

```
repo-mapper map [REPO_PATH] [-t <tokens>] [-c <chat-file>] [-v] [-p]
```

Run `repo-mapper map --help` for the full option list (chat context, mention boosts, PageRank tuning, cache control).

### `focus` — anchor-based dependency cone

Reseeds PageRank from one or more anchors (Random Walk with Restart) so the anchor and its entire dependency cone rank first. Anchor files are always included regardless of token budget — useful for entrypoints with no callers (tasks, handlers, hooks, CLI commands):

```
repo-mapper focus process_job                # anchor by identifier
repo-mapper focus apps/tasks.py              # anchor by file
repo-mapper focus apps/tasks.py:process_job  # anchor by file + ident (scoped)
repo-mapper focus src/lib.rs process_job     # multiple anchors
```

Run `repo-mapper focus --help` for advanced options.

Without anchors, `map` produces output identical to the original aider behavior.

### Key flags

| Flag | Short | Description |
|------|-------|-------------|
| `--max-tokens <N>` | `-t` | Token budget for the map (default: 1024) |
| `--verbose` | `-v` | Debug diagnostics to stderr |
| `--progress` | `-p` | Spinner and elapsed time on stderr |

Advanced flags (visible in `--help` only):

| Flag | Command | Description |
|------|---------|-------------|
| `--chat-file <PATH>` | `map` | File being edited — excluded from map, seeds ranking (repeatable) |
| `--mention-file <PATH>` | `map` | Boost a file's relevance without excluding it (repeatable) |
| `--mention-ident <NAME>` | `map` | Boost an identifier's defining file (repeatable) |
| `--refresh <MODE>` | both | Cache mode: `auto`, `always`, `manual`, `files` (default: `auto`) |
| `--force-refresh` | both | Bypass cache and recompute |
| `--pagerank-damping` | both | PageRank damping factor (default: 0.85) |

## Library

Add to `Cargo.toml`:

```toml
[dependencies]
repo-mapper = { version = "0.0.1", default-features = false }
```

The `cli` feature (on by default) pulls in `clap`, `ignore`, `indicatif`, and `tracing-subscriber` for the binary. Library consumers should disable it.

Basic usage:

```rust
use repo_mapper::RepoMapConfig;
use std::path::PathBuf;

let root = PathBuf::from("/path/to/repo");
let files: Vec<PathBuf> = std::fs::read_dir(&root)
    .unwrap()
    .filter_map(|e| e.ok().map(|e| e.path()))
    .filter(|p| p.extension().map_or(false, |e| e == "rs"))
    .collect();

let mut repo_map = RepoMapConfig::builder()
    .root(&root)
    .map_tokens(1024)
    .build();

if let Some(map) = repo_map.get_repo_map(&[], &files, &Default::default(), &Default::default()) {
    print!("{map}");
}
```

See the [API docs](https://docs.rs/repo-mapper) for the full interface.

## How it works

1. **Tag extraction** — tree-sitter parses each source file and extracts definition and reference tags (functions, classes, methods, etc.)
2. **Graph construction** — a weighted directed graph is built where nodes are files and edges represent definition/reference relationships
3. **PageRank** — Personalized PageRank seeds relevance from chat/anchor files; files that define symbols referenced by the chat files rank highest
4. **Budget rendering** — definitions are rendered in rank order; a binary search finds the largest set that fits within the token budget

## License

Apache-2.0. The ranking algorithm is derived from [aider](https://github.com/Aider-AI/aider) (Apache-2.0).
