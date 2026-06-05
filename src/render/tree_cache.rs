//! Render result caching (SPEC §9.2).

use crate::render::tree_context::TreeContext;
use std::collections::HashMap;

/// Cache key for rendered tree results: (rel_fname, sorted_lois, mtime_bits).
pub type TreeCacheKey = (String, Vec<i32>, u64);

/// Cache for rendered tree strings.
#[derive(Debug, Default)]
pub struct TreeCache {
    cache: HashMap<TreeCacheKey, String>,
}

impl TreeCache {
    /// Create a new empty tree cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the cache (called at start of each uncached computation).
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get a cached render result.
    pub fn get(&self, key: &TreeCacheKey) -> Option<&String> {
        self.cache.get(key)
    }

    /// Store a render result.
    pub fn set(&mut self, key: TreeCacheKey, value: String) {
        self.cache.insert(key, value);
    }

    /// Create a cache key from components.
    pub fn make_key(rel_fname: &str, lois: &[i32], mtime: f64) -> TreeCacheKey {
        let mut sorted_lois: Vec<i32> = lois.to_vec();
        sorted_lois.sort_unstable();
        (rel_fname.to_string(), sorted_lois, mtime.to_bits())
    }
}

/// Cache for TreeContext objects, keyed by rel_fname.
///
/// Per SPEC §9.2: stored with mtime, replaced on mismatch.
#[derive(Debug, Default)]
pub struct TreeContextCache {
    cache: HashMap<String, (TreeContext, f64)>,
}

impl TreeContextCache {
    /// Create a new empty tree context cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a TreeContext for a file.
    ///
    /// Returns the cached context if mtime matches, otherwise creates new.
    pub fn get_or_create(
        &mut self,
        rel_fname: &str,
        abs_fname: &std::path::Path,
        content: &str,
        current_mtime: f64,
    ) -> &mut TreeContext {
        // Check if we have a valid cached entry
        let needs_new = self
            .cache
            .get(rel_fname)
            .map(|(_, mtime)| (*mtime - current_mtime).abs() > 0.001)
            .unwrap_or(true);

        if needs_new {
            let ctx = TreeContext::new(content, abs_fname);
            self.cache
                .insert(rel_fname.to_string(), (ctx, current_mtime));
        }

        &mut self.cache.get_mut(rel_fname).unwrap().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_cache_key_sorting() {
        let key1 = TreeCache::make_key("test.rs", &[3, 1, 2], 1000.0);
        let key2 = TreeCache::make_key("test.rs", &[1, 2, 3], 1000.0);
        // Keys should be equal (lois sorted)
        assert_eq!(key1, key2);
    }

    #[test]
    fn tree_cache_set_get() {
        let mut cache = TreeCache::new();
        let key = TreeCache::make_key("test.rs", &[1, 2], 1000.0);
        cache.set(key.clone(), "rendered content".to_string());

        let result = cache.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "rendered content");
    }

    #[test]
    fn tree_cache_clear() {
        let mut cache = TreeCache::new();
        let key = TreeCache::make_key("test.rs", &[1], 1000.0);
        cache.set(key.clone(), "content".to_string());
        cache.clear();

        assert!(cache.get(&key).is_none());
    }
}
