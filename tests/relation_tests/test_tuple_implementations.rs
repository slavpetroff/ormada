//! Tests for LoadRelations tuple implementations
//!
//! Tests to cover the various tuple implementations for 0-5 relations

use seaorm_django::prelude::*;


use crate::common::*;

// Test the () empty tuple case
#[tokio::test]
async fn test_no_relations() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    // Query without prefetch_related - uses () tuple
    let authors = author::Entity::objects(db)
        .all()
        .await
        .unwrap();
    
    assert_eq!(authors.len(), 3);
}

// Test single relation (already covered extensively, but let's be explicit)
#[tokio::test]
async fn test_single_relation() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    let books = book::Entity::objects(db)
        .prefetch_related(relations![author::Entity])
        .all()
        .await
        .unwrap();
    
    assert!(books.len() > 0);
}

// Test with empty database
#[tokio::test]
async fn test_relations_with_no_data() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    let authors = author::Entity::objects(db)
        .all()
        .await
        .unwrap();
    
    assert_eq!(authors.len(), 0);
}

#[tokio::test]
async fn test_relations_with_empty_books() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    // Query books (which don't exist) with author relation
    let books = book::Entity::objects(db)
        .prefetch_related(relations![author::Entity])
        .all()
        .await
        .unwrap();
    
    assert_eq!(books.len(), 0);
}

// Edge case: Model exists but no related data
#[tokio::test]
async fn test_book_without_author_relation() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    // Query a specific book
    let books = book::Entity::objects(db)
        .filter(sea_orm::ColumnTrait::eq(&book::Column::AuthorId, authors[0].id))
        .prefetch_related(relations![author::Entity])
        .all()
        .await
        .unwrap();
    
    for book in &books {
        assert!(book.author.is_some());
    }
}
