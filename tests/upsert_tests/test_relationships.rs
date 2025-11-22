use crate::common::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_upsert_with_foreign_key_relationships() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    // Upsert books with different authors
    let books = vec![
        Book {
            id: 1,
            title: "Book by Alice".to_string(),
            author_id: authors[0].id, // Alice
            price: 1000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Book by Bob".to_string(),
            author_id: authors[1].id, // Bob
            price: 2000,
            published: true,
            ..Default::default()
        },
    ];

    let count = Book::objects(&db)
        .upsert_many(books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::AuthorId, Book::Price])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 2);

    let book1 = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book1.author_id, authors[0].id);

    let book2 = Book::objects(&db).get(2).await.unwrap();
    assert_eq!(book2.author_id, authors[1].id);
}

#[tokio::test]
async fn test_upsert_change_foreign_key() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    // Create book with first author
    let initial = Book {
        id: 1,
        title: "My Book".to_string(),
        author_id: authors[0].id,
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db).create(initial).await.unwrap();

    // Upsert to change author
    let updated = Book {
        id: 1,
        title: "My Book".to_string(),
        author_id: authors[2].id, // Change to Charlie
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db)
        .upsert_many(vec![updated])
        .on_conflict(Book::Id)
        .update_fields(&[Book::AuthorId])
        .execute()
        .await
        .unwrap();

    let book = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book.author_id, authors[2].id, "Author should be changed to Charlie");
}

#[tokio::test]
async fn test_upsert_multiple_books_same_author() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    // Upsert multiple books for the same author
    let books = vec![
        Book {
            id: 1,
            title: "Alice Book 1".to_string(),
            author_id: authors[0].id,
            price: 1000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Alice Book 2".to_string(),
            author_id: authors[0].id,
            price: 1500,
            published: true,
            ..Default::default()
        },
        Book {
            id: 3,
            title: "Alice Book 3".to_string(),
            author_id: authors[0].id,
            price: 2000,
            published: false,
            ..Default::default()
        },
    ];

    let count = Book::objects(&db)
        .upsert_many(books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price, Book::Published])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 3);

    // Verify all books have the same author
    let all_books = Book::objects(&db).all().await.unwrap();
    assert_eq!(all_books.len(), 3);
    for book in &all_books {
        assert_eq!(book.author_id, authors[0].id);
    }
}

#[tokio::test]
async fn test_upsert_preserves_foreign_key_when_not_updated() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let initial = Book {
        id: 1,
        title: "Original Title".to_string(),
        author_id: authors[0].id,
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db).create(initial).await.unwrap();

    // Upsert without updating author_id
    let updated = Book {
        id: 1,
        title: "Updated Title".to_string(),
        author_id: authors[1].id, // Try to change but don't include in update_fields
        price: 1500,
        published: true,
        ..Default::default()
    };

    Book::objects(&db)
        .upsert_many(vec![updated])
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    let book = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book.title, "Updated Title");
    assert_eq!(book.price, 1500);
    assert_eq!(book.author_id, authors[0].id, "Author should remain unchanged");
}

#[tokio::test]
async fn test_upsert_batch_with_mixed_authors() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    // Create one existing book
    let existing = Book {
        id: 1,
        title: "Existing".to_string(),
        author_id: authors[0].id,
        price: 1000,
        published: true,
        ..Default::default()
    };
    Book::objects(&db).create(existing).await.unwrap();

    // Upsert batch with mixed authors (1 update, 2 inserts)
    let books = vec![
        Book {
            id: 1,
            title: "Updated".to_string(),
            author_id: authors[1].id, // Change author
            price: 1500,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "New by Bob".to_string(),
            author_id: authors[1].id,
            price: 2000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 3,
            title: "New by Charlie".to_string(),
            author_id: authors[2].id,
            price: 2500,
            published: false,
            ..Default::default()
        },
    ];

    let count = Book::objects(&db)
        .upsert_many(books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::AuthorId, Book::Price])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 3);

    let book1 = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book1.author_id, authors[1].id, "Book 1 author should be updated to Bob");

    let book2 = Book::objects(&db).get(2).await.unwrap();
    assert_eq!(book2.author_id, authors[1].id);

    let book3 = Book::objects(&db).get(3).await.unwrap();
    assert_eq!(book3.author_id, authors[2].id);
}
