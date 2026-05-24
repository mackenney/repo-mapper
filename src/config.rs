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
