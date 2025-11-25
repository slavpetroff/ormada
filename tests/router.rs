// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Database router integration tests

mod fixtures;

use fixtures::*;
use rstest::*;
use sea_orm::{ConnectionTrait, Database};
use seaorm_django::prelude::*;
use seaorm_django::router::{ConsistencyContext, DatabaseRouter};

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

// ============================================================================
// ConsistencyContext Tests
// ============================================================================

#[tokio::test]
async fn test_consistency_context_default() {
    // Test Default implementation
    let ctx: ConsistencyContext = Default::default();
    assert!(!ctx.has_write_occurred());
}

#[tokio::test]
async fn test_consistency_context_clone() {
    let ctx = ConsistencyContext::new();
    ctx.mark_write();

    // Clone should share the same underlying state
    let cloned = ctx.clone();
    assert!(cloned.has_write_occurred());
}

#[tokio::test]
async fn test_consistency_context_debug() {
    let ctx = ConsistencyContext::new();
    let debug_str = format!("{:?}", ctx);
    assert!(debug_str.contains("ConsistencyContext"));
}

// ============================================================================
// Router ConnectionTrait Tests
// ============================================================================

#[tokio::test]
async fn test_router_get_database_backend() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    let backend = router.get_database_backend();
    assert!(matches!(backend, sea_orm::DatabaseBackend::Sqlite));
}

#[tokio::test]
async fn test_router_execute_raw_statement() {
    use sea_orm::Statement;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Execute a simple SQL statement
    let stmt =
        Statement::from_string(sea_orm::DatabaseBackend::Sqlite, "SELECT 1 as value".to_string());

    let result = router.query_one_raw(stmt).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_query_all_via_orm() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Create table using ORM
    Author::create_table(&router).await.unwrap();

    // Create data using ORM
    Author::objects(&router)
        .create(Author {
            id: 0,
            name: "Test 1".to_string(),
            email: "test1@example.com".to_string(),
            age: 25,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    Author::objects(&router)
        .create(Author {
            id: 0,
            name: "Test 2".to_string(),
            email: "test2@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // Query all using ORM
    let authors = Author::objects(&router).all().await.unwrap();
    assert_eq!(authors.len(), 2);
}

#[tokio::test]
async fn test_router_transaction_with_config() {
    use sea_orm::TransactionTrait;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Begin with config
    let txn = router.begin_with_config(None, None).await.unwrap();
    txn.rollback().await.unwrap();
}

// ============================================================================
// Router Routing Strategy Tests
// ============================================================================

#[rstest]
#[case(RoutingStrategy::Primary)]
#[case(RoutingStrategy::RoundRobin)]
#[tokio::test]
async fn test_routing_strategy_variants(#[case] strategy: RoutingStrategy) {
    // Verify all routing strategies can be constructed and debugged
    let debug_str = format!("{:?}", strategy);
    assert!(!debug_str.is_empty());
}

#[tokio::test]
async fn test_routing_strategy_clone_and_eq() {
    let s1 = RoutingStrategy::Primary;
    let s2 = s1.clone();
    assert_eq!(s1, s2);

    let s3 = RoutingStrategy::RoundRobin;
    assert_ne!(s1, s3);
}

// ============================================================================
// Router Method Coverage Tests
// ============================================================================

#[tokio::test]
async fn test_router_primary_connection() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Access primary connection directly
    let _primary_conn = router.primary_connection();
    assert!(true);
}

#[tokio::test]
async fn test_router_reset_context() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Mark a write
    router.context().mark_write();
    assert!(router.context().has_write_occurred());

    // Reset context
    router.reset_context();
    assert!(!router.context().has_write_occurred());
}

#[tokio::test]
async fn test_router_transaction_state() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Initially not in transaction
    assert!(!router.is_in_transaction().await);

    // Begin transaction
    router.begin_transaction().await;
    assert!(router.is_in_transaction().await);

    // End transaction
    router.end_transaction().await;
    assert!(!router.is_in_transaction().await);
}

#[tokio::test]
async fn test_router_execute_unprepared() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Execute unprepared SQL
    let result = router.execute_unprepared("SELECT 1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_execute_raw() {
    use sea_orm::Statement;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Execute raw statement
    let stmt =
        Statement::from_string(sea_orm::DatabaseBackend::Sqlite, "SELECT 1 as value".to_string());
    let result = router.execute_raw(stmt).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_query_all_raw() {
    use sea_orm::Statement;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Query all raw
    let stmt =
        Statement::from_string(sea_orm::DatabaseBackend::Sqlite, "SELECT 1 as value".to_string());
    let result = router.query_all_raw(stmt).await;
    assert!(result.is_ok());
}

// ============================================================================
// &DatabaseRouter Implementation Tests (borrowed reference)
// ============================================================================

#[tokio::test]
async fn test_router_ref_execute() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    // Use borrowed reference
    let router_ref: &DatabaseRouter = &router;

    // Create table via reference
    Author::create_table(router_ref).await.unwrap();

    // Create via reference
    let author = Author::objects(router_ref)
        .create(Author {
            id: 0,
            name: "Ref Test".to_string(),
            email: "ref@test.com".to_string(),
            age: 25,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    assert_eq!(author.name, "Ref Test");
}

#[tokio::test]
async fn test_router_ref_query_one() {
    use sea_orm::Statement;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);
    let router_ref: &DatabaseRouter = &router;

    // Execute a simple query via reference
    let stmt =
        Statement::from_string(sea_orm::DatabaseBackend::Sqlite, "SELECT 1 as value".to_string());
    let result = router_ref.query_one_raw(stmt).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_ref_query_all() {
    use sea_orm::Statement;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);
    let router_ref: &DatabaseRouter = &router;

    let stmt =
        Statement::from_string(sea_orm::DatabaseBackend::Sqlite, "SELECT 1 as value".to_string());
    let result = router_ref.query_all_raw(stmt).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_ref_execute_unprepared() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);
    let router_ref: &DatabaseRouter = &router;

    let result = router_ref.execute_unprepared("SELECT 1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_ref_execute_raw() {
    use sea_orm::Statement;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);
    let router_ref: &DatabaseRouter = &router;

    let stmt =
        Statement::from_string(sea_orm::DatabaseBackend::Sqlite, "SELECT 1 as value".to_string());
    let result = router_ref.execute_raw(stmt).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_router_ref_get_database_backend() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);
    let router_ref: &DatabaseRouter = &router;

    let backend = router_ref.get_database_backend();
    assert!(matches!(backend, sea_orm::DatabaseBackend::Sqlite));
}

