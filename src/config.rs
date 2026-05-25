//! Configuration types (SPEC §14).

use std::path::PathBuf;

pub use crate::cache::map_cache::RefreshMode;

/// Configuration for RepoMap (SPEC §14).
#[derive(Debug, Clone)]
pub struct RepoMapConfig {
    /// Maximum tokens in repo map output (default: 1024)
    pub map_tokens: usize,
    /// Repository root directory (default: cwd)
    pub root: PathBuf,
    /// Prefix prepended to map output
    pub repo_content_prefix: Option<String>,
    /// Emit diagnostic output
    pub verbose: bool,
    /// LLM context window size
    pub max_context_window: Option<usize>,
    /// Multiplier for no-chat mode (default: 8)
    pub map_mul_no_files: usize,
    /// Map cache refresh mode (default: Auto)
    pub refresh: RefreshMode,
    /// Force cache recomputation
    pub force_refresh: bool,
    /// Exclude files with PageRank ≤ 0.0001
    pub exclude_unranked: bool,
    /// Self-edge weight (default: 0.1)
    pub self_edge_weight: f64,
    /// Maximum line length (default: 100)
    pub max_line_length: usize,
    /// PageRank damping factor (default: 0.85)
    pub pagerank_damping: f64,
    /// PageRank convergence tolerance (default: 1e-6)
    pub pagerank_tol: f64,
    /// PageRank maximum iterations (default: 100)
    pub pagerank_max_iter: usize,
}

impl Default for RepoMapConfig {
    fn default() -> Self {
        Self {
            map_tokens: 1024,
            root: std::env::current_dir().unwrap_or_default(),
            repo_content_prefix: None,
            verbose: false,
            max_context_window: None,
            map_mul_no_files: 8,
            refresh: RefreshMode::Auto,
            force_refresh: false,
            exclude_unranked: false,
            self_edge_weight: 0.1,
            max_line_length: 100,
            pagerank_damping: 0.85,
            pagerank_tol: 1e-6,
            pagerank_max_iter: 100,
        }
    }
}

/// Builder for RepoMapConfig.
#[derive(Debug, Default)]
pub struct RepoMapConfigBuilder {
    config: RepoMapConfig,
}

impl RepoMapConfig {
    /// Create a new builder.
    pub fn builder() -> RepoMapConfigBuilder {
        RepoMapConfigBuilder::default()
    }
}

impl RepoMapConfigBuilder {
    pub fn map_tokens(mut self, n: usize) -> Self {
        self.config.map_tokens = n;
        self
    }

    pub fn root(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.root = path.into();
        self
    }

    pub fn repo_content_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.repo_content_prefix = Some(prefix.into());
        self
    }

    pub fn verbose(mut self, v: bool) -> Self {
        self.config.verbose = v;
        self
    }

    pub fn max_context_window(mut self, n: Option<usize>) -> Self {
        self.config.max_context_window = n;
        self
    }

    pub fn map_mul_no_files(mut self, n: usize) -> Self {
        self.config.map_mul_no_files = n;
        self
    }

    pub fn refresh(mut self, mode: RefreshMode) -> Self {
        self.config.refresh = mode;
        self
    }

    pub fn force_refresh(mut self, v: bool) -> Self {
        self.config.force_refresh = v;
        self
    }

    pub fn exclude_unranked(mut self, v: bool) -> Self {
        self.config.exclude_unranked = v;
        self
    }

    pub fn self_edge_weight(mut self, w: f64) -> Self {
        self.config.self_edge_weight = w;
        self
    }

    pub fn max_line_length(mut self, n: usize) -> Self {
        self.config.max_line_length = n;
        self
    }

    pub fn pagerank_damping(mut self, d: f64) -> Self {
        self.config.pagerank_damping = d;
        self
    }

    pub fn pagerank_tol(mut self, t: f64) -> Self {
        self.config.pagerank_tol = t;
        self
    }

    pub fn pagerank_max_iter(mut self, n: usize) -> Self {
        self.config.pagerank_max_iter = n;
        self
    }

    pub fn build(self) -> crate::repo_map::RepoMap {
        crate::repo_map::RepoMap::new(self.config)
    }
}
