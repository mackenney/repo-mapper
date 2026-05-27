# repo-mapper

> **⚠ Experimental** — this library is under active development and not recommended for production use.

A Rust implementation of [aider's repo map](https://github.com/Aider-AI/aider/blob/main/aider/repomap.py) — a token-budget-respecting textual summary of a source code repository.

Given a set of "chat" files (the ones you're actively editing) and the rest of the repository, it uses tree-sitter tag extraction and Personalized PageRank to identify the most structurally relevant files and definitions, then renders them as a compact text map.

## Commands

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

## Usage

```
repo-mapper map    [OPTIONS] [REPO_PATH]   # aider-style ranked map
repo-mapper focus  [OPTIONS] <ANCHOR>...   # anchor-based dependency cone
```

Key flags (visible in `-h` for both commands):

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
| `--refresh <MODE>` | both | Cache mode: auto, always, manual, files (default: auto) |
| `--force-refresh` | both | Bypass cache and recompute |
| `--pagerank-damping` | both | PageRank damping factor (default: 0.85) |

See `SPEC.md` for the full behavioral specification.

## Building

```
cargo build --release   # optimized build (thin LTO, codegen-units=1, stripped)
cargo install --path .  # build release and install to ~/.cargo/bin
cargo nextest run       # run tests
```
