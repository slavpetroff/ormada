//! Basic CRUD Operations Example
//!
//! This example demonstrates Create, Read, Update, Delete operations.

use ormada::prelude::*;

mod author {
    use ormada::prelude::*;

    #[ormada_model(table = "crud_authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
    }
}

mod book {
    use ormada::prelude::*;

    #[ormada_model(table = "crud_books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub author_id: i32,
        pub title: String,
        pub price: i32,
        pub published: bool,
    }
}

pub use author::Author;
pub use book::Book;

/// Setup database with tables
pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Ok(router)
}

/// Example: Create a single record
pub async fn example_create(db: &DatabaseRouter) -> Result<Author, OrmadaError> {
    let author = Author::objects(db)
        .create(Author {
            name: "Alice Smith".into(),
            email: "alice@example.com".into(),
            ..Default::default()
        })
        .await?;

    assert!(author.id > 0, "Author should have a valid ID after creation");
    assert_eq!(author.name, "Alice Smith");
    assert_eq!(author.email, "alice@example.com");

    Ok(author)
}

/// Example: Read records with filtering
pub async fn example_read(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let author = example_create(db).await?;

    Book::objects(db)
        .create(Book {
            author_id: author.id,
            title: "Rust Programming".into(),
            price: 2999,
            published: true,
            ..Default::default()
        })
        .await?;

    Book::objects(db)
        .create(Book {
            author_id: author.id,
            title: "Advanced Rust".into(),
            price: 3999,
            published: false,
            ..Default::default()
        })
        .await?;

    // Get all books
    let all_books = Book::objects(db).all().await?;
    assert_eq!(all_books.len(), 2, "Should have 2 books");

    // Get by primary key
    let book = Book::objects(db).get(all_books[0].id).await?;
    assert_eq!(book.title, "Rust Programming");

    // Filter by field
    let published_books = Book::objects(db).filter(Book::Published.eq(true)).all().await?;
    assert_eq!(published_books.len(), 1, "Should have 1 published book");

    // Count records
    let count = Book::objects(db).count().await?;
    assert_eq!(count, 2, "Should count 2 books");

    Ok(())
}

/// Example: Update records
pub async fn example_update(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let author = example_create(db).await?;

    let book = Book::objects(db)
        .create(Book {
            author_id: author.id,
            title: "Old Title".into(),
            price: 1999,
            published: false,
            ..Default::default()
        })
        .await?;

    let updated_count = Book::objects(db)
        .filter(Book::Id.eq(book.id))
        .update(|mut book| async move {
            book.title = "New Title".into();
            book.price = 2499;
            book.published = true;
            Ok(book)
        })
        .await?;

    assert_eq!(updated_count, 1, "Should update 1 record");

    let updated_book = Book::objects(db).get(book.id).await?;
    assert_eq!(updated_book.title, "New Title");
    assert_eq!(updated_book.price, 2499);
    assert!(updated_book.published);

    Ok(())
}

/// Example: Delete records
pub async fn example_delete(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let author = example_create(db).await?;

    Book::objects(db)
        .create(Book {
            author_id: author.id,
            title: "To Delete".into(),
            price: 999,
            published: false,
            ..Default::default()
        })
        .await?;

    let initial_count = Book::objects(db).count().await?;
    assert_eq!(initial_count, 1);

    let deleted_count = Book::objects(db).filter(Book::Published.eq(false)).delete().await?;

    assert_eq!(deleted_count, 1, "Should delete 1 record");

    let final_count = Book::objects(db).count().await?;
    assert_eq!(final_count, 0, "Should have 0 books after delete");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create() {
        let db = setup_db().await.unwrap();
        example_create(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_read() {
        let db = setup_db().await.unwrap();
        example_read(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_update() {
        let db = setup_db().await.unwrap();
        example_update(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete() {
        let db = setup_db().await.unwrap();
        example_delete(&db).await.unwrap();
    }
}
