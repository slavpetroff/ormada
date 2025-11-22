use crate::common::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_upsert_large_batch() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let mut books = Vec::new();
    for i in 1..=100 {
        books.push(Book {
            id: i,
            title: format!("Book {}", i),
            author_id: 1,
            price: i * 100,
            published: i % 2 == 0,
            ..Default::default()
        });
    }

    let count = Book::objects(&db)
        .upsert_many(books)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 100);

    let all_books = Book::objects(&db).count().await.unwrap();
    assert_eq!(all_books, 100);
}

#[tokio::test]
async fn test_upsert_then_update_batch() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let books1 = vec![
        Book {
            id: 1,
            title: "Book 1 v1".to_string(),
            author_id: 1,
            price: 1000,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Book 2 v1".to_string(),
            author_id: 1,
            price: 2000,
            published: true,
            ..Default::default()
        },
    ];

    Book::objects(&db)
        .upsert_many(books1)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    let books2 = vec![
        Book {
            id: 1,
            title: "Book 1 v2".to_string(),
            author_id: 1,
            price: 1500,
            published: true,
            ..Default::default()
        },
        Book {
            id: 2,
            title: "Book 2 v2".to_string(),
            author_id: 1,
            price: 2500,
            published: true,
            ..Default::default()
        },
        Book {
            id: 3,
            title: "Book 3 v1".to_string(),
            author_id: 1,
            price: 3000,
            published: false,
            ..Default::default()
        },
    ];

    let count = Book::objects(&db)
        .upsert_many(books2)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await
        .unwrap();

    assert_eq!(count, 3);

    let book1 = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book1.title, "Book 1 v2");
    assert_eq!(book1.price, 1500);

    let book2 = Book::objects(&db).get(2).await.unwrap();
    assert_eq!(book2.title, "Book 2 v2");
    assert_eq!(book2.price, 2500);

    let book3 = Book::objects(&db).get(3).await.unwrap();
    assert_eq!(book3.title, "Book 3 v1");
}

#[tokio::test]
async fn test_upsert_idempotent() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let books = vec![Book {
        id: 1,
        title: "Book 1".to_string(),
        author_id: 1,
        price: 1000,
        published: true,
        ..Default::default()
    }];

    for _ in 0..3 {
        Book::objects(&db)
            .upsert_many(books.clone())
            .on_conflict(Book::Id)
            .update_fields(&[Book::Title, Book::Price])
            .execute()
            .await
            .unwrap();
    }

    let count = Book::objects(&db).count().await.unwrap();
    assert_eq!(count, 1, "Should still have only 1 book after multiple upserts");

    let book = Book::objects(&db).get(1).await.unwrap();
    assert_eq!(book.title, "Book 1");
    assert_eq!(book.price, 1000);
}

#[tokio::test]
async fn test_upsert_preserves_unchanged_fields() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let initial = Book {
        id: 1,
        title: "Original".to_string(),
        author_id: 1,
        price: 1000,
        published: true,
        ..Default::default()
    };

    Book::objects(&db).create(initial).await.unwrap();

    let updated = Book {
        id: 1,
        title: "Updated".to_string(),
        author_id: 999,
        price: 9999,
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
    assert_eq!(book.title, "Updated");
    assert_eq!(book.author_id, 1, "author_id should be unchanged");
    assert_eq!(book.price, 1000, "price should be unchanged");
    assert_eq!(book.published, true, "published should be unchanged");
}
