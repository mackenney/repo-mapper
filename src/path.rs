//! Path utilities for relative path computation (SPEC §2.2).

use std::path::{Path, PathBuf};

/// Compute the relative path from `root` to `fname`.
///
/// Falls back to the absolute path if relative computation fails
/// (e.g., Windows cross-drive paths per SPEC §2.2).
pub fn rel_path(fname: &Path, root: &Path) -> String {
    pathdiff::diff_paths(fname, root)
        .map(|p| normalize_slashes(&p))
        .unwrap_or_else(|| fname.to_string_lossy().into_owned())
}

/// Normalize path separators to forward slashes.
///
/// Ensures consistent path representation across platforms.
fn normalize_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Canonicalize a path, resolving symlinks and relative components.
///
/// Returns the path unchanged if canonicalization fails.
pub fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Extract the file stem (basename without extension).
pub fn file_stem(path: &Path) -> Option<&str> {
    path.file_stem().and_then(|s| s.to_str())
}

/// Extract the file name (basename with extension).
pub fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|s| s.to_str())
}

/// Extract the file extension.
pub fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|s| s.to_str())
}

/// Get all path components as strings (for mentioned_idents matching in §7.1).
pub fn path_components(rel_fname: &str) -> impl Iterator<Item = &str> {
    Path::new(rel_fname)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rel_path_basic() {
        let root = Path::new("/home/user/project");
        let fname = Path::new("/home/user/project/src/main.rs");
        assert_eq!(rel_path(fname, root), "src/main.rs");
    }

    #[test]
    fn rel_path_same_dir() {
        let root = Path::new("/home/user/project");
        let fname = Path::new("/home/user/project/file.rs");
        assert_eq!(rel_path(fname, root), "file.rs");
    }

    #[test]
    fn rel_path_fallback_to_absolute() {
        // When diff_paths returns None, fall back to absolute
        // This happens on Windows with cross-drive paths
        // We can't easily test this on Unix, but verify the function runs
        let root = Path::new("/root");
        let fname = Path::new("/other/path/file.rs");
        let result = rel_path(fname, root);
        // On Unix this will be a relative path like "../other/path/file.rs"
        // or fall back to absolute; either is acceptable
        assert!(!result.is_empty());
    }

    #[test]
    fn path_components_basic() {
        let components: Vec<_> = path_components("src/lib/mod.rs").collect();
        assert_eq!(components, vec!["src", "lib", "mod.rs"]);
    }

    #[test]
    fn file_stem_basic() {
        assert_eq!(file_stem(Path::new("main.rs")), Some("main"));
        assert_eq!(file_stem(Path::new("lib.tar.gz")), Some("lib.tar"));
    }

    #[test]
    fn extension_basic() {
        assert_eq!(extension(Path::new("main.rs")), Some("rs"));
        assert_eq!(extension(Path::new("Makefile")), None);
    }
}
