// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Upsert operations integration tests

mod fixtures;

use fixtures::*;
use rstest::*;
use seaorm_django::prelude::*;

// ============================================================================
// Basic Upsert Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_insert_new_record(#[future] db: DatabaseRouter) {
    let author = Author {
        id: 100,
        name: "New Author".to_string(),
        email: "new@example.com".to_string(),
        age: 30,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    };

    let count = Author::objects(&db)
        .upsert_many(vec![author.clone()])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Email])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 1);

    let fetched = Author::objects(&db).get(100).await.unwrap();
    assert_eq!(fetched.name, "New Author");
    assert_eq!(fetched.email, "new@example.com");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_update_existing_record(
    #[future] db: DatabaseRouter,
    #[future] author: Author,
) {
    let mut updated = author.clone();
    updated.name = "Updated Name".to_string();
    updated.email = "updated@example.com".to_string();

    let count = Author::objects(&db)
        .upsert_many(vec![updated])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Email])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 1);

    let fetched = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "Updated Name");
    assert_eq!(fetched.email, "updated@example.com");
    assert_eq!(fetched.age, author.age); // Age should remain unchanged
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_mixed_insert_and_update(
    #[future] db: DatabaseRouter,
    #[future] sample_authors: Vec<Author>,
) {
    let alice = sample_authors.iter().find(|a| a.name == "Alice").unwrap();

    let records = vec![
        Author {
            id: alice.id,
            name: "Alice Updated".to_string(),
            email: "alice.updated@example.com".to_string(),
            age: alice.age,
            created_at: alice.created_at,
            updated_at: chrono::Utc::now().fixed_offset(),
        },
        Author {
            id: 999,
            name: "Brand New".to_string(),
            email: "brandnew@example.com".to_string(),
            age: 40,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        },
    ];

    let count = Author::objects(&db)
        .upsert_many(records)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Email])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 2);

    // Check update
    let alice_updated = Author::objects(&db).get(alice.id).await.unwrap();
    assert_eq!(alice_updated.name, "Alice Updated");

    // Check insert
    let new_author = Author::objects(&db).get(999).await.unwrap();
    assert_eq!(new_author.name, "Brand New");
}

// ============================================================================
// Bulk Upsert Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_bulk_all_new(#[future] db: DatabaseRouter) {
    let authors: Vec<Author> = (1..=10)
        .map(|i| Author {
            id: i * 100,
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 25 + i,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .collect();

    let count = Author::objects(&db)
        .upsert_many(authors)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 10);

    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 10);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_bulk_all_existing(
    #[future] db: DatabaseRouter,
    #[future] sample_authors: Vec<Author>,
) {
    let updated: Vec<Author> = sample_authors
        .iter()
        .map(|a| Author {
            id: a.id,
            name: format!("{} Updated", a.name),
            email: a.email.clone(),
            age: a.age + 10,
            created_at: a.created_at,
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .collect();

    let count = Author::objects(&db)
        .upsert_many(updated)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Age])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 3);

    let authors = Author::objects(&db).all().await.unwrap();
    assert!(authors.iter().all(|a| a.name.ends_with("Updated")));
}

#[rstest]
#[awt]
#[case(10)]
#[case(50)]
#[case(100)]
#[tokio::test]
async fn test_upsert_performance(#[future] db: DatabaseRouter, #[case] count: usize) {
    let authors: Vec<Author> = (0..count)
        .map(|i| Author {
            id: (i + 1) as i32,
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 25 + (i as i32 % 50),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .collect();

    let upserted = Author::objects(&db)
        .upsert_many(authors)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Email])
        .execute()
        .await
        .unwrap();

    assert_eq!(upserted, count as u64);

    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, count as u64);
}

// ============================================================================
// Field Selection Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_update_specific_fields_only(
    #[future] db_with_author: (DatabaseRouter, Author),
) {
    let (db, author) = db_with_author;
    let original_age = author.age;
    let original_email = author.email.clone();

    let updated = Author {
        id: author.id,
        name: "New Name".to_string(),
        email: "newemail@example.com".to_string(),
        age: 99,
        created_at: author.created_at,
        updated_at: chrono::Utc::now().fixed_offset(),
    };

    // Only update name, not email or age
    Author::objects(&db)
        .upsert_many(vec![updated])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    let fetched = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "New Name");
    assert_eq!(fetched.email, original_email); // Should remain unchanged
    assert_eq!(fetched.age, original_age); // Should remain unchanged
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_update_multiple_fields(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let original_email = author.email.clone();

    let updated = Author {
        id: author.id,
        name: "Updated Name".to_string(),
        email: "updated@example.com".to_string(),
        age: 50,
        created_at: author.created_at,
        updated_at: chrono::Utc::now().fixed_offset(),
    };

    Author::objects(&db)
        .upsert_many(vec![updated])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Age])
        .execute()
        .await
        .unwrap();

    let fetched = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "Updated Name");
    assert_eq!(fetched.age, 50);
    assert_eq!(fetched.email, original_email); // Should remain unchanged
}

