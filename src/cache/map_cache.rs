//! In-memory map cache (SPEC §11).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

/// Map cache refresh modes (SPEC §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefreshMode {
    /// Return last_map if set, never recompute
    Manual,
    /// Never use cache, always recompute
    Always,
    /// Always use cache if key matches
    Files,
    /// Use cache only if last computation took >1.0s
    #[default]
    Auto,
}

/// Cache key for map cache (SPEC §11).
///
/// Different modes use different key structures:
/// - Auto mode: 5-tuple (chat, other, max_tokens, mentioned_fnames, mentioned_idents)
/// - Files mode: 3-tuple (chat, other, max_tokens)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapCacheKey {
    /// Files mode key (3-tuple)
    Files(FilesKey),
    /// Auto mode key (5-tuple)
    Auto(AutoKey),
}

/// Files mode cache key (SPEC §11).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FilesKey {
    pub chat_fnames: Option<Vec<PathBuf>>,
    pub other_fnames: Option<Vec<PathBuf>>,
    pub max_tokens: usize,
}

/// Auto mode cache key (SPEC §11).
/// Auto mode cache key (SPEC §11).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AutoKey {
    pub chat_fnames: Option<Vec<PathBuf>>,
    pub other_fnames: Option<Vec<PathBuf>>,
    pub max_tokens: usize,
    pub mentioned_fnames: Option<Vec<String>>,
    pub mentioned_idents: Option<Vec<String>>,
    pub anchor_fnames: Option<Vec<PathBuf>>,
    pub anchor_idents: Option<Vec<String>>,
}

impl MapCacheKey {
    /// Create a Files mode key.
    pub fn files(chat_fnames: &[PathBuf], other_fnames: &[PathBuf], max_tokens: usize) -> Self {
        MapCacheKey::Files(FilesKey {
            chat_fnames: non_empty_sorted(chat_fnames),
            other_fnames: non_empty_sorted(other_fnames),
            max_tokens,
        })
    }

    /// Create an Auto mode key.
    pub fn auto(
        chat_fnames: &[PathBuf],
        other_fnames: &[PathBuf],
        max_tokens: usize,
        mentioned_fnames: &HashSet<String>,
        mentioned_idents: &HashSet<String>,
        anchor_fnames: &[PathBuf],
        anchor_idents: &HashSet<String>,
    ) -> Self {
        MapCacheKey::Auto(AutoKey {
            chat_fnames: non_empty_sorted(chat_fnames),
            other_fnames: non_empty_sorted(other_fnames),
            max_tokens,
            mentioned_fnames: non_empty_sorted_set(mentioned_fnames),
            mentioned_idents: non_empty_sorted_set(mentioned_idents),
            anchor_fnames: non_empty_sorted(anchor_fnames),
            anchor_idents: non_empty_sorted_set(anchor_idents),
        })
    }
}

/// Convert to sorted Vec, or None if empty (SPEC §11: empty → None).
fn non_empty_sorted<T: Clone + Ord>(items: &[T]) -> Option<Vec<T>> {
    if items.is_empty() {
        None
    } else {
        let mut v: Vec<_> = items.to_vec();
        v.sort();
        Some(v)
    }
}

/// Convert HashSet to sorted Vec, or None if empty.
fn non_empty_sorted_set<T: Clone + Ord>(items: &HashSet<T>) -> Option<Vec<T>> {
    if items.is_empty() {
        None
    } else {
        let mut v: Vec<_> = items.iter().cloned().collect();
        v.sort();
        Some(v)
    }
}

/// In-memory map cache (SPEC §5.6, §11).
#[derive(Debug, Default)]
pub struct MapCache {
    /// Keyed cache for Files and Auto modes
    cache: HashMap<MapCacheKey, String>,
    /// Last computed map for Manual mode
    last_map: Option<String>,
    /// Duration of last computation (for Auto mode threshold)
    last_duration: Duration,
}

impl MapCache {
    /// Create a new empty map cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if cache should be used based on mode and flags.
    ///
    /// Returns true if a cache lookup should be attempted.
    pub fn should_use_cache(&self, mode: RefreshMode, force_refresh: bool) -> bool {
        if force_refresh {
            return false;
        }

        match mode {
            RefreshMode::Manual => self.last_map.is_some(),
            RefreshMode::Always => false,
            RefreshMode::Files => true,
            RefreshMode::Auto => self.last_duration > Duration::from_secs_f64(1.0),
        }
    }

    /// Get a cached map.
    ///
    /// For Manual mode, use `get_last_map` instead.
    pub fn get(
        &self,
        key: &MapCacheKey,
        mode: RefreshMode,
        force_refresh: bool,
    ) -> Option<&String> {
        if !self.should_use_cache(mode, force_refresh) {
            return None;
        }

        // Manual mode uses last_map, not keyed cache
        if mode == RefreshMode::Manual {
            return self.last_map.as_ref();
        }

        self.cache.get(key)
    }

