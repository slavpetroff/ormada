// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::indexing_slicing)]

//! rstest fixtures for comprehensive test support
//!
//! This module provides reusable fixtures for all integration tests,
//! following rstest best practices and eliminating test boilerplate.

use ormada::prelude::*;
use rstest::*;

// ============================================================================
// Test Models
// ============================================================================

pub mod models {
    use ormada::prelude::*;

    pub mod author {
        use super::*;

        #[ormada_model(table = "authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,

            #[max_length(100)]
            pub name: String,

            #[max_length(200)]
            pub email: String,

            #[range(min = 0, max = 150)]
            pub age: i32,

            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,

            #[auto_now]
            pub updated_at: DateTimeWithTimeZone,
        }
        // LifecycleHooks is auto-implemented by #[ormada_model] - no manual impl needed!
    }

    pub mod book {
        use super::*;

        #[ormada_model(table = "books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,

            #[foreign_key(Author)]
            pub author_id: i32,

            #[max_length(200)]
            pub title: String,

            pub price: i32,
            pub published: bool,

            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,

            #[auto_now]
            pub updated_at: DateTimeWithTimeZone,
        }
        // LifecycleHooks is auto-implemented by #[ormada_model] - no manual impl needed!
    }
}

// Re-export for convenience
pub use models::author::Author;
pub use models::book::Book;

// ============================================================================
// Helper Functions
// ============================================================================

pub async fn create_sample_authors(db: &DatabaseRouter) -> Vec<Author> {
    let mut authors = Vec::new();

    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 35)] {
        let author = Author::objects(db)
            .create(Author {
                id: 0,
                name: name.to_string(),
                email: format!("{}@example.com", name.to_lowercase()),
                age,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
            .expect("Failed to create author");
        authors.push(author);
    }

    authors
}

// ============================================================================
// Database Fixtures
// ============================================================================

/// Minimal database fixture - creates an empty in-memory `SQLite` database
///
/// Use this when you want to create your own tables.
#[fixture]
pub async fn db_empty() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    DatabaseRouter::new_single(db)
}

/// Primary database fixture - creates a fresh in-memory `SQLite` database
///
/// Base database fixture with tables created
///
/// This is the base fixture that most other fixtures depend on.
/// Automatically creates Author and Book tables.
///
/// NOTE: Cannot use #[once] because this is an async fixture and rstest
/// forbids #[once] on async functions. Each test gets its own database instance.
#[fixture]
#[awt]
pub async fn db() -> DatabaseRouter {
    let db_router = db_empty().await;

    // Create tables using macro-generated methods
    Author::create_table(&db_router).await.expect("Failed to create authors table");
    Book::create_table(&db_router).await.expect("Failed to create books table");

    db_router
}

// ============================================================================
// Author Fixtures
// ============================================================================

