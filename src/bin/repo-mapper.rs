//! repo-mapper CLI binary (SPEC §17).

use clap::{ArgAction, Parser, Subcommand};
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use repo_mapper::{RefreshMode, RepoMapConfig};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(
    name = "repo-mapper",
    about = "Ranked structural summary of a codebase",
    long_about = "Generate a token-budget-respecting map of a repository's structure.\n\n\
repo-mapper parses source files with tree-sitter, builds a weighted file-dependency graph,\n\
and runs Personalized PageRank to surface the most relevant definitions.\n\n\
Output is a compact text listing of file paths and key symbols, sized to fit within a\n\
token budget — suitable for pasting into an LLM context window."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Aider-style ranked map driven by chat context
    ///
    /// Parses every source file with tree-sitter, builds a weighted file-dependency graph,
    /// ranks files with Personalized PageRank, and renders a structured text map within
    /// the token budget. Chat files (-c) are excluded from the map but seed the ranking.
    #[command(
        after_help = "Run `repo-mapper map --help` for advanced options (chat context, mention boosts, PageRank tuning, cache control).",
        after_long_help = ""
    )]
    Map {
        /// Repository root (auto-detected from .git if omitted)
        #[arg(value_name = "REPO_PATH")]
        repo_path: Option<PathBuf>,

        /// Override repository root (highest priority; overrides REPO_PATH and .git detection)
        #[arg(long, hide = true)]
        root: Option<PathBuf>,

        /// Token budget for the map output
        #[arg(short = 't', long, default_value = "1024")]
        max_tokens: usize,

        /// LLM context window size; when set with no open chat files the token budget is multiplied by 8
        #[arg(long, hide = true)]
        max_context_window: Option<usize>,

        /// File you are actively editing — excluded from the map; its references seed the ranking (repeatable)
        #[arg(short = 'c', long = "chat-file", value_name = "PATH", action = ArgAction::Append, hide = true)]
        chat_files: Vec<PathBuf>,

        /// Boost a file's relevance without excluding it from the map (repeatable)
        #[arg(short = 'm', long = "mention-file", value_name = "PATH", action = ArgAction::Append, hide = true)]
        mention_files: Vec<PathBuf>,

        /// Boost an identifier's defining file by increasing edge weights to it (repeatable)
        #[arg(short = 'i', long = "mention-ident", value_name = "NAME", action = ArgAction::Append, hide = true)]
        mention_idents: Vec<String>,

        /// Map cache refresh mode: auto, always, manual, files
        #[arg(long, default_value = "auto", value_name = "MODE", hide = true)]
        refresh: String,

        /// Bypass the cache and force map recomputation
        #[arg(long, hide = true)]
        force_refresh: bool,

        /// Omit files whose PageRank score is ≤ 0.0001
        #[arg(long, hide = true)]
        exclude_unranked: bool,

        /// Truncate output lines longer than this many characters
        #[arg(long, default_value = "100", hide = true)]
        max_line_length: usize,

        /// PageRank damping factor
        #[arg(long, default_value = "0.85", hide = true)]
        pagerank_damping: f64,

        /// PageRank convergence tolerance
        #[arg(long, default_value = "1e-6", hide = true)]
        pagerank_tol: f64,

        /// PageRank maximum iterations before forced stop
        #[arg(long, default_value = "100", hide = true)]
        pagerank_max_iter: usize,

        /// Emit debug diagnostics to stderr
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Show a progress spinner and elapsed time on stderr
        #[arg(short = 'p', long)]
        progress: bool,
    },

    /// Focused map anchored on specific files or identifiers
    ///
    /// Reseeds PageRank from the given anchors (Random Walk with Restart) so the anchor
    /// and its entire dependency cone rank first. Anchors are always included regardless
    /// of token budget — useful for entrypoints with zero callers (tasks, handlers, hooks).
    ///
    /// Anchor formats:
    ///   src/jobs.rs          — anchor a file
    ///   process_job          — anchor an identifier (resolved via tag index)
    ///   src/jobs.rs:process_job — anchor a file and boost a specific identifier within it
    #[command(
        after_help = "Run `repo-mapper focus --help` for advanced options (cache control, output tuning, PageRank parameters).",
        after_long_help = ""
    )]
    Focus {
        /// Files, identifiers, or file:ident pairs to anchor on
        #[arg(required = true, value_name = "ANCHOR")]
        anchors: Vec<String>,

        /// Repository root (auto-detected from .git if omitted)
        #[arg(long, hide = true)]
        root: Option<PathBuf>,

        /// Token budget for the map output
        #[arg(short = 't', long, default_value = "1024")]
        max_tokens: usize,

        /// LLM context window size; when set with no open chat files the token budget is multiplied by 8
        #[arg(long, hide = true)]
        max_context_window: Option<usize>,

        /// Map cache refresh mode: auto, always, manual, files
        #[arg(long, default_value = "auto", value_name = "MODE", hide = true)]
        refresh: String,

        /// Bypass the cache and force map recomputation
        #[arg(long, hide = true)]
        force_refresh: bool,

        /// Omit files whose PageRank score is ≤ 0.0001
        #[arg(long, hide = true)]
        exclude_unranked: bool,

        /// Truncate output lines longer than this many characters
        #[arg(long, default_value = "100", hide = true)]
        max_line_length: usize,

        /// PageRank damping factor
        #[arg(long, default_value = "0.85", hide = true)]
        pagerank_damping: f64,

        /// PageRank convergence tolerance
        #[arg(long, default_value = "1e-6", hide = true)]
        pagerank_tol: f64,

        /// PageRank maximum iterations before forced stop
        #[arg(long, default_value = "100", hide = true)]
        pagerank_max_iter: usize,

        /// Emit debug diagnostics to stderr
        #[arg(short = 'v', long)]
        verbose: bool,

        /// Show a progress spinner and elapsed time on stderr
        #[arg(short = 'p', long)]
        progress: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        eprintln!("error: a subcommand is required\n");
        eprintln!("  repo-mapper map    aider-style ranked map driven by chat context");
        eprintln!("  repo-mapper focus  focused map anchored on specific files or identifiers");
        eprintln!("\nRun `repo-mapper --help` for usage.");
        std::process::exit(1);
    };

    let exit_code = match command {
        Command::Map {
            repo_path,
            root,
            max_tokens,
            max_context_window,
            chat_files,
            mention_files,
            mention_idents,
            refresh,
            force_refresh,
            exclude_unranked,
            max_line_length,
            pagerank_damping,
            pagerank_tol,
            pagerank_max_iter,
            verbose,
            progress,
        } => {
            setup_tracing(verbose);
            let repo_root = detect_root(root.as_ref(), repo_path.as_ref());
            let cwd = std::env::current_dir().unwrap_or_default();

            let chat_fnames = resolve_chat_files(&chat_files);
            let mentioned_fnames = resolve_mention_files(&mention_files, &repo_root);
            let mentioned_idents: HashSet<String> = mention_idents.into_iter().collect();

            run_inner(RunArgs {
                root: repo_root,
                cwd,
                max_tokens,
                max_context_window,
                refresh: &refresh,
                force_refresh,
                exclude_unranked,
                max_line_length,
                pagerank_damping,
                pagerank_tol,
                pagerank_max_iter,
                verbose,
                progress,
                chat_fnames,
                mentioned_fnames,
                mentioned_idents,
                anchor_strs: &[],
            })
        }

        Command::Focus {
            anchors,
            root,
            max_tokens,
            max_context_window,
            refresh,
            force_refresh,
            exclude_unranked,
            max_line_length,
            pagerank_damping,
            pagerank_tol,
            pagerank_max_iter,
            verbose,
            progress,
        } => {
            setup_tracing(verbose);
            let repo_root = detect_root(root.as_ref(), None);
            let cwd = std::env::current_dir().unwrap_or_default();

            run_inner(RunArgs {
                root: repo_root,
                cwd,
                max_tokens,
                max_context_window,
                refresh: &refresh,
                force_refresh,
                exclude_unranked,
                max_line_length,
                pagerank_damping,
                pagerank_tol,
                pagerank_max_iter,
                verbose,
                progress,
                chat_fnames: vec![],
                mentioned_fnames: HashSet::new(),
                mentioned_idents: HashSet::new(),
                anchor_strs: &anchors,
            })
        }
    };

    std::process::exit(exit_code);
}

