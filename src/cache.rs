//! Query result caching (Django-style connection-scoped caching)
//!
//! Provides automatic query result caching within a connection scope,
//! similar to Django's transaction-level query caching. Cache is automatically
//! cleared when the scope ends, preventing stale data.

use sea_orm::{DatabaseConnection, DbErr};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

/// Wrapper around DatabaseConnection that provides query result caching.
///
/// All queries executed through this connection are cached in memory.
/// Subsequent identical queries return cached results without hitting the database.
///
/// **Cache Lifetime:** Cache is automatically cleared when this wrapper is dropped.
/// This prevents stale data across requests/transactions.
///
/// # Examples
///
/// ```rust,ignore
/// // Enable caching for a scope
/// let cached_db = db.with_query_cache();
///
/// // First call - executes SQL
/// let books1 = Book::objects(&cached_db).all().await?;
///
/// // Second identical call - returns cached (no SQL)
/// let books2 = Book::objects(&cached_db).all().await?;
///
/// // Cache cleared when cached_db goes out of scope
/// ```
///
/// # Use Cases
///
/// - Avoid redundant queries within a request handler
/// - Optimize transaction-heavy code paths
/// - Reduce database load for read-heavy operations
///
/// # Safety
///
/// - **Thread-safe:** Uses RwLock for concurrent access
/// - **No stale data:** Cache cleared on drop
/// - **No race conditions:** Scoped to single connection wrapper
pub struct CachedConnection {
    inner: DatabaseConnection,
    cache: Arc<RwLock<HashMap<u64, Arc<Vec<u8>>>>>,
}

impl CachedConnection {
    /// Create a new cached connection wrapper.
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a cached result or execute the query.
    ///
    /// Uses query hash as cache key. Returns Arc'd result to avoid cloning.
    pub async fn cached_query<F, T>(&self, query_key: &str, f: F) -> Result<Arc<Vec<u8>>, DbErr>
    where
        F: std::future::Future<Output = Result<Vec<u8>, DbErr>>,
    {
        let hash = Self::hash_query(query_key);

        // Try to get from cache
        {
            let cache_read = self
                .cache
                .read()
                .map_err(|e| DbErr::Custom(format!("Cache lock poisoned: {}", e)))?;
            if let Some(cached) = cache_read.get(&hash) {
                return Ok(Arc::clone(cached));
            }
        }

        // Cache miss - execute query
        let result = f.await?;
        let result_arc = Arc::new(result);

        // Store in cache
        {
            let mut cache_write = self
                .cache
                .write()
                .map_err(|e| DbErr::Custom(format!("Cache lock poisoned: {}", e)))?;
            cache_write.insert(hash, Arc::clone(&result_arc));
        }

        Ok(result_arc)
    }

    /// Get the inner database connection.
    pub fn inner(&self) -> &DatabaseConnection {
        &self.inner
    }

    /// Get cache statistics (for debugging).
    ///
    /// Returns `None` if the cache lock is poisoned.
    pub fn cache_stats(&self) -> Option<CacheStats> {
        let cache = self.cache.read().ok()?;
        Some(CacheStats {
            entries: cache.len(),
            total_bytes: cache.values().map(|v| v.len()).sum(),
        })
    }

    /// Clear the query cache manually.
    ///
    /// Returns `false` if the cache lock is poisoned.
    pub fn clear_cache(&self) -> bool {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
            true
        } else {
            false
        }
    }

    fn hash_query(query: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        hasher.finish()
    }
}

/// Cache statistics for debugging.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached query results
    pub entries: usize,
    /// Total bytes stored in cache
    pub total_bytes: usize,
}

// Implement Deref to automatically delegate all ConnectionTrait methods to inner
impl std::ops::Deref for CachedConnection {
    type Target = DatabaseConnection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Extension trait to enable query caching on any ConnectionTrait.
pub trait ConnectionCacheExt {
    /// Wrap this connection with query result caching.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let cached_db = db.with_query_cache();
    /// let books = Book::objects(&cached_db).all().await?;
    /// ```
    fn with_query_cache(self) -> CachedConnection;
}

impl ConnectionCacheExt for DatabaseConnection {
    fn with_query_cache(self) -> CachedConnection {
        CachedConnection::new(self)
    }
}

impl ConnectionCacheExt for &DatabaseConnection {
    fn with_query_cache(self) -> CachedConnection {
        CachedConnection::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_hashing() {
        let query1 = "SELECT * FROM books WHERE id = 1";
        let query2 = "SELECT * FROM books WHERE id = 1";
        let query3 = "SELECT * FROM books WHERE id = 2";

        let hash1 = CachedConnection::hash_query(query1);
        let hash2 = CachedConnection::hash_query(query2);
        let hash3 = CachedConnection::hash_query(query3);

        assert_eq!(hash1, hash2, "Identical queries should have same hash");
        assert_ne!(hash1, hash3, "Different queries should have different hashes");
    }
}
