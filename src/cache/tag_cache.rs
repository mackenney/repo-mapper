//! Persistent tag cache (SPEC §5).

use crate::tag::Tag;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Cache version, increment when format changes (SPEC §5.2).
const CACHE_VERSION: u32 = 1;

/// Persistent tag cache backed by SQLite (SPEC §5.1).
pub struct TagCache {
    /// SQLite connection, or None if using fallback
    conn: Option<Connection>,
    /// In-memory fallback cache
    fallback: HashMap<String, CacheEntry>,
    /// Whether we're using fallback mode
    using_fallback: bool,
}

/// A cached entry for a file.
#[derive(Debug, Clone)]
struct CacheEntry {
    mtime: f64,
    tags: Vec<Tag>,
}

impl TagCache {
    /// Create or open the tag cache for a repository root.
    ///
    /// Cache location: `{root}/.aider.tags.cache.v{VERSION}` (SPEC §5.2)
    pub fn new(root: &Path) -> Self {
        let cache_path = cache_path(root);

        match Self::open_sqlite(&cache_path) {
            Ok(conn) => {
                debug!("Opened tag cache at {}", cache_path.display());
                TagCache {
                    conn: Some(conn),
                    fallback: HashMap::new(),
                    using_fallback: false,
                }
            }
            Err(e) => {
                // SPEC §5.5 step 1: attempt delete and recreate before giving up
                warn!(
                    "Tag cache open failed ({}); attempting delete and recreate",
                    e
                );
                let _ = std::fs::remove_dir_all(&cache_path);
                match Self::open_sqlite(&cache_path) {
                    Ok(conn) => {
                        debug!("Tag cache recreated at {}", cache_path.display());
                        TagCache {
                            conn: Some(conn),
                            fallback: HashMap::new(),
                            using_fallback: false,
                        }
                    }
                    Err(e2) => {
                        // SPEC §5.5 step 2: recreation also failed, fall back to in-memory
                        warn!(
                            "Tag cache recreation failed ({}); using in-memory fallback",
                            e2
                        );
                        TagCache {
                            conn: None,
                            fallback: HashMap::new(),
                            using_fallback: true,
                        }
                    }
                }
            }
        }
    }

    /// Open SQLite database, creating schema if needed.
    fn open_sqlite(path: &Path) -> Result<Connection, rusqlite::Error> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;