/// Creates a single author with default values
#[fixture]
#[awt]
pub async fn author(#[future] db: DatabaseRouter) -> Author {
    Author::objects(&db)
        .create(Author {
            id: 0,
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .expect("Failed to create author")
}

/// Creates a single author with custom name
#[fixture]
#[awt]
pub async fn author_named(
    #[future] db: DatabaseRouter,
    #[default("Custom Author")] name: &str,
) -> Author {
    Author::objects(&db)
        .create(Author {
            id: 0,
            name: name.to_string(),
            email: format!("{}@example.com", name.to_lowercase().replace(' ', ".")),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .expect("Failed to create author")
}

/// Creates 3 sample authors (Alice, Bob, Charlie)
#[fixture]
#[awt]
pub async fn sample_authors(#[future] db: DatabaseRouter) -> Vec<Author> {
    create_sample_authors(&db).await
}

/// Creates sample authors and returns both the db and the authors
///
/// **Pattern Note**: This combined fixture exists because rstest cannot use #[once]
/// with async fixtures. When a test needs both `db` and a fixture that depends on `db`
/// (like `sample_authors`), using them as separate `#[future]` parameters creates
/// TWO database instances. This combined fixture ensures both use the SAME database.
///
/// Use this pattern when you need both the database and fixture data in your test:
/// ```rust
/// #[rstest]
/// #[awt]
/// #[tokio::test]
/// async fn my_test(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
///     let (db, authors) = db_with_sample_authors;
///     // Now db and authors share the same database instance
/// }
/// ```
#[fixture]
#[awt]
pub async fn db_with_sample_authors(#[future] db: DatabaseRouter) -> (DatabaseRouter, Vec<Author>) {
    let authors = create_sample_authors(&db).await;
    (db, authors)
}

/// Combined fixture: db with a single author
///
/// Creates a single author within the same database instance.
#[fixture]
#[awt]
pub async fn db_with_author(#[future] db: DatabaseRouter) -> (DatabaseRouter, Author) {
    let author = Author::objects(&db)
        .create(Author {
            id: 0,
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .expect("Failed to create author");
    (db, author)
}

/// Combined fixture: db with `authors_with_books`
///
/// Creates multiple authors with books in the same database instance.
#[fixture]
#[awt]
pub async fn db_with_authors_with_books(
    #[future] db: DatabaseRouter,
    #[default(3)] author_count: usize,
    #[default(2)] books_per_author: usize,
) -> (DatabaseRouter, Vec<(Author, Vec<Book>)>) {
    let mut result = Vec::new();

    for i in 0..author_count {
        let author = Author::objects(&db)
            .create(Author {
                id: 0,
                name: format!("Author {}", i + 1),
                email: format!("author{}@example.com", i + 1),
                age: 30 + (i as i32 * 5),
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
            .expect("Failed to create author");

        let mut books = Vec::new();
        for j in 0..books_per_author {
            let book = Book::objects(&db)
                .create(Book {
                    author_id: author.id,
                    title: format!("Author {} Book {}", i + 1, j + 1),
                    price: 1000 + (j as i32 * 500),
                    published: j % 2 == 0,
                    ..Default::default()
                })
                .await
                .expect("Failed to create book");
            books.push(book);
        }

        result.push((author, books));
    }

    (db, result)
}

/// Combined fixture: db with a single author and books
#[fixture]
#[awt]
pub async fn db_with_author_with_books(
    #[future] db: DatabaseRouter,
    #[default(3)] book_count: usize,
) -> (DatabaseRouter, (Author, Vec<Book>)) {
    let author = Author::objects(&db)
        .create(Author {
            id: 0,
            name: "Author with Books".to_string(),
            email: "author.books@example.com".to_string(),
            age: 35,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .expect("Failed to create author");

    let mut books = Vec::new();
    for i in 0..book_count {
        let book = Book::objects(&db)
            .create(Book {
                author_id: author.id,
                title: format!("Book {}", i + 1),
                price: 1000 + (i as i32 * 500),
                published: i % 2 == 0,
                ..Default::default()
            })
            .await
            .expect("Failed to create book");
        books.push(book);
    }

    (db, (author, books))
}

/// Creates N authors with sequential names
#[fixture]
#[awt]
pub async fn authors_n(#[future] db: DatabaseRouter, #[default(5)] count: usize) -> Vec<Author> {
    let mut authors = Vec::new();
    for i in 0..count {
        let author = Author::objects(&db)
            .create(Author {
                id: 0,
                name: format!("Author {}", i + 1),
                email: format!("author{}@example.com", i + 1),
                age: 25 + (i as i32 * 5),
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
            .expect("Failed to create author");
        authors.push(author);
    }
    authors
}

// ============================================================================
// Book Fixtures
// ============================================================================

/// Creates a single book with a given author
#[fixture]
#[awt]
pub async fn book(#[future] db: DatabaseRouter, #[future] author: Author) -> Book {
    Book::objects(&db)
        .create(Book {
            author_id: author.id,
            title: "Test Book".to_string(),
            price: 1999,
            published: true,
            ..Default::default()
        })
        .await
        .expect("Failed to create book")
}

/// Creates N books for a given author
#[fixture]
#[awt]
pub async fn books_for_author(
    #[future] db: DatabaseRouter,
    #[future] author: Author,
    #[default(3)] count: usize,
) -> Vec<Book> {
    let mut books = Vec::new();
    for i in 0..count {
        let book = Book::objects(&db)
            .create(Book {
                author_id: author.id,
                title: format!("Book {}", i + 1),
                price: 1000 + (i as i32 * 500),
                published: i % 2 == 0,
                ..Default::default()
            })
            .await
            .expect("Failed to create book");
        books.push(book);
    }
    books
}

/// Creates an author with N books (commonly needed combination)
#[fixture]
#[awt]
pub async fn author_with_books(
    #[future] db: DatabaseRouter,
    #[default(3)] book_count: usize,
) -> (Author, Vec<Book>) {
    let author = Author::objects(&db)
        .create(Author {
            id: 0,
            name: "Author with Books".to_string(),
            email: "author.books@example.com".to_string(),
            age: 35,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .expect("Failed to create author");

    let mut books = Vec::new();
    for i in 0..book_count {
        let book = Book::objects(&db)
            .create(Book {
                author_id: author.id,
                title: format!("Book {}", i + 1),
                price: 1000 + (i as i32 * 500),
                published: i % 2 == 0,
                ..Default::default()
            })
            .await
            .expect("Failed to create book");
        books.push(book);
    }

    (author, books)
}

/// Creates multiple authors each with books
#[fixture]
#[awt]
pub async fn authors_with_books(
    #[future] db: DatabaseRouter,
    #[default(3)] author_count: usize,
    #[default(2)] books_per_author: usize,
) -> Vec<(Author, Vec<Book>)> {
    let mut result = Vec::new();

    for i in 0..author_count {
        let author = Author::objects(&db)
            .create(Author {
                id: 0,
                name: format!("Author {}", i + 1),
                email: format!("author{}@example.com", i + 1),
                age: 30 + (i as i32 * 5),
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
            .expect("Failed to create author");

        let mut books = Vec::new();
        for j in 0..books_per_author {
            let book = Book::objects(&db)
                .create(Book {
                    author_id: author.id,
                    title: format!("Author {} Book {}", i + 1, j + 1),
                    price: 1000 + (j as i32 * 500),
                    published: j % 2 == 0,
                    ..Default::default()
                })
                .await
                .expect("Failed to create book");
            books.push(book);
        }

        result.push((author, books));
    }

    result
}