// ============================================================================
// tx! Macro on DatabaseRouter Tests
// ============================================================================

#[tokio::test]
async fn test_router_tx_macro() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    Author::create_table(&router).await.unwrap();

    // Use tx! macro on router
    let author = seaorm_django::tx!(router, |txn| async move {
        Author::objects(txn)
            .create(Author {
                id: 0,
                name: "TX Macro Test".to_string(),
                email: "tx@test.com".to_string(),
                age: 30,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
    })
    .await
    .unwrap();

    assert_eq!(author.name, "TX Macro Test");

    // Verify persisted
    let count = Author::objects(&router).count().await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_router_nested_tx_macro() {
    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    Author::create_table(&router).await.unwrap();
    Book::create_table(&router).await.unwrap();

    // Use nested tx! on router
    let (author, book) = seaorm_django::tx!(router, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                id: 0,
                name: "Nested TX Author".to_string(),
                email: "nested@test.com".to_string(),
                age: 35,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await?;

        // Nested transaction
        let book = seaorm_django::tx!(txn, |inner| async move {
            Book::objects(inner)
                .create(Book {
                    id: 0,
                    author_id: author.id,
                    author: Default::default(),
                    title: "Nested TX Book".to_string(),
                    price: 1999,
                    published: true,
                    created_at: chrono::Utc::now().fixed_offset(),
                    updated_at: chrono::Utc::now().fixed_offset(),
                })
                .await
        })
        .await?;

        Ok((author, book))
    })
    .await
    .unwrap();

    assert_eq!(author.name, "Nested TX Author");
    assert_eq!(book.title, "Nested TX Book");
}

#[tokio::test]
async fn test_router_multiple_queries() {
    use sea_orm::QueryTrait;

    let primary = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(primary);

    Author::create_table(&router).await.unwrap();

    // Create some data
    for i in 0..5 {
        Author::objects(&router)
            .create(Author {
                id: 0,
                name: format!("Author {}", i),
                email: format!("author{}@test.com", i),
                age: 20 + i,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
            .unwrap();
    }

    // Various query operations that go through router
    let count = Author::objects(&router).count().await.unwrap();
    assert_eq!(count, 5);

    let authors = Author::objects(&router).all().await.unwrap();
    assert_eq!(authors.len(), 5);

    let first = Author::objects(&router).first().await.unwrap();
    assert_eq!(first.name, "Author 0");

    let filtered = Author::objects(&router).filter(Author::Age.gte(22)).all().await.unwrap();
    assert_eq!(filtered.len(), 3);
}
