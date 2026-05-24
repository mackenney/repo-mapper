# Step 15: CLI

## Context

### Overall Objective
Build repo-mapper, a Rust library and CLI for token-budget-respecting repository maps.

### Phase Context
Wave 10 — CLI. Final step.

### This Step
Implement CLI binary with clap, file enumeration, and exit codes per SPEC §17.

## Prerequisites
- Step 14 complete (public API)

## Files to Read Before Starting
- `SPEC.md` §17 — CLI specification
- `src/bin/repo-mapper.rs` — current stub

## Implementation

### Task 1: Implement CLI in src/bin/repo-mapper.rs

```rust
//! repo-mapper CLI binary (SPEC §17).

use clap::{ArgAction, Parser};
use ignore::WalkBuilder;
use repo_mapper::{RefreshMode, RepoMap, RepoMapConfig};
use std::collections::HashSet;
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

/// Generate a token-budget-respecting repository map.
#[derive(Parser, Debug)]
#[command(name = "repo-mapper", about = "Generate a repo map")]
struct Cli {
    /// Repository root path
    #[arg(value_name = "REPO_PATH")]
    repo_path: Option<PathBuf>,

    /// Repository root (explicit override)
    #[arg(long)]
    root: Option<PathBuf>,

    /// Maximum tokens in output
    #[arg(short = 't', long = "max-tokens", default_value = "1024")]
    max_tokens: usize,

    /// LLM context window size
    #[arg(long)]
    max_context_window: Option<usize>,

    /// Chat file (active editing)
    #[arg(short = 'c', long = "chat-file", action = ArgAction::Append)]
    chat_files: Vec<PathBuf>,

    /// Mentioned file
    #[arg(short = 'm', long = "mention-file", action = ArgAction::Append)]
    mention_files: Vec<PathBuf>,

    /// Mentioned identifier
    #[arg(short = 'i', long = "mention-ident", action = ArgAction::Append)]
    mention_idents: Vec<String>,

    /// Cache refresh mode
    #[arg(long, default_value = "auto")]
    refresh: String,

    /// Force cache recomputation
    #[arg(long)]
    force_refresh: bool,

    /// Exclude unranked files
    #[arg(long)]
    exclude_unranked: bool,

    /// Maximum line length
    #[arg(long, default_value = "100")]
    max_line_length: usize,

    /// PageRank damping factor
    #[arg(long, default_value = "0.85")]
    pagerank_damping: f64,

    /// PageRank convergence tolerance
    #[arg(long, default_value = "1e-6")]
    pagerank_tol: f64,

    /// PageRank maximum iterations
    #[arg(long, default_value = "100")]
    pagerank_max_iter: usize,

    /// Verbose output
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();
    setup_tracing(cli.verbose);

    let exit_code = run(&cli);
    std::process::exit(exit_code);
}

/// Set up tracing subscriber.
fn setup_tracing(verbose: bool) {
    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Main execution logic.
/// Main execution logic. Returns exit code.
/// Exit codes (SPEC §17.6):
///   0 = success, map written to stdout
///   1 = fatal error (I/O, invalid config, unrecoverable failure)
///   2 = no map generated (budget, no files, all excluded)
fn run(cli: &Cli) -> i32 {
    // Detect repository root (SPEC §17.2)
    let root = detect_root(cli);
    tracing::debug!("Repository root: {}", root.display());

    // Parse refresh mode (invalid value falls back to auto, not exit 1)
    let refresh_mode = match cli.refresh.as_str() {
        "manual" => RefreshMode::Manual,
        "always" => RefreshMode::Always,
        "files" => RefreshMode::Files,
        "auto" | _ => RefreshMode::Auto,
    };

    // Build configuration
    let config = RepoMapConfig::builder()
        .root(root.clone())
        .map_tokens(cli.max_tokens)
        .max_context_window(cli.max_context_window)
        .refresh(refresh_mode)
        .force_refresh(cli.force_refresh)
        .exclude_unranked(cli.exclude_unranked)
        .max_line_length(cli.max_line_length)
        .pagerank_damping(cli.pagerank_damping)
        .pagerank_tol(cli.pagerank_tol)
        .pagerank_max_iter(cli.pagerank_max_iter)
        .verbose(cli.verbose)
        .build()
        .unwrap_or_else(|e| {
            tracing::error!("Configuration error: {}", e);
            std::process::exit(1); // exit code 1: fatal configuration error
        });

    // Enumerate files (SPEC §17.3)
    let other_fnames = match std::panic::catch_unwind(|| enumerate_files(&root)) {
        Ok(files) => files,
        Err(_) => {
            tracing::error!("Fatal error enumerating repository files");
            return 1;
        }
    };
    if other_fnames.is_empty() {
        tracing::warn!("No files found in repository");
        return 2; // exit code 2: no map generated
    }
    tracing::debug!("Found {} files", other_fnames.len());

    // Resolve chat files relative to CWD
    let chat_fnames: Vec<PathBuf> = cli
        .chat_files
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(p))
                    .unwrap_or_else(|_| p.clone())
            }
        })
        .collect();
    // Warn about chat files that don't exist (they'll be skipped by the library)
    for p in &chat_fnames {
        if !p.exists() {
            tracing::warn!("Chat file not found: {}", p.display());
        }
    }

    // Build mention sets
    let mentioned_fnames: HashSet<String> = cli
        .mention_files
        .iter()
        .filter_map(|p| p.to_str().map(String::from))
        .collect();

    let mentioned_idents: HashSet<String> = cli.mention_idents.iter().cloned().collect();

    // Generate repo map
    let mut repo_map = config;
    match repo_map.get_repo_map(&chat_fnames, &other_fnames, &mentioned_fnames, &mentioned_idents) {
        Some(map) => {
            if let Err(e) = std::io::Write::write_all(&mut std::io::stdout(), map.as_bytes()) {
                tracing::error!("Failed to write map to stdout: {}", e);
                return 1; // exit code 1: I/O error writing output
            }
            0 // exit code 0: success
        }
        None => {
            tracing::info!("No map generated");
            2 // exit code 2: no map generated
        }
    }
}

/// Detect repository root (§17.2).
fn detect_root(cli: &Cli) -> PathBuf {
    // Priority 1: --root flag
    if let Some(ref root) = cli.root {
        return root.clone();
    }

    // Priority 2: positional argument
    if let Some(ref path) = cli.repo_path {
        return path.clone();
    }

    // Priority 3: Walk up looking for .git
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut dir = cwd.as_path();
    loop {
        if dir.join(".git").is_dir() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    // Priority 4: CWD fallback
    cwd
}

/// Enumerate files in repository (SPEC §17.3).
fn enumerate_files(root: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // hidden(true) tells ignore to SKIP hidden entries entirely.
    // This prevents descent into hidden directories (.git, .venv, etc.).
    // We then re-enable hidden FILES selectively via filter_entry, but since
    // we only collect is_file() entries the effect is: hidden dirs skipped,
    // hidden files at non-hidden paths are collected if gitignore permits.
    //
    // NOTE: Using hidden(false) + manual continue does NOT prevent the walker
    // from descending into hidden directories; filter_entry is the correct API.
    let walker = WalkBuilder::new(root)
        .hidden(true)         // skip hidden directories (names starting with '.')
        .git_ignore(true)     // respect .gitignore
        .git_global(true)     // respect global gitignore
        .git_exclude(true)    // respect .git/info/exclude
        .follow_links(false)  // do NOT follow symlinks to directories (SPEC §17.3 step 3)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();

        // Follow symlinks to regular files, skip symlinks to directories.
        // With follow_links(false), symlinks appear as entries; resolve them.
        let target = if path.is_symlink() {
            match std::fs::metadata(path) {
                Ok(meta) if meta.is_file() => path.to_path_buf(),
                _ => continue, // symlink to dir or broken symlink: skip
            }
        } else if path.is_file() {
            path.to_path_buf()
        } else {
            continue;
        };

        files.push(target);
    }

    // Sort lexicographically (SPEC §2.2)
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cli_defaults() {
        let cli = Cli::parse_from(["repo-mapper"]);
        assert_eq!(cli.max_tokens, 1024);
        assert_eq!(cli.refresh, "auto");
        assert!(!cli.verbose);
    }

    #[test]
    fn parse_cli_with_args() {
        let cli = Cli::parse_from([
            "repo-mapper",
            "-t",
            "2048",
            "--verbose",
            "-c",
            "main.rs",
            "-m",
            "lib.rs",
            "-i",
            "foo",
            ".",
        ]);
        assert_eq!(cli.max_tokens, 2048);
        assert!(cli.verbose);
        assert_eq!(cli.chat_files.len(), 1);
        assert_eq!(cli.mention_files.len(), 1);
        assert_eq!(cli.mention_idents.len(), 1);
        assert!(cli.repo_path.is_some());
    }

    #[test]
    fn parse_refresh_mode() {
        let cli = Cli::parse_from(["repo-mapper", "--refresh", "always"]);
        assert_eq!(cli.refresh, "always");
    }
}
```

## Acceptance Criteria

- [ ] `cargo check` exits 0
- [ ] `cargo build --bin repo-mapper` exits 0
- [ ] `./target/debug/repo-mapper --help` shows usage
- [ ] Root detection follows §17.2 priority order
- [ ] File enumeration respects .gitignore (§17.3)
- [ ] Exit code 0 on success, 2 on no map generated, 1 on fatal I/O/config error
- [ ] `--verbose` enables debug logging to stderr
- [ ] `RUST_LOG` environment variable overrides log level
- [ ] All flags from SPEC §17.4 are implemented

## Reviewer Instructions

You are reviewing Step 15. Verify:

1. Run `cargo build --bin repo-mapper` — must exit 0
2. Run `./target/debug/repo-mapper --help` — must show all flags
3. Test in a git repo: `./target/debug/repo-mapper` should produce output
4. Check exit codes: success = 0, no map = 2
5. Verify `-v` enables debug output to stderr
6. Run `cargo test --bin repo-mapper` — all tests must pass
7. Run `cargo clippy -- -D warnings` — must exit 0

Report: PASS with each criterion confirmed, or FAIL: <criterion> — <what's wrong>

## Rollback

```bash
git checkout HEAD -- src/bin/repo-mapper.rs
```
