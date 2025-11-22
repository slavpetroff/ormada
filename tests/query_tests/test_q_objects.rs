//! Tests for Q object functionality
//!
//! Tests Q object construction and combination methods

use seaorm_django::prelude::*;
use seaorm_django::query::{ColumnExt, Q};

use crate::common::*;

// ============================================================================
// Q Object Creation
// ============================================================================

#[tokio::test]
async fn test_q_all() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all();
    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_q_any() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::any();
    let results = Author::objects(db).filter(q).all().await.unwrap();

    // Q::any() with no conditions should match nothing
    assert_eq!(results.len(), 0);
}

// ============================================================================
// Q Object Combinations
// ============================================================================

#[tokio::test]
async fn test_q_add_condition() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all().add(ColumnExt::eq(&Author::Id, authors[0].id));

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, authors[0].id);
}

#[tokio::test]
async fn test_q_add_multiple_conditions() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all()
        .add(ColumnExt::gt(&Author::Id, 0))
        .add(ColumnExt::lt(&Author::Age, 100));

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert!(results.len() > 0);
    for author in &results {
        assert!(author.id > 0);
        assert!(author.age < 100);
    }
}

#[tokio::test]
async fn test_q_not() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all().add(ColumnExt::eq(&Author::Id, authors[0].id)).not();

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(!results.iter().any(|a| a.id == authors[0].id));
}

// ============================================================================
// Complex Q Object Patterns
// ============================================================================

#[tokio::test]
async fn test_q_any_with_conditions() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::any()
        .add(ColumnExt::eq(&Author::Id, authors[0].id))
        .add(ColumnExt::eq(&Author::Id, authors[1].id));

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_q_all_negated() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all().add(ColumnExt::gt(&Author::Id, authors[2].id)).not();

    let results = Author::objects(db).filter(q).all().await.unwrap();

    // Should get all authors with id <= authors[2].id
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_q_with_like_pattern() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all().add(ColumnExt::contains(&Author::Name, "Bob"));

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].name.contains("Bob"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_q_empty_all() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all();
    let results = Author::objects(db).filter(q).all().await.unwrap();

    // Empty Q::all() should match everything
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_q_double_negation() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all().add(ColumnExt::eq(&Author::Id, authors[0].id)).not().not();

    let results = Author::objects(db).filter(q).all().await.unwrap();

    // Double negation should return to original condition
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, authors[0].id);
}

#[tokio::test]
async fn test_q_any_single_condition() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::any().add(ColumnExt::eq(&Author::Id, authors[0].id));

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, authors[0].id);
}

#[tokio::test]
async fn test_q_chain_multiple_adds() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let q = Q::all()
        .add(ColumnExt::gt(&Author::Id, 0))
        .add(ColumnExt::lt(&Author::Id, 1000))
        .add(ColumnExt::gt(&Author::Age, 0));

    let results = Author::objects(db).filter(q).all().await.unwrap();

    assert_eq!(results.len(), 3);
}
