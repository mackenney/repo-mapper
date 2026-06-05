//! Edge case handling (SPEC §13).

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Per-instance warning deduplication (SPEC §13.5).
pub struct WarnedFiles {
    warned: RefCell<HashSet<PathBuf>>,
}

impl WarnedFiles {
    /// Create a new, empty warning tracker.
    pub fn new() -> Self {
        Self {
            warned: RefCell::new(HashSet::new()),
        }
    }

    /// Warn about a missing file once per path per instance.
    pub fn warn_missing(&self, path: &Path) {
        let mut warned = self.warned.borrow_mut();
        if !warned.contains(path) {
            warn!("File not found, skipping: {}", path.display());
            warned.insert(path.to_path_buf());
        }
    }
}

impl Default for WarnedFiles {
    fn default() -> Self {
        Self::new()
    }
}

/// Substitute {other} placeholder in prefix (SPEC §12.1).
pub fn substitute_prefix(prefix: Option<&str>, has_chat_files: bool) -> String {
    match prefix {
        None => String::new(),
        Some(p) => {
            let other = if has_chat_files { "other " } else { "" };
            p.replace("{other}", other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_with_chat() {
        let result = substitute_prefix(Some("Here are the {other}files:"), true);
        assert_eq!(result, "Here are the other files:");
    }

    #[test]
    fn substitute_without_chat() {
        let result = substitute_prefix(Some("Here are the {other}files:"), false);
        assert_eq!(result, "Here are the files:");
    }

    #[test]
    fn substitute_none() {
        let result = substitute_prefix(None, true);
        assert_eq!(result, "");
    }
}
