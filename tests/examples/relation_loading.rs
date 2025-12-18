//! Relation Loading Examples - select_related vs prefetch_related
//!
//! This module demonstrates the two main strategies for eager loading relations:
//!
//! ## `select_related` vs `prefetch_related`
//!
//! | Method | Best For | Query Pattern | Use When |
//! |--------|----------|---------------|----------|
//! | `select_related` | 1:1, FK (single object) | 1+M queries | Loading parent from child |
//! | `prefetch_related` | 1:N, M:N (multiple objects) | 1+M queries | Loading children from parent |
//!
//! Both methods prevent N+1 queries by batching relation loads.
//! Currently they use the same implementation (batched queries).
//!
//! ## When to Use Each
//!
//! - **`select_related`**: Use when following a ForeignKey or OneToOne field
//!   (e.g., Book -> Author). Each book has exactly one author.
//!
//! - **`prefetch_related`**: Use when loading reverse relations or M:N
//!   (e.g., Author -> Books). Each author may have many books.

use ormada::prelude::*;

pub mod models {
    pub mod author {
        use ormada::prelude::*;

        #[ormada_model(table = "rl_authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,
            pub name: String,
            pub email: String,
        }
    }

    pub mod book {
        use super::author::Author;
        use ormada::prelude::*;

        #[ormada_model(table = "rl_books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author)]
            pub author_id: i32,
            pub title: String,
            pub price: i32,
            pub published: bool,
        }
    }
}

pub use models::author::Author;
pub use models::book::Book;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Ok(router)
}

async fn seed_data(db: &DatabaseRouter) -> Result<Vec<(Author, Vec<Book>)>, OrmadaError> {
    let mut result = Vec::new();

    for (name, email) in [
        ("Alice", "alice@example.com"),
        ("Bob", "bob@example.com"),
        ("Charlie", "charlie@example.com"),
    ] {
        let author = Author::objects(db)
            .create(Author {
                name: name.into(),
                email: email.into(),
                ..Default::default()
            })
            .await?;

        let mut books = Vec::new();
        for i in 0..3 {
            let book = Book::objects(db)
                .create(Book {
                    author_id: author.id,
                    title: format!("{}'s Book {}", name, i + 1),
                    price: 1000 + i * 500,
                    published: i % 2 == 0,
                    ..Default::default()
                })
                .await?;
            books.push(book);
        }
        result.push((author, books));
    }

    Ok(result)
}

/// select_related - Load parent from child (FK direction)
///
/// Use when: You have Books and want to load their Authors
/// Pattern: Book -> Author (following the FK)
pub async fn example_select_related(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    // Load books WITH their authors in 2 queries (not N+1)
    // Query 1: SELECT * FROM books
    // Query 2: SELECT * FROM authors WHERE id IN (...)
    let books = Book::objects(db)
        .filter(Book::Published.eq(true))
        .select_related(relations![Author])
        .all()
        .await?;

    assert!(!books.is_empty());

    // Access author directly - no additional query!
    for book in &books {
        assert!(book.author.id > 0);
        assert!(!book.author.name.is_empty());
    }

    Ok(())
}

/// prefetch_related - Load related objects in batch
///
/// Use when: You have Books and want to load their Authors
/// (Same as select_related for FK relations)
pub async fn example_prefetch_related(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    // Load all books with authors
    let books = Book::objects(db).prefetch_related(relations![Author]).all().await?;

    assert_eq!(books.len(), 9); // 3 authors * 3 books

    // All books have their author loaded
    for book in &books {
        assert_eq!(book.author.id, book.author_id);
    }

    Ok(())
}

/// Chaining filters with relation loading
pub async fn example_filter_then_load(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    // Complex query chain with relation loading
    let books = Book::objects(db)
        .filter(Book::Price.gte(1000))
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::Price)
        .limit(5)
        .prefetch_related(relations![Author])
        .all()
        .await?;

    // Verify filter applied
    for book in &books {
        assert!(book.price >= 1000);
        assert!(book.published);
        // Author is loaded
        assert!(book.author.id > 0);
    }

    Ok(())
}

/// Loading single record with relations
pub async fn example_first_with_relations(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    // Get first book with author
    let book = Book::objects(db)
        .order_by_asc(Book::Id)
        .prefetch_related(relations![Author])
        .first()
        .await?;

    assert!(book.author.id > 0);

    Ok(())
}

/// Count and exists with prefetch (prefetch is ignored for these)
pub async fn example_count_exists_with_prefetch(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    // Count still works (prefetch is not executed for count)
    let count = Book::objects(db).prefetch_related(relations![Author]).count().await?;

    assert_eq!(count, 9);

    // Exists still works
    let exists = Book::objects(db)
        .filter(Book::Published.eq(true))
        .prefetch_related(relations![Author])
        .exists()
        .await?;

    assert!(exists);

    Ok(())
}

/// Empty result handling
pub async fn example_empty_result(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    setup_db().await?; // Fresh DB, no data

    // No books = no relation queries executed
    let books = Book::objects(db)
        .filter(Book::Id.eq(99999))
        .prefetch_related(relations![Author])
        .all()
        .await?;

    assert!(books.is_empty());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_select_related() {
        let db = setup_db().await.unwrap();
        example_select_related(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_prefetch_related() {
        let db = setup_db().await.unwrap();
        example_prefetch_related(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_filter_then_load() {
        let db = setup_db().await.unwrap();
        example_filter_then_load(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_first_with_relations() {
        let db = setup_db().await.unwrap();
        example_first_with_relations(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_count_exists_with_prefetch() {
        let db = setup_db().await.unwrap();
        example_count_exists_with_prefetch(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_empty_result() {
        let db = setup_db().await.unwrap();
        example_empty_result(&db).await.unwrap();
    }
}