    /// Get the last computed map (for Manual mode).
    pub fn get_last_map(&self) -> Option<&String> {
        self.last_map.as_ref()
    }

    /// Store a computed map.
    ///
    /// Per SPEC §11: always writes to both keyed cache and last_map,
    /// regardless of mode or force_refresh.
    pub fn set(&mut self, key: MapCacheKey, value: String, duration: Duration) {
        self.cache.insert(key, value.clone());
        self.last_map = Some(value);
        self.last_duration = duration;
    }

    /// Get the duration of the last computation.
    pub fn last_duration(&self) -> Duration {
        self.last_duration
    }

    /// Clear the cache (for testing or reset).
    pub fn clear(&mut self) {
        self.cache.clear();
        self.last_map = None;
        self.last_duration = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_files_mode() {
        let key = MapCacheKey::files(
            &[PathBuf::from("a.rs"), PathBuf::from("b.rs")],
            &[PathBuf::from("c.rs")],
            1024,
        );

        if let MapCacheKey::Files(k) = &key {
            assert_eq!(k.max_tokens, 1024);
            // Should be sorted
            let chat = k.chat_fnames.as_ref().unwrap();
            assert_eq!(chat[0], PathBuf::from("a.rs"));
            assert_eq!(chat[1], PathBuf::from("b.rs"));
        } else {
            panic!("Expected Files key");
        }
    }

    #[test]
    fn cache_key_empty_is_none() {
        let key = MapCacheKey::files(&[], &[PathBuf::from("c.rs")], 1024);

        if let MapCacheKey::Files(k) = &key {
            assert!(k.chat_fnames.is_none()); // Empty → None
            assert!(k.other_fnames.is_some());
        } else {
            panic!("Expected Files key");
        }
    }

    #[test]
    fn cache_set_and_get() {
        let mut cache = MapCache::new();
        let key = MapCacheKey::files(&[PathBuf::from("a.rs")], &[], 1024);

        cache.set(key.clone(), "test map".to_string(), Duration::from_secs(2));

        let result = cache.get(&key, RefreshMode::Files, false);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "test map");
    }

    #[test]
    fn cache_manual_mode() {
        let mut cache = MapCache::new();
        let key = MapCacheKey::files(&[PathBuf::from("a.rs")], &[], 1024);

        cache.set(key.clone(), "test map".to_string(), Duration::from_secs(1));

        // Manual mode ignores key, returns last_map
        let other_key = MapCacheKey::files(&[PathBuf::from("b.rs")], &[], 2048);
        let result = cache.get(&other_key, RefreshMode::Manual, false);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "test map");
    }

    #[test]
    fn cache_always_mode() {
        let mut cache = MapCache::new();
        let key = MapCacheKey::files(&[PathBuf::from("a.rs")], &[], 1024);

        cache.set(key.clone(), "test map".to_string(), Duration::from_secs(1));

        // Always mode never uses cache
        let result = cache.get(&key, RefreshMode::Always, false);
        assert!(result.is_none());
    }

    #[test]
    fn cache_auto_mode_threshold() {
        let mut cache = MapCache::new();
        let key = MapCacheKey::auto(
            &[PathBuf::from("a.rs")],
            &[],
            1024,
            &HashSet::new(),
            &HashSet::new(),
            &[],
            &HashSet::new(),
        );

        // Duration < 1s → don't use cache
        cache.set(
            key.clone(),
            "fast map".to_string(),
            Duration::from_millis(500),
        );
        let result = cache.get(&key, RefreshMode::Auto, false);
        assert!(result.is_none());

        // Duration > 1s → use cache
        cache.set(key.clone(), "slow map".to_string(), Duration::from_secs(2));
        let result = cache.get(&key, RefreshMode::Auto, false);
        assert!(result.is_some());
    }

    #[test]
    fn cache_force_refresh() {
        let mut cache = MapCache::new();
        let key = MapCacheKey::files(&[PathBuf::from("a.rs")], &[], 1024);

        cache.set(key.clone(), "test map".to_string(), Duration::from_secs(2));

        // force_refresh bypasses cache
        let result = cache.get(&key, RefreshMode::Files, true);
        assert!(result.is_none());
    }

    #[test]
    fn cache_always_writes() {
        let mut cache = MapCache::new();
        let key = MapCacheKey::files(&[PathBuf::from("a.rs")], &[], 1024);

        // Even in Always mode, set still writes
        cache.set(key.clone(), "test map".to_string(), Duration::from_secs(1));

        // Verify it was written (by checking in Files mode)
        let result = cache.get(&key, RefreshMode::Files, false);
        assert!(result.is_some());
    }
}
