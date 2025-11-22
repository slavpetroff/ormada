//! Tests for bulk operations

use seaorm_django::prelude::*;

use crate::common::*;

// Helper to create a test author
async fn create_test_author(db: &DatabaseRouter) -> Author {
    Author::objects(db)
        .create(Author {
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap()
}

// ============================================================================
// bulk_create() Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_create_basic() {
    let db = setup_test_db().await;

    let models: Vec<Author> = (0..10)
        .map(|i| Author {
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 20 + i,
            ..Default::default()
        })
        .collect();

    let count = Author::objects(&db).bulk_create(models).await.unwrap();
    assert_eq!(count, 10);

    // Verify count
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 10);
}

#[tokio::test]
async fn test_bulk_create_empty() {
    let db = setup_test_db().await;

    let models: Vec<Author> = vec![];
    let count = Author::objects(&db).bulk_create(models).await.unwrap();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_bulk_create_large_batch() {
    let db = setup_test_db().await;

    let models: Vec<Author> = (0..100)
        .map(|i| Author {
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 20 + (i % 50),
            ..Default::default()
        })
        .collect();

    let count = Author::objects(&db).bulk_create(models).await.unwrap();
    assert_eq!(count, 100);

    // Verify all inserted
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 100);
}

#[tokio::test]
async fn test_bulk_create_with_relationships() {
    let db = setup_test_db().await;

    // Create author first
    let author = create_test_author(&db).await;

    // Bulk create books
    let books: Vec<Book> = (0..20)
        .map(|i| Book {
            title: format!("Book {}", i),
            author_id: author.id,
            price: 1000 + i * 100,
            published: true,
            ..Default::default()
        })
        .collect();

    let count = Book::objects(&db).bulk_create(books).await.unwrap();
    assert_eq!(count, 20);

    // Verify count
    let total = Book::objects(&db).count().await.unwrap();
    assert_eq!(total, 20);
}

#[tokio::test]
async fn test_bulk_create_single_record() {
    let db = setup_test_db().await;

    let models = vec![Author {
        name: "Single".to_string(),
        email: "single@example.com".to_string(),
        age: 30,
        ..Default::default()
    }];

    let count = Author::objects(&db).bulk_create(models).await.unwrap();
    assert_eq!(count, 1);

    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 1);
}

#[tokio::test]
async fn test_bulk_operations_in_transaction() {
    let db = setup_test_db().await;

    // Bulk create within transaction
    let count = tx!(db, |txn| async move {
        let models: Vec<Author> = (0..10)
            .map(|i| Author {
                name: format!("TX Author {}", i),
                email: format!("tx{}@example.com", i),
                age: 30 + i,
                ..Default::default()
            })
            .collect();

        // Use objects(txn) which now supports generics
        let count = Author::objects(txn).bulk_create(models).await?;
        Ok(count)
    })
    .await
    .unwrap();

    assert_eq!(count, 10);

    // Verify committed
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 10);
}

#[tokio::test]
async fn test_bulk_create_rollback_on_error() {
    let db = setup_test_db().await;

    let result: Result<(), DjangoOrmError> = tx!(db, |txn| async move {
        let models: Vec<Author> = (0..5)
            .map(|i| Author {
                name: format!("Rollback {}", i),
                email: format!("rollback{}@example.com", i),
                age: 30 + i,
                ..Default::default()
            })
            .collect();

        let count = Author::objects(txn).bulk_create(models).await?;
        assert_eq!(count, 5);

        // Force rollback
        Err(DjangoOrmError::Custom("Intentional rollback".into()))
    })
    .await;

    assert!(result.is_err());

    // Verify nothing was committed
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 0);
}

#[tokio::test]
async fn test_bulk_create_performance_comparison() {
    let db = setup_test_db().await;
    let count = 50;

    // Bulk insert
    let start = std::time::Instant::now();
    let models: Vec<Author> = (0..count)
        .map(|i| Author {
            name: format!("Bulk {}", i),
            email: format!("bulk{}@example.com", i),
            age: 25 + i,
            ..Default::default()
        })
        .collect();

    Author::objects(&db).bulk_create(models).await.unwrap();
    let bulk_duration = start.elapsed();

    // Verify
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, count as u64);

    println!("Bulk insert of {} records: {:?}", count, bulk_duration);
}

#[tokio::test]
async fn test_bulk_create_duplicate_handling() {
    let db = setup_test_db().await;

    // First batch
    let models1: Vec<Author> = (0..5)
        .map(|i| Author {
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 25,
            ..Default::default()
        })
        .collect();

    let count1 = Author::objects(&db).bulk_create(models1).await.unwrap();
    assert_eq!(count1, 5);

    // Second batch with different data
    let models2: Vec<Author> = (5..10)
        .map(|i| Author {
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 30,
            ..Default::default()
        })
        .collect();

    let count2 = Author::objects(&db).bulk_create(models2).await.unwrap();
    assert_eq!(count2, 5);

    // Verify total
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 10);
}
