//! Tests for aggregation operations
//!
//! Tests aggregate_count, aggregate_sum, aggregate_avg, aggregate_max, aggregate_min

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// Helper to create a test author
async fn create_test_author(db: &DatabaseRouter) -> Author {
    Author::objects(&db)
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
// aggregate_count() Tests
// ============================================================================

#[tokio::test]
async fn test_aggregate_count_all() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let count = Author::objects(&db).aggregate_count().await.unwrap();

    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_aggregate_count_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let count = Author::objects(&db)
        .filter(ColumnTrait::gt(&Author::Age, 30))
        .aggregate_count()
        .await
        .unwrap();

    assert!(count <= 3);
}

#[tokio::test]
async fn test_aggregate_count_empty() {
    let db = setup_test_db().await;

    let count = Author::objects(&db).aggregate_count().await.unwrap();

    assert_eq!(count, 0);
}

// ============================================================================
// aggregate_sum() Tests
// ============================================================================

#[tokio::test]
async fn test_aggregate_sum_basic() {
    let db = setup_test_db().await;

    // Create author first
    let author = create_test_author(&db).await;

    // Create books with known prices
    for (i, price) in [1000, 2000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let sum = Book::objects(&db).aggregate_sum(Book::Price).await.unwrap();

    assert_eq!(sum, Some(6000.0));
}

#[tokio::test]
async fn test_aggregate_sum_with_filter() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 2000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: *price >= 2000,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let sum = Book::objects(&db)
        .filter(ColumnTrait::eq(&Book::Published, true))
        .aggregate_sum(Book::Price)
        .await
        .unwrap();

    assert_eq!(sum, Some(5000.0)); // 2000 + 3000
}

#[tokio::test]
async fn test_aggregate_sum_empty() {
    let db = setup_test_db().await;

    let sum = Book::objects(&db).aggregate_sum(Book::Price).await.unwrap();

    assert_eq!(sum, None);
}

// ============================================================================
// aggregate_avg() Tests
// ============================================================================

#[tokio::test]
async fn test_aggregate_avg_basic() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 2000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let avg = Book::objects(&db).aggregate_avg(Book::Price).await.unwrap();

    assert_eq!(avg, Some(2000.0));
}

#[tokio::test]
async fn test_aggregate_avg_with_filter() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 2000, 3000, 4000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let avg = Book::objects(&db)
        .filter(ColumnTrait::gte(&Book::Price, 2000))
        .aggregate_avg(Book::Price)
        .await
        .unwrap();

    assert_eq!(avg, Some(3000.0)); // (2000 + 3000 + 4000) / 3
}

#[tokio::test]
async fn test_aggregate_avg_empty() {
    let db = setup_test_db().await;

    let avg = Book::objects(&db).aggregate_avg(Book::Price).await.unwrap();

    assert_eq!(avg, None);
}

// ============================================================================
// aggregate_max() Tests
// ============================================================================

#[tokio::test]
async fn test_aggregate_max_basic() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 5000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let max = Book::objects(&db).aggregate_max(Book::Price).await.unwrap();

    assert_eq!(max, Some(5000.0));
}

#[tokio::test]
async fn test_aggregate_max_with_filter() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 5000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let max = Book::objects(&db)
        .filter(ColumnTrait::lt(&Book::Price, 4000))
        .aggregate_max(Book::Price)
        .await
        .unwrap();

    assert_eq!(max, Some(3000.0));
}

#[tokio::test]
async fn test_aggregate_max_empty() {
    let db = setup_test_db().await;

    let max = Book::objects(&db).aggregate_max(Book::Price).await.unwrap();

    assert_eq!(max, None);
}

// ============================================================================
// aggregate_min() Tests
// ============================================================================

#[tokio::test]
async fn test_aggregate_min_basic() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 5000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let min = Book::objects(&db).aggregate_min(Book::Price).await.unwrap();

    assert_eq!(min, Some(1000.0));
}

#[tokio::test]
async fn test_aggregate_min_with_filter() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 5000, 3000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let min = Book::objects(&db)
        .filter(ColumnTrait::gt(&Book::Price, 2000))
        .aggregate_min(Book::Price)
        .await
        .unwrap();

    assert_eq!(min, Some(3000.0));
}

#[tokio::test]
async fn test_aggregate_min_empty() {
    let db = setup_test_db().await;

    let min = Book::objects(&db).aggregate_min(Book::Price).await.unwrap();

    assert_eq!(min, None);
}

// ============================================================================
// Combined Aggregation Tests
// ============================================================================

#[tokio::test]
async fn test_aggregations_comprehensive() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;
    let prices = vec![1000, 2000, 3000, 4000, 5000];

    for (i, price) in prices.iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Test all aggregations on the same data
    let count = Book::objects(&db).aggregate_count().await.unwrap();
    let sum = Book::objects(&db).aggregate_sum(Book::Price).await.unwrap();
    let avg = Book::objects(&db).aggregate_avg(Book::Price).await.unwrap();
    let max = Book::objects(&db).aggregate_max(Book::Price).await.unwrap();
    let min = Book::objects(&db).aggregate_min(Book::Price).await.unwrap();

    assert_eq!(count, 5);
    assert_eq!(sum, Some(15000.0));
    assert_eq!(avg, Some(3000.0));
    assert_eq!(max, Some(5000.0));
    assert_eq!(min, Some(1000.0));
}

#[tokio::test]
async fn test_aggregate_chain_operations() {
    let db = setup_test_db().await;

    let author = create_test_author(&db).await;

    for (i, price) in [1000, 2000, 3000, 4000, 5000].iter().enumerate() {
        Book::objects(&db)
            .create(Book {
                title: format!("Book {}", i),
                author_id: author.id,
                price: *price,
                published: *price >= 3000,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Chain filter -> aggregate
    let avg_published = Book::objects(&db)
        .filter(ColumnTrait::eq(&Book::Published, true))
        .aggregate_avg(Book::Price)
        .await
        .unwrap();

    assert_eq!(avg_published, Some(4000.0)); // (3000 + 4000 + 5000) / 3
}
