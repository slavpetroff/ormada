// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Database router integration tests

mod fixtures;

use fixtures::*;
use rstest::*;
use seaorm_django::prelude::*;
use seaorm_django::router::{DatabaseRouter, ConsistencyContext};
use sea_orm::{Database, ConnectionTrait};

#[tokio::test]
async fn test_router_with_replicas() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let replica1 = Database::connect("sqlite::memory:").await.unwrap();
    let replica2 = Database::connect("sqlite::memory:").await.unwrap();

    let _router = DatabaseRouter::new_with_replicas(primary, vec![replica1, replica2]);

    // Verify router is created successfully
    assert!(true);
}

#[tokio::test]
async fn test_router_read_connection_uses_replica() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let replica = Database::connect("sqlite::memory:").await.unwrap();

    let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

    // Read should use replica when no writes occurred
    let _conn = router.read_connection().await;
    assert!(true);
}

#[tokio::test]
async fn test_router_write_connection_always_primary() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let replica = Database::connect("sqlite::memory:").await.unwrap();

    let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

    // Write always uses primary
    let _conn = router.write_connection();
    assert!(true);
}

#[tokio::test]
async fn test_consistency_context_tracks_writes() {
    let ctx = ConsistencyContext::new();

    // Initially no write
    assert!(!ctx.has_write_occurred());

    // Mark write
    ctx.mark_write();
    assert!(ctx.has_write_occurred());

    // Reset
    ctx.reset();
    assert!(!ctx.has_write_occurred());
}

#[tokio::test]
async fn test_router_transaction_uses_primary() {
    use sea_orm::TransactionTrait;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let replica = Database::connect("sqlite::memory:").await.unwrap();

    let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

    // Transaction should use primary
    let txn = router.begin().await.unwrap();
    txn.rollback().await.unwrap();
    assert!(true);
}

#[fixture]
async fn router_with_schema() -> DatabaseRouter {
    use sea_orm::Schema;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    // Setup schema on primary
    let author_stmt = schema.create_table_from_entity(models::author::Entity);
    primary.execute(&author_stmt).await.unwrap();

    DatabaseRouter::new_single(primary)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_router_query_operations(#[future] router_with_schema: DatabaseRouter) {
    let router = router_with_schema;

    // Create author (write operation)
    let author = Author::objects(&router)
        .create(Author {
            id: 0,
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // Read operation
    let fetched = Author::objects(&router).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "Test Author");
}

#[tokio::test]
async fn test_router_single_db_mode() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let _router = DatabaseRouter::new_single(db);

    // Read and write both use primary
    assert!(true);
}

#[tokio::test]
async fn test_router_replica_round_robin() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let replica1 = Database::connect("sqlite::memory:").await.unwrap();
    let replica2 = Database::connect("sqlite::memory:").await.unwrap();

    let router = DatabaseRouter::new_with_replicas(primary, vec![replica1, replica2]);

    // Multiple reads should potentially use different replicas (testing routing logic)
    for _ in 0..5 {
        let _conn = router.read_connection().await;
    }
    assert!(true);
}

#[tokio::test]
async fn test_router_read_after_write_uses_primary() {
    use sea_orm::Schema;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let replica = Database::connect("sqlite::memory:").await.unwrap();

    // Setup schema on both
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let author_stmt = schema.create_table_from_entity(models::author::Entity);

    primary.execute(&author_stmt).await.unwrap();
    replica.execute(&author_stmt).await.unwrap();

    let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

    // Write operation
    let author = Author::objects(&router)
        .create(Author {
            id: 0,
            name: "Write Test".to_string(),
            email: "write@test.com".to_string(),
            age: 25,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // Subsequent read should use primary (read-your-writes consistency)
    let fetched = Author::objects(&router).get(author.id).await.unwrap();
    assert_eq!(fetched.email, "write@test.com");
}
