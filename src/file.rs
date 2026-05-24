//! File reading utilities (SPEC §13.5, §13.6).

use std::fs;
use std::path::Path;

/// Read a file as UTF-8, replacing invalid bytes with replacement character.
///
/// Returns `None` if the file cannot be read (SPEC §13.6).
pub fn read_file_utf8(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Check if a path points to a regular file.
///
/// Returns `false` for directories, symlinks to directories, and broken symlinks.
pub fn is_regular_file(path: &Path) -> bool {
    path.is_file()
}

/// Get the modification time of a file as floating-point seconds.
///
/// Returns `None` if the file doesn't exist or mtime cannot be read.
pub fn get_mtime(path: &Path) -> Option<f64> {
    let metadata = fs::metadata(path).ok()?;
    let mtime = metadata.modified().ok()?;
    let duration = mtime.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn read_valid_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "hello world").unwrap();
        let content = read_file_utf8(file.path()).unwrap();
        assert!(content.contains("hello world"));
    }

    #[test]
    fn read_missing_file() {
        let result = read_file_utf8(Path::new("/nonexistent/file.txt"));
        assert!(result.is_none());
    }

    #[test]
    fn read_invalid_utf8() {
        let mut file = NamedTempFile::new().unwrap();
        // Write invalid UTF-8 bytes
        file.write_all(&[0xFF, 0xFE, b'h', b'i']).unwrap();
        let content = read_file_utf8(file.path()).unwrap();
        // Should contain replacement characters but not fail
        assert!(content.contains("hi"));
    }

    #[test]
    fn is_regular_file_true() {
        let file = NamedTempFile::new().unwrap();
        assert!(is_regular_file(file.path()));
    }

    #[test]
    fn is_regular_file_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_regular_file(dir.path()));
    }

    #[test]
    fn get_mtime_exists() {
        let file = NamedTempFile::new().unwrap();
        let mtime = get_mtime(file.path());
        assert!(mtime.is_some());
        assert!(mtime.unwrap() > 0.0);
    }
}
