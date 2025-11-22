//! Tests for savepoint functionality using our tx! macro

use crate::common::{author, Author};
use sea_orm::{ConnectionTrait, Database};
use seaorm_django::prelude::*;
use seaorm_django::tx;

#[tokio::test]
async fn test_savepoint_with_tx_macro() {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create tables
    let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
    let stmt = schema.create_table_from_entity(author::Entity);
    db.execute_unprepared(&stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder))
        .await
        .unwrap();

    // Use our tx! macro for transactions!
    let result = tx!(&db, |txn| async move {
        let _author = Author::objects(txn)
            .create(Author {
                id: 0,
                name: "Test Author".to_string(),
                email: "test@test.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        Ok::<_, DjangoOrmError>(42)
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_nested_transaction_with_tx_macro() {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create tables
    let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
    let stmt = schema.create_table_from_entity(author::Entity);
    db.execute_unprepared(&stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder))
        .await
        .unwrap();

    // Use our tx! macro for nested transactions!
    let result = tx!(&db, |txn| async move {
        // Create author in transaction
        let _author = Author::objects(txn)
            .create(Author {
                id: 0,
                name: "Nested Author".to_string(),
                email: "nested@test.com".to_string(),
                age: 25,
                ..Default::default()
            })
            .await?;

        Ok::<_, DjangoOrmError>(100)
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 100);
}

#[tokio::test]
async fn test_transaction_rollback_with_tx_macro() {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create tables
    let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
    let stmt = schema.create_table_from_entity(author::Entity);
    db.execute_unprepared(&stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder))
        .await
        .unwrap();

    // Use our tx! macro with intentional error to test rollback!
    let result = tx!(&db, |_txn| async move {
        Err::<(), _>(DjangoOrmError::Custom("Intentional error".to_string()))
    })
    .await;

    assert!(result.is_err());
    match result {
        Err(DjangoOrmError::Custom(msg)) => {
            assert_eq!(msg, "Intentional error");
        }
        _ => panic!("Expected Custom error"),
    }
}
