//! repo-mapper CLI binary (SPEC §17).

use clap::{ArgAction, Parser};
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use repo_mapper::{RefreshMode, RepoMapConfig};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
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

    /// Anchor: file or identifier to use as RWR seed (always appears in map).
    /// If the value resolves to an existing file path, treated as anchor file;
    /// otherwise treated as an anchor identifier (looked up in tag index).
    #[arg(short = 'a', long = "anchor", action = ArgAction::Append)]
    anchors: Vec<String>,

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

    /// Show progress on stderr
    #[arg(short = 'p', long)]
    progress: bool,
}

fn main() {
    let cli = Cli::parse();
    setup_tracing(cli.verbose);

    let exit_code = run(&cli);
    std::process::exit(exit_code);
}

/// Spinner for progress reporting.
fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(msg.to_string());
    pb
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

    // Parse refresh mode — invalid value exits with code 1 per SPEC §17.6
    let refresh_mode = match cli.refresh.as_str() {
        "auto" => RefreshMode::Auto,
        "manual" => RefreshMode::Manual,
        "always" => RefreshMode::Always,
        "files" => RefreshMode::Files,
        other => {
            eprintln!(
                "error: invalid --refresh value '{other}'. Expected: auto, manual, always, files"
            );
            return 1;
        }
    };

    // Resolve -a/--anchor values into three buckets:
    //   file:ident  → anchor_scoped  (colon form, file part must exist)
    //   path/file   → anchor_fnames  (resolves to existing file)
    //   identifier  → anchor_idents  (anything else; tag-index lookup at runtime)
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut anchor_fnames: Vec<PathBuf> = Vec::new();
    let mut anchor_idents: HashSet<String> = HashSet::new();
    let mut anchor_scoped: Vec<(PathBuf, String)> = Vec::new();

    for val in &cli.anchors {
        // Try file:ident form first. Split on first colon.
        if let Some(colon) = val.find(':') {
            let file_part = &val[..colon];
            let ident_part = &val[colon + 1..];
            if !file_part.is_empty() && !ident_part.is_empty() {
                let resolved = resolve_path(file_part, &cwd, &root);
                if let Some(candidate) = resolved {
                    anchor_scoped.push((candidate, ident_part.to_string()));
                    continue;
                }
            }
        }
        // Try as a plain file path (CWD-relative, then root-relative).
        if let Some(candidate) = resolve_path(val, &cwd, &root) {
            anchor_fnames.push(candidate);
        } else {
            anchor_idents.insert(val.clone());
        }
    }

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
        .anchor_fnames(anchor_fnames)
        .anchor_idents(anchor_idents)
        .anchor_scoped(anchor_scoped)
        .build();

    // Enumerate files (SPEC §17.3)
    let other_fnames = if cli.progress {
        let pb = make_spinner("Scanning repository...");
        let files = enumerate_files(&root);
        pb.finish_and_clear();
        eprintln!("✓ Found {} files", files.len());
        files
    } else {
        enumerate_files(&root)
    };
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

    // Build mention sets — resolve paths relative to CWD then compute rel_fname against root
    let mentioned_fnames: HashSet<String> = cli
        .mention_files
        .iter()
        .map(|p| {
            let abs = if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(p))
                    .unwrap_or_else(|_| p.clone())
            };
            repo_mapper::path::rel_path(&abs, &root)
        })
        .collect();

    let mentioned_idents: HashSet<String> = cli.mention_idents.iter().cloned().collect();

    // Generate repo map
    let map_result = if cli.progress {
        let pb = make_spinner("Generating repo map...");
        let t0 = std::time::Instant::now();
        let r = repo_map.get_repo_map(
            &chat_fnames,
            &other_fnames,
            &mentioned_fnames,
            &mentioned_idents,
        );
        let elapsed = t0.elapsed();
        pb.finish_and_clear();
        match &r {
            Some(_) => eprintln!("✓ Done  ({:.1}s)", elapsed.as_secs_f64()),
            None => eprintln!("✗ No map generated  ({:.1}s)", elapsed.as_secs_f64()),
        }
        r
    } else {
        repo_map.get_repo_map(
            &chat_fnames,
            &other_fnames,
            &mentioned_fnames,
            &mentioned_idents,
        )
    };
    match map_result {
        Some(map) => {
            use std::io::Write;
            if let Err(e) = std::io::stdout().write_all(map.as_bytes()) {
                tracing::error!("Failed to write map to stdout: {}", e);
                return 1;
            }
            0
        }
        None => {
            tracing::warn!("No map generated (exit 2)");
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
        .hidden(false) // don't skip dotfiles; SPEC §8.1 important files include .gitignore, .env, .travis.yml, etc.
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

/// Resolve a path string to an existing absolute PathBuf.
/// Tries absolute, then CWD-relative, then root-relative.
/// Returns None if none of the candidates exist.
fn resolve_path(s: &str, cwd: &std::path::Path, root: &std::path::Path) -> Option<PathBuf> {
    if std::path::Path::new(s).is_absolute() {
        let p = PathBuf::from(s);
        return p.exists().then_some(p);
    }
    let cwd_rel = cwd.join(s);
    if cwd_rel.exists() {
        return Some(cwd_rel);
    }
    let root_rel = root.join(s);
    root_rel.exists().then_some(root_rel)
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
