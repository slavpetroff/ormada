//! Common test utilities and infrastructure
//!
//! This module provides shared functionality for all tests:
//! - Database setup helpers (`test_helpers`)
//! - Test fixtures/models (`fixtures`)
//! - Legacy test entities (`author`, `book`)
//! - Assertion utilities
//! - Test data factories
//!
//! # Usage
//!
//! ## Using New Test Infrastructure
//!
//! ```rust
//! use common::test_helpers::*;
//! use common::fixtures::simple_item;
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let db = setup_test_db().await;
//!     simple_item::create_table(&db).await;
//!     
//!     let items = simple_item::sample_items(10);
//!     // ... test code
//! }
//! ```
//!
//! ## Using Legacy Setup
//!
//! ```rust
//! use common::{setup_test_db, create_sample_authors};
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let db = setup_test_db().await;
//!     let authors = create_sample_authors(&db).await;
//!     // ...
//! }
//! ```

// New modular test infrastructure
pub mod test_helpers;
pub mod fixtures;

// Test utilities using OUR django_model macro
use sea_orm::{Database, DatabaseConnection, DbBackend, Schema};
use seaorm_django::prelude::*;

/// Test Author model - properly using django_model macro
pub mod author {
    use super::*;
    
    #[django_model(table = "authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub age: i32,
        
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
        
        #[auto_now]
        pub updated_at: DateTimeWithTimeZone,
    }
    
    impl AsyncLifecycleHooks for Model {}
}

/// Test Book model - properly using django_model macro
pub mod book {
    use super::*;
    
    #[django_model(table = "books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub title: String,
        pub author_id: i32,
        pub price: i32,
        pub published: bool,
        
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
        
        #[auto_now]
        pub updated_at: DateTimeWithTimeZone,
    }
    
    impl AsyncLifecycleHooks for Model {}
}

// Convenience type aliases - users work with Author and Book directly
pub use author::Author;
pub use book::Book;

/// Setup in-memory SQLite database for testing
pub async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    // Create schema
    let schema = Schema::new(DbBackend::Sqlite);

    // Note: For schema creation we still need Entity (SeaORM requirement)
    let author_stmt = schema.create_table_from_entity(author::Entity);
    let book_stmt = schema.create_table_from_entity(book::Entity);

    use sea_orm::ConnectionTrait;
    use sea_orm::Statement;
    
    let sql = author_stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql)
        .await
        .expect("Failed to create authors table");

    let sql = book_stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql)
        .await
        .expect("Failed to create books table");

    db
}

/// Create sample authors for testing
pub async fn create_sample_authors(db: &DatabaseConnection) -> Vec<Author> {
    let mut authors = Vec::new();

    let author1 = Author::objects(db)
        .create(Author {
            name: "Alice Johnson".to_string(),
            email: "alice@example.com".to_string(),
            age: 35,
            ..Default::default()
        })
        .await
        .expect("Failed to insert author");
    authors.push(author1);

    let author2 = Author::objects(db)
        .create(Author {
            name: "Bob Smith".to_string(),
            email: "bob@example.com".to_string(),
            age: 42,
            ..Default::default()
        })
        .await
        .expect("Failed to insert author");
    authors.push(author2);

    let author3 = Author::objects(db)
        .create(Author {
            name: "Charlie Brown".to_string(),
            email: "charlie@example.com".to_string(),
            age: 28,
            ..Default::default()
        })
        .await
        .expect("Failed to insert author");
    authors.push(author3);

    authors
}

/// Create sample books for testing
pub async fn create_sample_books(db: &DatabaseConnection) -> Vec<Book> {
    let mut books = Vec::new();

    let book1 = Book::objects(db)
        .create(Book {
            title: "Rust Programming".to_string(),
            author_id: 1,
            price: 4999,
            published: true,
            ..Default::default()
        })
        .await
        .expect("Failed to insert book");
    books.push(book1);

    let book2 = Book::objects(db)
        .create(Book {
            title: "Advanced Rust".to_string(),
            author_id: 1,
            price: 5999,
            published: true,
            ..Default::default()
        })
        .await
        .expect("Failed to insert book");
    books.push(book2);

    let book3 = Book::objects(db)
        .create(Book {
            title: "Web Development".to_string(),
            author_id: 2,
            price: 3999,
            published: false,
            ..Default::default()
        })
        .await
        .expect("Failed to insert book");
    books.push(book3);

    books
}
