//! Comprehensive aggregation tests to achieve 95%+ coverage

use seaorm_django::prelude::*;
use seaorm_django::query::QueryExt;
use seaorm_django::traits::DjangoEntity;
use seaorm_django::aggregations::AggregateExt; // For sum, avg, max, min
use crate::common::{author, book, setup_test_db, Author, Book};

#[tokio::test]
async fn test_sum_on_empty_table() {
    let db = setup_test_db().await;
    
    // Test sum on empty table - should return None using our ORM!
    let result = Author::objects(&db)
        .aggregate_sum(author::Column::Age)
        .await
        .unwrap();
    
    assert_eq!(result, None, "Sum on empty table should be None");
}

#[tokio::test]
async fn test_avg_on_empty_table() {
    let db = setup_test_db().await;
    
    // Test avg on empty table - should return None using our ORM!
    let result = Author::objects(&db)
        .aggregate_avg(author::Column::Age)
        .await
        .unwrap();
    
    assert_eq!(result, None, "Avg on empty table should be None");
}

#[tokio::test]
async fn test_max_on_empty_table() {
    let db = setup_test_db().await;
    
    // Test max on empty table - should return None using our ORM!
    let result = Author::objects(&db)
        .aggregate_max(author::Column::Age)
        .await
        .unwrap();
    
    assert_eq!(result, None, "Max on empty table should be None");
}

#[tokio::test]
async fn test_min_on_empty_table() {
    let db = setup_test_db().await;
    
    // Test min on empty table - should return None using our ORM!
    let result = Author::objects(&db)
        .aggregate_min(author::Column::Age)
        .await
        .unwrap();
    
    assert_eq!(result, None, "Min on empty table should be None");
}

#[tokio::test]
async fn test_sum_on_float_column() {
    let db = setup_test_db().await;
    
    // Create author first to satisfy foreign key constraint
    let author = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Test Author".to_string(),
        email: "test@example.com".to_string(),
        age: 30,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    // Create test data using our ORM's objects API!
    let _book1 = Book::objects(&db).create(book::Model {
        id: 0,
        title: "Book 1".to_string(),
        author_id: author.id,
        price: 1999, // $19.99
        published: true,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    let _book2 = Book::objects(&db).create(book::Model {
        id: 0,
        title: "Book 2".to_string(),
        author_id: author.id,
        price: 2999, // $29.99
        published: true,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    // Test sum on numeric column
    let result = Book::objects(&db)
        .aggregate_sum(book::Column::Price)
        .await
        .unwrap();
    
    assert!(result.is_some(), "Sum should return Some value");
    assert_eq!(result.unwrap(), 4998.0, "Sum should be correct");
}

#[tokio::test]
async fn test_avg_returns_float() {
    let db = setup_test_db().await;
    
    // Create test authors with varying ages using our ORM!
    let _author1 = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Author 1".to_string(),
        email: "author1@test.com".to_string(),
        age: 30,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    let _author2 = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Author 2".to_string(),
        email: "author2@test.com".to_string(),
        age: 40,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    // Test avg - should return float
    let result = Author::objects(&db)
        .aggregate_avg(author::Column::Age)
        .await
        .unwrap();
    
    assert!(result.is_some(), "Avg should return Some value");
    assert_eq!(result.unwrap(), 35.0, "Avg should be 35.0");
}

#[tokio::test]
async fn test_max_with_multiple_values() {
    let db = setup_test_db().await;
    
    // Create test authors using our ORM!
    let _author1 = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Young Author".to_string(),
        email: "young@test.com".to_string(),
        age: 25,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    let _author2 = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Old Author".to_string(),
        email: "old@test.com".to_string(),
        age: 65,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    // Test max
    let result = Author::objects(&db)
        .aggregate_max(author::Column::Age)
        .await
        .unwrap();
    
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 65.0);
}

#[tokio::test]
async fn test_min_with_multiple_values() {
    let db = setup_test_db().await;
    
    // Create test authors using our ORM!
    let _author1 = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Young Author".to_string(),
        email: "young@test.com".to_string(),
        age: 25,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    let _author2 = Author::objects(&db).create(author::Model {
        id: 0,
        name: "Old Author".to_string(),
        email: "old@test.com".to_string(),
        age: 65,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    })
    .await
    .unwrap();
    
    // Test min
    let result = Author::objects(&db)
        .aggregate_min(author::Column::Age)
        .await
        .unwrap();
    
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 25.0);
}

#[tokio::test]
async fn test_aggregations_with_filter() {
    let db = setup_test_db().await;
    
    // Create test authors using our ORM!
    for i in 1..=5 {
        Author::objects(&db).create(author::Model {
            id: 0,
            name: format!("Author {}", i),
            email: format!("author{}@test.com", i),
            age: 20 + i * 10,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();
    }
    
    // Test sum with filter (only authors with age > 40)
    let result = Author::objects(&db)
        .filter(author::Column::Age.gt(40))
        .aggregate_sum(author::Column::Age)
        .await
        .unwrap();
    
    assert!(result.is_some());
    // Ages: 30, 40, 50, 60, 70 -> filter > 40 = 50, 60, 70 = 180
    assert_eq!(result.unwrap(), 180.0);
}
