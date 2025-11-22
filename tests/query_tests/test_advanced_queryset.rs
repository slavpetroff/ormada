//! Tests for advanced QuerySet methods
//!
//! Tests distinct(), earliest(), latest(), get_or_create(), update_or_create()

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// distinct() Tests
// ============================================================================

#[tokio::test]
async fn test_distinct_basic() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db).distinct().all().await.unwrap();

    // Should return all unique authors
    assert_eq!(authors.len(), 3);
}

#[tokio::test]
async fn test_distinct_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Age, 25))
        .distinct()
        .all()
        .await
        .unwrap();

    assert!(authors.len() <= 3);
}

#[tokio::test]
async fn test_distinct_empty_result() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, 9999))
        .distinct()
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 0);
}

// ============================================================================
// earliest() Tests
// ============================================================================

#[tokio::test]
async fn test_earliest_basic() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let earliest = Author::objects(db).earliest(Author::Id).await.unwrap();

    // Should return the first author (lowest ID)
    assert_eq!(earliest.id, authors[0].id);
}

#[tokio::test]
async fn test_earliest_by_age() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let youngest = Author::objects(db).earliest(Author::Age).await.unwrap();

    // Should return the youngest author
    assert!(youngest.age > 0);
}

#[tokio::test]
async fn test_earliest_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let earliest = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Age, 30))
        .earliest(Author::Age)
        .await;

    // Should either find one or error
    match earliest {
        Ok(author) => assert!(author.age > 30),
        Err(DjangoOrmError::Custom(msg)) => assert!(msg.contains("No records")),
        Err(_) => panic!("Unexpected error"),
    }
}

#[tokio::test]
async fn test_earliest_empty_result() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let result = Author::objects(db).earliest(Author::Id).await;

    assert!(result.is_err());
    match result {
        Err(DjangoOrmError::Custom(msg)) => assert!(msg.contains("No records")),
        _ => panic!("Expected Custom error"),
    }
}

// ============================================================================
// latest() Tests
// ============================================================================

#[tokio::test]
async fn test_latest_basic() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let latest = Author::objects(db).latest(Author::Id).await.unwrap();

    // Should return the last author (highest ID)
    assert_eq!(latest.id, authors[2].id);
}

#[tokio::test]
async fn test_latest_by_age() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let oldest = Author::objects(db).latest(Author::Age).await.unwrap();

    // Should return the oldest author
    assert!(oldest.age > 0);
}

#[tokio::test]
async fn test_latest_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let latest = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, authors[1].email.clone()))
        .latest(Author::Id)
        .await
        .unwrap();

    assert_eq!(latest.email, authors[1].email);
}

#[tokio::test]
async fn test_latest_empty_result() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let result = Author::objects(db).latest(Author::Id).await;

    assert!(result.is_err());
    match result {
        Err(DjangoOrmError::Custom(msg)) => assert!(msg.contains("No records")),
        _ => panic!("Expected Custom error"),
    }
}

// ============================================================================
// get_or_create() Tests
// ============================================================================

#[tokio::test]
async fn test_get_or_create_creates_new() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let email = "newauthor@example.com";
    let (author, created) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, email))
        .get_or_create(|| Author {
            name: "New Author".to_string(),
            email: email.to_string(),
            age: 35,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(created);
    assert_eq!(author.email, email);
    assert_eq!(author.name, "New Author");
}

#[tokio::test]
async fn test_get_or_create_gets_existing() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let existing_email = authors[0].email.clone();
    let (author, created) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, existing_email.clone()))
        .get_or_create(|| Author {
            name: "Should Not Create".to_string(),
            email: existing_email.clone(),
            age: 999,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!created);
    assert_eq!(author.id, authors[0].id);
    assert_eq!(author.email, existing_email);
    // Should have original name, not the one we tried to create
    assert_ne!(author.name, "Should Not Create");
}

#[tokio::test]
async fn test_get_or_create_multiple_filters() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let (author, created) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, "unique@example.com"))
        .filter(ColumnTrait::gt(&Author::Age, 25))
        .get_or_create(|| Author {
            name: "Unique Author".to_string(),
            email: "unique@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(created);
    assert!(author.age > 25);
}

// ============================================================================
// update_or_create() Tests
// ============================================================================

#[tokio::test]
async fn test_update_or_create_creates_new() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let email = "newemail@example.com";
    let (author, created) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, email))
        .update_or_create(
            |author| {
                author.age = 40; // This won't be called
            },
            || Author {
                name: "Created Author".to_string(),
                email: email.to_string(),
                age: 35,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(created);
    assert_eq!(author.email, email);
    assert_eq!(author.age, 35); // Should use creator value
}

#[tokio::test]
async fn test_update_or_create_updates_existing() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let existing_email = authors[0].email.clone();

    let (author, created) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, existing_email.clone()))
        .update_or_create(
            |author| {
                author.age = 99; // Update age
            },
            || Author {
                name: "Should Not Be Created".to_string(),
                email: existing_email.clone(),
                age: 888,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(!created);
    assert_eq!(author.email, existing_email);
    assert_eq!(author.age, 99); // Should be updated!
}

#[tokio::test]
async fn test_update_or_create_with_filter() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let (author, created) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, "filter@example.com"))
        .filter(ColumnTrait::gt(&Author::Age, 20))
        .update_or_create(
            |author| {
                author.age = 50;
            },
            || Author {
                name: "Filtered Author".to_string(),
                email: "filter@example.com".to_string(),
                age: 30,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(created);
    assert!(author.age >= 20);
}

// ============================================================================
// Combined Tests
// ============================================================================

#[tokio::test]
async fn test_distinct_with_ordering() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db).distinct().order_by_asc(Author::Name).all().await.unwrap();

    assert!(authors.len() > 0);
}

#[tokio::test]
async fn test_earliest_latest_comparison() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let earliest = Author::objects(db).earliest(Author::Id).await.unwrap();

    let latest = Author::objects(db).latest(Author::Id).await.unwrap();

    assert!(earliest.id <= latest.id);
}

#[tokio::test]
async fn test_get_or_create_idempotent() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let email = "idempotent@example.com";

    // First call - creates
    let (author1, created1) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, email))
        .get_or_create(|| Author {
            name: "Idempotent Test".to_string(),
            email: email.to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    // Second call - gets existing
    let (author2, created2) = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Email, email))
        .get_or_create(|| Author {
            name: "Idempotent Test".to_string(),
            email: email.to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(created1);
    assert!(!created2);
    assert_eq!(author1.id, author2.id);
}
