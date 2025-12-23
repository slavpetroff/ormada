//! Bulk Operations Example
//!
//! **Note**: Lifecycle hooks are NOT called for bulk operations for performance.

#![allow(clippy::items_after_statements)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::similar_names)]

use ormada::prelude::*;

mod author {
    use ormada::prelude::*;

    #[ormada_model(table = "bulk_authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
    }
}

mod book {
    use ormada::prelude::*;

    #[ormada_model(table = "bulk_books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub isbn: String,
        pub title: String,
        pub price: i32,
    }
}

pub use author::Author;
pub use book::Book;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Ok(router)
}

/// `bulk_create` - insert many records in a single query
/// **Note**: Does NOT trigger lifecycle hooks
pub async fn example_bulk_create(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let authors: Vec<Author> = (0..100)
        .map(|i| Author {
            name: format!("Author {i}"),
            email: format!("author{i}@example.com"),
            ..Default::default()
        })
        .collect();

    let count = Author::objects(db).bulk_create(authors).await?;
    assert_eq!(count, 100, "Should insert 100 authors");

    let total = Author::objects(db).count().await?;
    assert_eq!(total, 100);

    Ok(())
}

/// `upsert_many` - INSERT ... ON CONFLICT DO UPDATE
/// **Note**: Does NOT trigger lifecycle hooks
/// **Note**: Requires UNIQUE constraint on conflict column (isbn in this case)
pub async fn example_bulk_upsert(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    // For upsert to work, we need a unique constraint on isbn
    // In real usage, this would be defined in the model with #[unique]
    // For this test, we'll use the primary key (id) instead

    let books = vec![
        Book {
            id: 1,
            isbn: "978-1234567890".into(),
            title: "Original 1".into(),
            price: 1999,
        },
        Book {
            id: 2,
            isbn: "978-0987654321".into(),
            title: "Original 2".into(),
            price: 2999,
        },
    ];

    Book::objects(db)
        .upsert_many(books)
        .on_conflict(Book::Id)  // Use primary key which has unique constraint
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await?;

    assert_eq!(Book::objects(db).count().await?, 2);

    // Upsert with updates - same IDs will update
    let updated = vec![
        Book {
            id: 1,
            isbn: "978-1234567890".into(),
            title: "Updated 1".into(),
            price: 2499,
        },
        Book {
            id: 3,
            isbn: "978-1111111111".into(),
            title: "New Book".into(),
            price: 1999,
        },
    ];

    Book::objects(db)
        .upsert_many(updated)
        .on_conflict(Book::Id)
        .update_fields(&[Book::Title, Book::Price])
        .execute()
        .await?;

    assert_eq!(Book::objects(db).count().await?, 3);

    let book1 = Book::objects(db).filter(Book::Id.eq(1)).first().await?;
    assert_eq!(book1.title, "Updated 1");
    assert_eq!(book1.price, 2499);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bulk_create() {
        let db = setup_db().await.unwrap();
        example_bulk_create(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_bulk_upsert() {
        let db = setup_db().await.unwrap();
        example_bulk_upsert(&db).await.unwrap();
    }
}
