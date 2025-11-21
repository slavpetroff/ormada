use crate::common::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_upsert_insert_new_records() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let books = vec![
        Book {
            id: 1,
            title: "Book 1".to_string(),
            author_id: 1,
            price: 1000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Book 2".to_string(),
            author_id: 1,
            price: 2000,
            published: true,
            ..Default::default()
        },
    ];

    let count = Book::objects(&db)
        .upsert_many(books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 2, "Should process 2 records");

    let all_books = Book::objects(&db).all().await.unwrap();
    assert_eq!(all_books.len(), 2, "Should have 2 books in database");
    assert_eq!(all_books[0].title, "Book 1");
    assert_eq!(all_books[1].title, "Book 2");
}

#[tokio::test]
async fn test_upsert_update_existing_records() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let initial = Book {
        id: 1,
        title: "Original Title".to_string(),
        author_id: 1,
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db).create(initial).await.unwrap();

    let updated_book = Book {
        id: 1,
        title: "Updated Title".to_string(),
        author_id: 1,
        price: 1500,
        published: true,
        ..Default::default()
    };

    let count = Book::objects(&db)
        .upsert_many(vec![updated_book])
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 1);

    let book = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book.title, "Updated Title", "Title should be updated");
    assert_eq!(book.price, 1500, "Price should be updated");
}

#[tokio::test]
async fn test_upsert_mixed_insert_and_update() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let existing = Book {
        id: 1,
        title: "Existing Book".to_string(),
        author_id: 1,
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db).create(existing).await.unwrap();

    let books = vec![
        Book {
            id: 1,
            title: "Updated Existing".to_string(),
            author_id: 1,
            price: 1500,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "New Book".to_string(),
            author_id: 1,
            price: 2000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 3,
            title: "Another New Book".to_string(),
            author_id: 1,
            price: 2500,
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

    assert_eq!(count, 3, "Should process 3 records");

    let all_books = Book::objects(&db).all().await.unwrap();
    assert_eq!(all_books.len(), 3, "Should have 3 books");

    let book1 = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book1.title, "Updated Existing");
    assert_eq!(book1.price, 1500);

    let book2 = Book::objects(&db).get(2).await.unwrap();
    assert_eq!(book2.title, "New Book");

    let book3 = Book::objects(&db).get(3).await.unwrap();
    assert_eq!(book3.title, "Another New Book");
}

#[tokio::test]
async fn test_upsert_empty_list() {
    let db = setup_test_db().await;

    let count = Book::objects(&db)
        .upsert_many(vec![])
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 0, "Should return 0 for empty list");
}

#[tokio::test]
async fn test_upsert_partial_field_update() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let initial = Book {
        id: 1,
        title: "Original Title".to_string(),
        author_id: 1,
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db).create(initial).await.unwrap();

    let updated = Book {
        id: 1,
        title: "Updated Title".to_string(),
        author_id: 1,
        price: 5000,
        published: false,
        ..Default::default()
    };

    Book::objects(&db)
        .upsert_many(vec![updated])
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title])
        .execute()
        .await
        .unwrap();

    let book = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book.title, "Updated Title", "Title should be updated");
    assert_eq!(book.price, 1000, "Price should NOT be updated");
    assert_eq!(book.published, true, "Published should NOT be updated");
}
