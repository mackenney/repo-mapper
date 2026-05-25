//! repo-mapper CLI binary (SPEC §17).

use clap::{ArgAction, Parser};
use ignore::WalkBuilder;
use repo_mapper::{RefreshMode, RepoMapConfig};
use std::collections::HashSet;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, fmt};

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

/// Main execution logic. Returns exit code.
/// Exit codes (SPEC §17.6):
///   0 = success, map written to stdout
///   1 = fatal error (I/O, invalid config, unrecoverable failure)
///   2 = no map generated (budget, no files, all excluded)
fn run(cli: &Cli) -> i32 {
    // Detect repository root (SPEC §17.2)
    let root = detect_root(cli);
    tracing::debug!("Repository root: {}", root.display());

    // Parse refresh mode (invalid value falls back to auto)
    let refresh_mode = match cli.refresh.as_str() {
        "manual" => RefreshMode::Manual,
        "always" => RefreshMode::Always,
        "files" => RefreshMode::Files,
        _ => RefreshMode::Auto,
    };

    // Build RepoMap
    let mut repo_map = RepoMapConfig::builder()
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
        .build();

    // Enumerate files (SPEC §17.3)
    let other_fnames = enumerate_files(&root);
    if other_fnames.is_empty() {
        tracing::warn!("No files found in repository");
        return 2;
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
    match repo_map.get_repo_map(
        &chat_fnames,
        &other_fnames,
        &mentioned_fnames,
        &mentioned_idents,
    ) {
        Some(map) => {
            use std::io::Write;
            if let Err(e) = std::io::stdout().write_all(map.as_bytes()) {
                tracing::error!("Failed to write map to stdout: {}", e);
                return 1;
            }
            0
        }
        None => {
            tracing::info!("No map generated");
            2
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

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .follow_links(false)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();

        let target = if path.is_symlink() {
            match std::fs::metadata(path) {
                Ok(meta) if meta.is_file() => path.to_path_buf(),
                _ => continue,
            }
        } else if path.is_file() {
            path.to_path_buf()
        } else {
            continue;
        };

        files.push(target);
    }

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
