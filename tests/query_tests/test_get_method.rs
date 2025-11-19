//! Tests for the .get() method
//!
//! Tests error handling and edge cases for get()

use seaorm_django::prelude::*;


use crate::common::*;

#[tokio::test]
async fn test_get_by_id_found() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    let author = author::Entity::objects(db)
        .get(authors[0].id)
        .await
        .unwrap();
    
    assert_eq!(author.id, authors[0].id);
    assert_eq!(author.name, authors[0].name);
}

#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    let result = author::Entity::objects(db)
        .get(99999)
        .await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_with_filter_ignored() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    // get() ignores filters and just uses the ID
    let author = author::Entity::objects(db)
        .filter(sea_orm::ColumnTrait::eq(&author::Column::Age, 999))
        .get(authors[0].id)
        .await
        .unwrap();
    
    assert_eq!(author.id, authors[0].id);
}
