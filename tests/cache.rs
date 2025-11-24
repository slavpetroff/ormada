// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Cache integration tests

mod fixtures;

use fixtures::*;
use rstest::*;
use seaorm_django::cache::{CachedConnection, ConnectionCacheExt};
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_cached_connection_basic() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    let cached = db.with_query_cache();
    
    // Verify we can access inner connection
    let inner = cached.inner();
    assert!(inner.ping().await.is_ok());
    
    // CachedConnection should deref to DatabaseConnection
    assert!(cached.ping().await.is_ok());
}

#[tokio::test]
async fn test_cache_stats_empty() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let cached = db.with_query_cache();
    
    let stats = cached.cache_stats().unwrap();
    assert_eq!(stats.entries, 0);
    assert_eq!(stats.total_bytes, 0);
}

#[tokio::test]
async fn test_cache_clear() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let cached = db.with_query_cache();
    
    // Initially cache should be clearable
    assert!(cached.clear_cache());
    
    let stats = cached.cache_stats().unwrap();
    assert_eq!(stats.entries, 0);
}

#[tokio::test]
async fn test_cached_query_hit_and_miss() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let cached = CachedConnection::new(db);
    
    // First call - cache miss
    let result1 = cached
        .cached_query::<_, ()>("test_query", async { Ok(vec![1, 2, 3]) })
        .await
        .unwrap();
    
    assert_eq!(*result1, vec![1, 2, 3]);
    
    // Second call - cache hit (closure not executed)
    let result2 = cached
        .cached_query::<_, ()>("test_query", async { Ok(vec![4, 5, 6]) })
        .await
        .unwrap();
    
    // Should return cached result (1,2,3), not new result (4,5,6)
    assert_eq!(*result2, vec![1, 2, 3]);
    
    // Verify cache has entry
    let stats = cached.cache_stats().unwrap();
    assert_eq!(stats.entries, 1);
}

#[tokio::test]
async fn test_cached_query_different_keys() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let cached = CachedConnection::new(db);
    
    let result1 = cached
        .cached_query::<_, ()>("query1", async { Ok(vec![1]) })
        .await
        .unwrap();
    
    let result2 = cached
        .cached_query::<_, ()>("query2", async { Ok(vec![2]) })
        .await
        .unwrap();
    
    assert_eq!(*result1, vec![1]);
    assert_eq!(*result2, vec![2]);
    
    // Should have 2 cache entries
    let stats = cached.cache_stats().unwrap();
    assert_eq!(stats.entries, 2);
}

#[tokio::test]
async fn test_clear_cache_after_use() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let cached = CachedConnection::new(db);
    
    // Add some cache entries
    cached.cached_query::<_, ()>("q1", async { Ok(vec![1]) }).await.unwrap();
    cached.cached_query::<_, ()>("q2", async { Ok(vec![2]) }).await.unwrap();
    
    let stats_before = cached.cache_stats().unwrap();
    assert_eq!(stats_before.entries, 2);
    
    // Clear cache
    assert!(cached.clear_cache());
    
    let stats_after = cached.cache_stats().unwrap();
    assert_eq!(stats_after.entries, 0);
}

#[tokio::test]
async fn test_connection_cache_ext_owned() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    // Test with_query_cache on owned DatabaseConnection
    let cached = db.with_query_cache();
    
    assert!(cached.ping().await.is_ok());
}

#[tokio::test]
async fn test_connection_cache_ext_ref() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    // Test with_query_cache on &DatabaseConnection
    let cached = (&db).with_query_cache();
    
    assert!(cached.ping().await.is_ok());
}
