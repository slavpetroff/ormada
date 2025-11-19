//! Additional tests to achieve 97%+ coverage for upsert methods

use seaorm_django::prelude::*;
use sea_orm::ColumnTrait;


use crate::common::*;

// Test the insert path of update_or_create (currently uncovered)
#[tokio::test]
async fn test_update_or_create_insert_path_detailed() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    let email = "newuser@example.com";
    
    // This should trigger the insert path
    let (author, created) = author::Entity::objects(db)
        .filter(ColumnTrait::eq(&author::Column::Email, email))
        .update_or_create(
            |_author| {
                // This won't be called since record doesn't exist
            },
            || {
                author::Model {
                    name: "New User".to_string(),
                    email: email.to_string(),
                    age: 28,
                    ..Default::default()
                }
            },
        )
        .await
        .unwrap();
    
    assert!(created);
    assert_eq!(author.email, email);
    assert_eq!(author.name, "New User");
}

// Test get_or_create insert path
#[tokio::test]
async fn test_get_or_create_insert_path_detailed() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));
    
    let email = "created@example.com";
    
    // This should trigger the insert path
    let (author, created) = author::Entity::objects(db)
        .filter(ColumnTrait::eq(&author::Column::Email, email))
        .get_or_create(|| {
            author::Model {
                name: "Created User".to_string(),
                email: email.to_string(),
                age: 32,
                ..Default::default()
            }
        })
        .await
        .unwrap();
    
    assert!(created);
    assert_eq!(author.email, email);
}