        // Create table if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tags (
                path TEXT PRIMARY KEY,
                mtime REAL NOT NULL,
                data BLOB NOT NULL
            )",
            [],
        )?;

        Ok(conn)
    }

    /// Get cached tags for a file if valid.
    ///
    /// Returns `None` if:
    /// - File not in cache
    /// - Cached mtime differs from current_mtime (SPEC §5.4)
    pub fn get(&self, abs_path: &str, current_mtime: f64) -> Option<Vec<Tag>> {
        if self.using_fallback {
            return self.get_fallback(abs_path, current_mtime);
        }

        let conn = self.conn.as_ref()?;

        let result: Result<(f64, Vec<u8>), _> = conn.query_row(
            "SELECT mtime, data FROM tags WHERE path = ?1",
            params![abs_path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok((stored_mtime, data)) => {
                // Check mtime validity (SPEC §5.4)
                if (stored_mtime - current_mtime).abs() > 0.001 {
                    debug!("Cache miss (mtime changed): {}", abs_path);
                    return None;
                }

                // Deserialize tags
                match bincode::deserialize(&data) {
                    Ok(tags) => {
                        debug!("Cache hit: {}", abs_path);
                        Some(tags)
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached tags for {}: {}", abs_path, e);
                        None
                    }
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                debug!("Cache miss (not found): {}", abs_path);
                None
            }
            Err(e) => {
                warn!("Cache lookup error for {}: {}", abs_path, e);
                None
            }
        }
    }

    /// Get from fallback cache.
    fn get_fallback(&self, abs_path: &str, current_mtime: f64) -> Option<Vec<Tag>> {
        let entry = self.fallback.get(abs_path)?;
        if (entry.mtime - current_mtime).abs() > 0.001 {
            return None;
        }
        Some(entry.tags.clone())
    }

    /// Store tags in the cache.
    pub fn set(&mut self, abs_path: &str, mtime: f64, tags: Vec<Tag>) {
        if self.using_fallback {
            self.set_fallback(abs_path, mtime, tags);
            return;
        }

        let conn = match self.conn.as_ref() {
            Some(c) => c,
            None => {
                self.set_fallback(abs_path, mtime, tags);
                return;
            }
        };

        // Serialize tags
        let data = match bincode::serialize(&tags) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to serialize tags for {}: {}", abs_path, e);
                return;
            }
        };

        // Insert or replace
        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO tags (path, mtime, data) VALUES (?1, ?2, ?3)",
            params![abs_path, mtime, data],
        ) {
            warn!("Failed to cache tags for {}: {}", abs_path, e);
            // Fall back to memory cache on error (SPEC §5.5)
            self.switch_to_fallback();
            self.set_fallback(abs_path, mtime, tags);
        } else {
            debug!("Cached tags for {}", abs_path);
        }
    }

    /// Store in fallback cache.
    fn set_fallback(&mut self, abs_path: &str, mtime: f64, tags: Vec<Tag>) {
        self.fallback
            .insert(abs_path.to_string(), CacheEntry { mtime, tags });
    }

    /// Switch to fallback mode on SQLite error (SPEC §5.5).
    fn switch_to_fallback(&mut self) {
        if !self.using_fallback {
            warn!("Switching to in-memory tag cache fallback");
            self.using_fallback = true;
            self.conn = None;
        }
    }

    /// Check if using fallback mode.
    pub fn is_using_fallback(&self) -> bool {
        self.using_fallback
    }
}

/// Compute the cache file path for a repository root.
fn cache_path(root: &Path) -> PathBuf {
    root.join(format!(".aider.tags.cache.v{}", CACHE_VERSION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::{Tag, TagKind};
    use tempfile::TempDir;

    fn test_tag(name: &str) -> Tag {
        Tag::new("test.rs", "/test.rs", 1, name, TagKind::Def)
    }

    #[test]
    fn cache_set_and_get() {
        let dir = TempDir::new().unwrap();
        let mut cache = TagCache::new(dir.path());

        let tags = vec![test_tag("foo"), test_tag("bar")];
        cache.set("/test.rs", 1000.0, tags.clone());

        let result = cache.get("/test.rs", 1000.0);
        assert!(result.is_some());
        let cached = result.unwrap();
        assert_eq!(cached.len(), 2);
        assert!(cached.iter().any(|t| t.name == "foo"));
        assert!(cached.iter().any(|t| t.name == "bar"));
    }

    #[test]
    fn cache_miss_mtime_changed() {
        let dir = TempDir::new().unwrap();
        let mut cache = TagCache::new(dir.path());

        let tags = vec![test_tag("foo")];
        cache.set("/test.rs", 1000.0, tags);

        // Different mtime should miss
        let result = cache.get("/test.rs", 1001.0);
        assert!(result.is_none());
    }

    #[test]
    fn cache_miss_not_found() {
        let dir = TempDir::new().unwrap();
        let cache = TagCache::new(dir.path());

        let result = cache.get("/nonexistent.rs", 1000.0);
        assert!(result.is_none());
    }

    #[test]
    fn cache_path_format() {
        let root = Path::new("/home/user/project");
        let path = cache_path(root);
        assert!(path.to_string_lossy().contains(".aider.tags.cache.v"));
    }

    #[test]
    fn fallback_mode() {
        // Create cache with invalid path to force fallback
        let mut cache = TagCache {
            conn: None,
            fallback: HashMap::new(),
            using_fallback: true,
        };

        assert!(cache.is_using_fallback());

        let tags = vec![test_tag("foo")];
        cache.set("/test.rs", 1000.0, tags.clone());

        let result = cache.get("/test.rs", 1000.0);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }
}