struct RunArgs<'a> {
    root: PathBuf,
    cwd: PathBuf,
    max_tokens: usize,
    max_context_window: Option<usize>,
    refresh: &'a str,
    force_refresh: bool,
    exclude_unranked: bool,
    max_line_length: usize,
    pagerank_damping: f64,
    pagerank_tol: f64,
    pagerank_max_iter: usize,
    verbose: bool,
    progress: bool,
    chat_fnames: Vec<PathBuf>,
    mentioned_fnames: HashSet<String>,
    mentioned_idents: HashSet<String>,
    anchor_strs: &'a [String],
}

/// Main execution logic. Returns exit code.
/// Exit codes (SPEC §17.6):
///   0 = success, map written to stdout
///   1 = fatal error (I/O, invalid config, unrecoverable failure)
///   2 = no map generated (budget, no files, all excluded)
fn run_inner(args: RunArgs<'_>) -> i32 {
    tracing::debug!("Repository root: {}", args.root.display());

    let refresh_mode = match args.refresh {
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

    // Resolve anchor strings into three buckets:
    //   file:ident  → anchor_scoped  (colon form, file part must exist)
    //   path/file   → anchor_fnames  (resolves to existing file)
    //   identifier  → anchor_idents  (anything else; tag-index lookup at runtime)
    let mut anchor_fnames: Vec<PathBuf> = Vec::new();
    let mut anchor_idents: HashSet<String> = HashSet::new();
    let mut anchor_scoped: Vec<(PathBuf, String)> = Vec::new();

    for val in args.anchor_strs {
        if let Some(colon) = val.find(':') {
            let file_part = &val[..colon];
            let ident_part = &val[colon + 1..];
            if !file_part.is_empty() && !ident_part.is_empty() {
                if let Some(candidate) = resolve_path(file_part, &args.cwd, &args.root) {
                    anchor_scoped.push((candidate, ident_part.to_string()));
                    continue;
                }
            }
        }
        if let Some(candidate) = resolve_path(val, &args.cwd, &args.root) {
            anchor_fnames.push(candidate);
        } else {
            anchor_idents.insert(val.clone());
        }
    }

    let mut repo_map = RepoMapConfig::builder()
        .root(args.root.clone())
        .map_tokens(args.max_tokens)
        .max_context_window(args.max_context_window)
        .refresh(refresh_mode)
        .force_refresh(args.force_refresh)
        .exclude_unranked(args.exclude_unranked)
        .max_line_length(args.max_line_length)
        .pagerank_damping(args.pagerank_damping)
        .pagerank_tol(args.pagerank_tol)
        .pagerank_max_iter(args.pagerank_max_iter)
        .verbose(args.verbose)
        .anchor_fnames(anchor_fnames)
        .anchor_idents(anchor_idents)
        .anchor_scoped(anchor_scoped)
        .build();

    let other_fnames = if args.progress {
        let pb = make_spinner("Scanning repository...");
        let files = enumerate_files(&args.root);
        pb.finish_and_clear();
        eprintln!("✓ Found {} files", files.len());
        files
    } else {
        enumerate_files(&args.root)
    };

    if other_fnames.is_empty() {
        tracing::warn!("No files found in repository");
        return 2;
    }
    tracing::debug!("Found {} files", other_fnames.len());

    for p in &args.chat_fnames {
        if !p.exists() {
            tracing::warn!("Chat file not found: {}", p.display());
        }
    }

    let map_result = if args.progress {
        let pb = make_spinner("Generating repo map...");
        let t0 = std::time::Instant::now();
        let r = repo_map.get_repo_map(
            &args.chat_fnames,
            &other_fnames,
            &args.mentioned_fnames,
            &args.mentioned_idents,
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
            &args.chat_fnames,
            &other_fnames,
            &args.mentioned_fnames,
            &args.mentioned_idents,
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

/// Detect repository root (§17.2).
fn detect_root(root_override: Option<&PathBuf>, repo_path: Option<&PathBuf>) -> PathBuf {
    if let Some(root) = root_override {
        return root.clone();
    }
    if let Some(path) = repo_path {
        return path.clone();
    }
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

fn resolve_chat_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_default();
    paths
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                cwd.join(p)
            }
        })
        .collect()
}

fn resolve_mention_files(paths: &[PathBuf], root: &std::path::Path) -> HashSet<String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    paths
        .iter()
        .map(|p| {
            let abs = if p.is_absolute() {
                p.clone()
            } else {
                cwd.join(p)
            };
            repo_mapper::path::rel_path(&abs, root)
        })
        .collect()
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
    fn parse_map_defaults() {
        let cli = Cli::parse_from(["repo-mapper", "map"]);
        let Some(Command::Map {
            max_tokens,
            refresh,
            verbose,
            ..
        }) = cli.command
        else {
            panic!("expected map subcommand");
        };
        assert_eq!(max_tokens, 1024);
        assert_eq!(refresh, "auto");
        assert!(!verbose);
    }

    #[test]
    fn parse_map_with_args() {
        let cli = Cli::parse_from([
            "repo-mapper",
            "map",
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
        let Some(Command::Map {
            max_tokens,
            verbose,
            chat_files,
            mention_files,
            mention_idents,
            repo_path,
            ..
        }) = cli.command
        else {
            panic!("expected map subcommand");
        };
        assert_eq!(max_tokens, 2048);
        assert!(verbose);
        assert_eq!(chat_files.len(), 1);
        assert_eq!(mention_files.len(), 1);
        assert_eq!(mention_idents.len(), 1);
        assert!(repo_path.is_some());
    }

    #[test]
    fn parse_map_refresh_mode() {
        let cli = Cli::parse_from(["repo-mapper", "map", "--refresh", "always"]);
        let Some(Command::Map { refresh, .. }) = cli.command else {
            panic!("expected map subcommand");
        };
        assert_eq!(refresh, "always");
    }

    #[test]
    fn parse_focus_anchors() {
        let cli = Cli::parse_from(["repo-mapper", "focus", "src/lib.rs", "process_job"]);
        let Some(Command::Focus {
            anchors,
            max_tokens,
            ..
        }) = cli.command
        else {
            panic!("expected focus subcommand");
        };
        assert_eq!(anchors, &["src/lib.rs", "process_job"]);
        assert_eq!(max_tokens, 1024);
    }

    #[test]
    fn no_subcommand_gives_none() {
        let cli = Cli::parse_from(["repo-mapper"]);
        assert!(cli.command.is_none());
    }
}