// ============================================================================
// Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_empty_vec(#[future] db: DatabaseRouter) {
    let count = Author::objects(&db)
        .upsert_many(vec![])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_single_record(#[future] db: DatabaseRouter) {
    let author = Author {
        id: 42,
        name: "Single".to_string(),
        email: "single@example.com".to_string(),
        age: 30,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    };

    let count = Author::objects(&db)
        .upsert_many(vec![author])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 1);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_idempotent(#[future] db: DatabaseRouter) {
    let author = Author {
        id: 1,
        name: "Idempotent".to_string(),
        email: "test@example.com".to_string(),
        age: 30,
        created_at: chrono::Utc::now().fixed_offset(),
        updated_at: chrono::Utc::now().fixed_offset(),
    };

    // First upsert - insert
    Author::objects(&db)
        .upsert_many(vec![author.clone()])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    // Second upsert - update (but values are same)
    Author::objects(&db)
        .upsert_many(vec![author.clone()])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    // Third upsert
    Author::objects(&db)
        .upsert_many(vec![author.clone()])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 1); // Still only 1 record

    let fetched = Author::objects(&db).get(1).await.unwrap();
    assert_eq!(fetched.name, "Idempotent");
}

// ============================================================================
// Transaction Integration
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_in_transaction(#[future] db: DatabaseRouter) {
    let (count, total) = tx!(db, |txn| async move {
        let authors = vec![
            Author {
                id: 1,
                name: "Txn Author 1".to_string(),
                email: "txn1@example.com".to_string(),
                age: 25,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            },
            Author {
                id: 2,
                name: "Txn Author 2".to_string(),
                email: "txn2@example.com".to_string(),
                age: 30,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            },
        ];

        let count = Author::objects(txn)
            .upsert_many(authors)
            .on_conflict(Author::Id)
            .update_fields(&[Author::Name, Author::Email])
            .execute()
            .await?;

        let total = Author::objects(txn).count().await?;
        Ok((count, total))
    })
    .await
    .unwrap();

    assert_eq!(count, 2);
    assert_eq!(total, 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_rollback_on_error(#[future] db: DatabaseRouter, #[future] author: Author) {
    let result: Result<(), DjangoOrmError> = tx!(db, |txn| async move {
        // Upsert that would succeed
        Author::objects(txn)
            .upsert_many(vec![Author {
                id: 100,
                name: "Should Rollback".to_string(),
                email: "rollback@example.com".to_string(),
                age: 30,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            }])
            .on_conflict(Author::Id)
            .update_fields(&[Author::Name])
            .execute()
            .await?;

        // Force error
        Err(DjangoOrmError::Custom("Intentional error".to_string()))
    })
    .await;

    assert!(result.is_err());

    // Verify upsert was rolled back
    let result = Author::objects(&db).get(100).await;
    assert!(result.is_err());
}

// ============================================================================
// Upsert Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_single_then_update(#[future] db: DatabaseRouter) {
    // Insert new
    Author::objects(&db)
        .upsert_many(vec![Author {
            id: 999,
            name: "Single".to_string(),
            email: "single@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        }])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    let author = Author::objects(&db).get(999).await.unwrap();
    assert_eq!(author.name, "Single");

    // Update existing
    Author::objects(&db)
        .upsert_many(vec![Author {
            id: 999,
            name: "Updated Single".to_string(),
            email: "single@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        }])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    let author = Author::objects(&db).get(999).await.unwrap();
    assert_eq!(author.name, "Updated Single");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_with_empty_vec(#[future] db: DatabaseRouter) {
    let result = Author::objects(&db)
        .upsert_many(vec![])
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    assert_eq!(result, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_all_new_records(#[future] db: DatabaseRouter) {
    let authors: Vec<Author> = (1..=5)
        .map(|i| Author {
            id: i * 100,
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 25 + i,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .collect();

    Author::objects(&db)
        .upsert_many(authors)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 5);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_all_existing_records(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, sample_authors) = db_with_sample_authors;

    let mut updated_authors: Vec<Author> = sample_authors
        .into_iter()
        .map(|mut a| {
            a.name = format!("{} Updated", a.name);
            a
        })
        .collect();

    Author::objects(&db)
        .upsert_many(updated_authors)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name])
        .execute()
        .await
        .unwrap();

    let all_authors = Author::objects(&db).all().await.unwrap();
    assert_eq!(all_authors.len(), 3);
    assert!(all_authors.iter().all(|a| a.name.contains("Updated")));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_upsert_mixed_new_and_existing(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, existing_author) = db_with_author;

    let authors = vec![
        Author {
            id: existing_author.id,
            name: "Updated Existing".to_string(),
            email: existing_author.email.clone(),
            age: existing_author.age + 10,
            created_at: existing_author.created_at,
            updated_at: chrono::Utc::now().fixed_offset(),
        },
        Author {
            id: 888,
            name: "New Author".to_string(),
            email: "new@example.com".to_string(),
            age: 40,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        },
    ];

    Author::objects(&db)
        .upsert_many(authors)
        .on_conflict(Author::Id)
        .update_fields(&[Author::Name, Author::Age])
        .execute()
        .await
        .unwrap();

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 2);

    let updated = Author::objects(&db).get(existing_author.id).await.unwrap();
    assert_eq!(updated.name, "Updated Existing");
    assert_eq!(updated.age, existing_author.age + 10);

    let new = Author::objects(&db).get(888).await.unwrap();
    assert_eq!(new.name, "New Author");
}
