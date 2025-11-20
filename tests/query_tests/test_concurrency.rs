//! Tests for concurrency safety in get_or_create and update_or_create

use crate::common::*;
use sea_orm::ColumnTrait;
use seaorm_django::query::QueryExt;
use seaorm_django::traits::DjangoEntity;
use tokio::task::JoinSet;

#[tokio::test]
async fn test_get_or_create_concurrent_safe() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Launch multiple concurrent get_or_create operations for the same email
    let mut tasks = JoinSet::new();
    
    for i in 0..5 {
        tasks.spawn(async move {
            let email = "concurrent@example.com";
            author::Entity::objects(db)
                .filter(author::Column::Email.eq(email))
                .get_or_create(|| author::Model {
                    id: 0,
                    name: format!("Concurrent Author {}", i),
                    email: email.to_string(),
                    age: 30,
                    created_at: Default::default(),
                    updated_at: Default::default(),
                })
                .await
        });
    }

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result.unwrap().unwrap());
    }

    // All should succeed
    assert_eq!(results.len(), 5);

    // Only one should have created=true, rest should be false
    let created_count = results.iter().filter(|(_, created)| *created).count();
    assert_eq!(created_count, 1, "Exactly one task should have created the record");

    // All should have the same ID
    let first_id = results[0].0.id;
    for (model, _) in &results {
        assert_eq!(model.id, first_id, "All tasks should get the same author");
    }

    // Verify only one record exists in DB
    let count = author::Entity::objects(db)
        .filter(author::Column::Email.eq("concurrent@example.com"))
        .count()
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_update_or_create_concurrent_safe() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Launch multiple concurrent update_or_create operations
    let mut tasks = JoinSet::new();
    
    for i in 0..5 {
        tasks.spawn(async move {
            let email = "update_concurrent@example.com";
            author::Entity::objects(db)
                .filter(author::Column::Email.eq(email))
                .update_or_create(
                    |model| {
                        model.age = 30 + i;
                    },
                    || author::Model {
                        id: 0,
                        name: format!("Update Concurrent {}", i),
                        email: email.to_string(),
                        age: 25,
                        created_at: Default::default(),
                        updated_at: Default::default(),
                    },
                )
                .await
        });
    }

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result.unwrap().unwrap());
    }

    // All should succeed
    assert_eq!(results.len(), 5);

    // Verify only one record exists
    let count = author::Entity::objects(db)
        .filter(author::Column::Email.eq("update_concurrent@example.com"))
        .count()
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_get_or_create_basic_still_works() {
    let db = setup_test_db().await;

    // Simple case - should work as before
    let (author, created) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("simple@example.com"))
        .get_or_create(|| author::Model {
            id: 0,
            name: "Simple Author".to_string(),
            email: "simple@example.com".to_string(),
            age: 25,
            created_at: Default::default(),
            updated_at: Default::default(),
        })
        .await
        .unwrap();

    assert!(created);
    assert_eq!(author.name, "Simple Author");

    // Call again - should get existing
    let (author2, created2) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("simple@example.com"))
        .get_or_create(|| author::Model {
            id: 0,
            name: "Different Name".to_string(),
            email: "simple@example.com".to_string(),
            age: 30,
            created_at: Default::default(),
            updated_at: Default::default(),
        })
        .await
        .unwrap();

    assert!(!created2);
    assert_eq!(author2.id, author.id);
    assert_eq!(author2.name, "Simple Author"); // Should keep original
}

#[tokio::test]
async fn test_update_or_create_basic_still_works() {
    let db = setup_test_db().await;

    // Create initial record
    let (author, created) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("update@example.com"))
        .update_or_create(
            |_model| {},
            || author::Model {
                id: 0,
                name: "Original Name".to_string(),
                email: "update@example.com".to_string(),
                age: 25,
                created_at: Default::default(),
                updated_at: Default::default(),
            },
        )
        .await
        .unwrap();

    assert!(created);
    assert_eq!(author.age, 25);

    // Update it
    let (author2, created2) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("update@example.com"))
        .update_or_create(
            |model| {
                model.age = 35;
            },
            || author::Model {
                id: 0,
                name: "Fallback Name".to_string(),
                email: "update@example.com".to_string(),
                age: 999,
                created_at: Default::default(),
                updated_at: Default::default(),
            },
        )
        .await
        .unwrap();

    assert!(!created2);
    assert_eq!(author2.age, 35);
}
