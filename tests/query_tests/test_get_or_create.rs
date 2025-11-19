use super::common::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_get_or_create_creates_when_not_exists() {
    let db = setup_test_db().await;

    // Try to get_or_create with non-existent email
    let (author, created) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("newauthor@example.com"))
        .get_or_create(|| author::Model {
            name: "New Author".to_string(),
            email: "newauthor@example.com".to_string(),
            age: 25,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(created, "Should create new record");
    assert_eq!(author.name, "New Author");
    assert_eq!(author.email, "newauthor@example.com");

    // Verify it was actually created
    let found = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("newauthor@example.com"))
        .first()
        .await
        .unwrap();
    assert_eq!(found.id, author.id);
}

#[tokio::test]
async fn test_get_or_create_gets_when_exists() {
    let db = setup_test_db().await;

    // Create an author first
    let original = author::Entity::objects(&db)
        .create(author::Model {
            name: "Existing".to_string(),
            email: "existing@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    // Try to get_or_create - should get the existing one
    let (author, created) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("existing@example.com"))
        .get_or_create(|| author::Model {
            name: "Should Not Be Created".to_string(),
            email: "existing@example.com".to_string(),
            age: 99,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!created, "Should not create new record");
    assert_eq!(author.id, original.id);
    assert_eq!(author.name, "Existing"); // Original name, not the creator's name
}

#[tokio::test]
async fn test_update_or_create_creates_when_not_exists() {
    let db = setup_test_db().await;

    // Try to update_or_create with non-existent email
    let (author, created) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("brand_new@example.com"))
        .update_or_create(
            |model| {
                model.age = 35;
            },
            || author::Model {
                name: "Brand New".to_string(),
                email: "brand_new@example.com".to_string(),
                age: 28,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(created, "Should create new record");
    assert_eq!(author.name, "Brand New");
    assert_eq!(author.age, 28); // Creator value, updater not called
}

#[tokio::test]
async fn test_update_or_create_updates_when_exists() {
    let db = setup_test_db().await;

    // Create an author first
    let original = author::Entity::objects(&db)
        .create(author::Model {
            name: "To Update".to_string(),
            email: "toupdate@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    // Try to update_or_create - should update the existing one
    let (author, created) = author::Entity::objects(&db)
        .filter(author::Column::Email.eq("toupdate@example.com"))
        .update_or_create(
            |model| {
                model.age = 45;
            },
            || author::Model {
                name: "Should Not Be Used".to_string(),
                email: "toupdate@example.com".to_string(),
                age: 99,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(!created, "Should not create new record");
    assert_eq!(author.id, original.id);
    assert_eq!(author.age, 45); // Updated value
}
