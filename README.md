# repo-mapper

> **⚠ Experimental** — this library is under active development and not recommended for production use.

A Rust implementation of [aider's repo map](https://github.com/Aider-AI/aider/blob/main/aider/repomap.py) — a token-budget-respecting textual summary of a source code repository.

Given a set of "chat" files (the ones you're actively editing) and the rest of the repository, it uses tree-sitter tag extraction and Personalized PageRank to identify the most structurally relevant files and definitions, then renders them as a compact text map.

## Extension: anchor mode

repo-mapper adds an **anchor** input not present in the original aider implementation.

Anchors designate one or more files or identifiers as the starting point for the map. The ranking algorithm becomes Random Walk with Restart (RWR) seeded at the anchor, which guarantees that even nodes with no incoming edges (Celery tasks, CLI handlers, HTTP handlers, plugin hooks) accumulate rank and appear in the map alongside their dependency cone.

```
repo-mapper -a process_job                # anchor by identifier
repo-mapper -a apps/tasks.py              # anchor by file
repo-mapper -a apps/tasks.py:process_job  # anchor by file + ident (scoped)
```

Anchor files are always included in the map output regardless of token budget. Without any `-a` flags the output is identical to the original aider behavior.

## Usage

```
repo-mapper [OPTIONS] [REPO_PATH]
```

Key flags:

| Flag | Short | Description |
|------|-------|-------------|
| `--max-tokens <N>` | `-t` | Token budget for the map (default: 1024) |
| `--anchor <VALUE>` | `-a` | Anchor file or identifier for RWR seeding (repeatable) |
| `--mention-file <PATH>` | `-m` | Boost a file's relevance without excluding it (repeatable) |
| `--mention-ident <NAME>` | `-i` | Boost an identifier's defining file (repeatable) |

See `SPEC.md` for the full behavioral specification.

## Building

```
cargo build --release
cargo nextest run
```
