//! Tests for #[atomic] macro

use seaorm_django::prelude::*;

use crate::common::*;

// Test macro with explicit concrete type (Clean UX for end users)
#[atomic(db)]
async fn create_author_atomic(
    db: &DatabaseRouter,
    name: String,
) -> Result<Author, DjangoOrmError> {
    // This uses Entity::objects(db).create which handles auto fields
    let author = Author::objects(db)
        .create(Author {
            name,
            email: "atomic@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await?;
    Ok(author)
}

// Helper for nested tests - needs generics to accept both Connection and Transaction
// We use DjangoConnection alias to make it cleaner
#[atomic(db)]
async fn create_author_nested<C>(db: &C, name: String) -> Result<Author, DjangoOrmError>
where
    C: DjangoConnection,
{
    let author = Author::objects(db)
        .create(Author {
            name,
            email: "nested@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await?;
    Ok(author)
}

#[tokio::test]
async fn test_atomic_macro_success() {
    let db = setup_test_db().await;

    let result = create_author_atomic(&db, "Atomic Author".to_string()).await;
    assert!(result.is_ok());

    let author = result.unwrap();
    assert_eq!(author.name, "Atomic Author");
    assert!(author.id > 0); // ID should be auto-generated

    // Verify it persists using OUR api
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 1);
}

#[atomic(db)]
async fn create_author_atomic_fail(db: &DatabaseRouter) -> Result<(), DjangoOrmError> {
    Author::objects(db)
        .create(Author {
            name: "Rollback".to_string(),
            email: "rollback@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await?;

    // Force rollback
    Err(DjangoOrmError::Custom("Fail".into()))
}

#[tokio::test]
async fn test_atomic_macro_rollback() {
    let db = setup_test_db().await;

    let result = create_author_atomic_fail(&db).await;
    assert!(result.is_err());

    // Verify rollback using OUR api
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

// Test nested transactions
#[atomic(db)]
async fn nested_atomic(db: &DatabaseRouter) -> Result<(), DjangoOrmError> {
    // Outer insert
    Author::objects(db)
        .create(Author {
            name: "Outer".to_string(),
            email: "outer@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await?;

    // Inner atomic call (must be generic to accept transaction)
    create_author_nested(db, "Inner".to_string()).await?;

    Ok(())
}

#[tokio::test]
async fn test_atomic_macro_nested() {
    let db = setup_test_db().await;

    let result = nested_atomic(&db).await;
    assert!(result.is_ok());

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 2);
}
