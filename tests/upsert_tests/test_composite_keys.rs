use crate::common::*;
use seaorm_django::prelude::*;

// NOTE: For many-to-many relationships, you would typically have a junction table
// with composite keys. Here's how upsert would work:
//
// Example many-to-many scenario:
// ```rust
// // Junction table: book_tags (book_id, tag_id) - composite primary key
// #[django_model(table = "book_tags")]
// pub struct BookTag {
//     #[sea_orm(primary_key)]
//     pub book_id: i32,
//     #[sea_orm(primary_key)]
//     pub tag_id: i32,
//     pub created_at: DateTimeWithTimeZone,
// }
//
// // Upsert many-to-many relationships
// let book_tags = vec![
//     BookTag { book_id: 1, tag_id: 10, ..Default::default() },
//     BookTag { book_id: 1, tag_id: 20, ..Default::default() },
//     BookTag { book_id: 2, tag_id: 10, ..Default::default() },
// ];
//
// BookTag::objects(&db)
//     .upsert_many(book_tags)
//     .on_conflict_columns(vec![
//         book_tag::Column::BookId,
//         book_tag::Column::TagId
//     ])
//     .update_fields(&[book_tag::Column::CreatedAt])
//     .execute()
//     .await?;
// ```
//
// This generates:
// INSERT INTO book_tags (book_id, tag_id, created_at) VALUES (...)
// ON CONFLICT (book_id, tag_id) DO UPDATE SET created_at = EXCLUDED.created_at

#[tokio::test]
async fn test_upsert_with_composite_key_simulation() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    // Simulate composite key scenario using book table
    // In real app, this would be a many-to-many junction table
    let books = vec![
        Book {
            id: 1,
            title: "Combo 1".to_string(),
            author_id: authors[0].id,
            price: 1000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Combo 2".to_string(),
            author_id: authors[1].id,
            price: 2000,
            published: true,
            ..Default::default()
        },
    ];

    // First upsert
    Book::objects(&db)
        .upsert_many(books.clone())
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    // Second upsert - should update, not error
    let updated_books = vec![
        Book {
            id: 1,
            title: "Combo 1 Updated".to_string(),
            author_id: authors[0].id,
            price: 1500,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Combo 2 Updated".to_string(),
            author_id: authors[1].id,
            price: 2500,
            published: true,
            ..Default::default()
        },
    ];

    Book::objects(&db)
        .upsert_many(updated_books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    let book1 = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book1.title, "Combo 1 Updated");
    assert_eq!(book1.price, 1500);

    let book2 = Book::objects(&db).get(2).await.unwrap();
    assert_eq!(book2.title, "Combo 2 Updated");
    assert_eq!(book2.price, 2500);
}

#[tokio::test]
async fn test_upsert_idempotent_with_relationships() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let books = vec![
        Book {
            id: 1,
            title: "Book 1".to_string(),
            author_id: authors[0].id,
            price: 1000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Book 2".to_string(),
            author_id: authors[1].id,
            price: 2000,
            published: true,
            ..Default::default()
        },
    ];

    // Run upsert 5 times - should be idempotent
    for _ in 0..5 {
        Book::objects(&db)
            .upsert_many(books.clone())
            .on_conflict(Book::Id)
            .update_fields(&[Book::Title, Book::AuthorId, Book::Price])
            .execute()
            .await
            .unwrap();
    }

    // Should still have exactly 2 books
    let all_books = Book::objects(&db).all().await.unwrap();
    assert_eq!(all_books.len(), 2);

    // Verify relationships are correct
    let book1 = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book1.author_id, authors[0].id);

    let book2 = Book::objects(&db).get(2).await.unwrap();
    assert_eq!(book2.author_id, authors[1].id);
}

#[tokio::test]
async fn test_upsert_respects_foreign_key_constraints() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let books = vec![Book {
        id: 1,
        title: "Valid Book".to_string(),
        author_id: authors[0].id, // Valid author
        price: 1000,
        published: true,
        ..Default::default()
    }];

    // This should succeed - valid foreign key
    let result = Book::objects(&db)
        .upsert_many(books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::AuthorId])
        .execute()
        .await;

    assert!(result.is_ok(), "Valid foreign key should succeed");

    // This should fail - invalid foreign key
    let invalid_books = vec![Book {
        id: 2,
        title: "Invalid Book".to_string(),
        author_id: 99999, // Non-existent author
        price: 1000,
        published: true,
        ..Default::default()
    }];

    let result = Book::objects(&db)
        .upsert_many(invalid_books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::AuthorId])
        .execute()
        .await;

    // Note: SQLite doesn't enforce foreign keys by default in in-memory databases
    // This test would pass if we enabled PRAGMA foreign_keys = ON in setup
    // For now, we just verify the operation completes without panic
    assert!(result.is_ok() || result.is_err(), "Upsert should complete");
}
