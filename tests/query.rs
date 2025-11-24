// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Query integration tests
//!
//! This module contains all query-related integration tests for the Django-like ORM.

mod fixtures;

use fixtures::*;
use rstest::*;
use seaorm_django::prelude::*;

// ============================================================================
// Basic Query Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_all_empty(#[future] db: DatabaseRouter) {
    let books = Book::objects(&db).all().await.unwrap();
    assert_eq!(books.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_all_with_data(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, sample_authors) = db_with_sample_authors;
    assert_eq!(sample_authors.len(), 3); // Ensure fixture is used
    let authors = Author::objects(&db).all().await.unwrap();
    assert_eq!(authors.len(), 3);
    assert_eq!(authors[0].name, "Alice");
    assert_eq!(authors[1].name, "Bob");
    assert_eq!(authors[2].name, "Charlie");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_count_empty(#[future] db: DatabaseRouter) {
    let count = Book::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_count_with_data(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_exists_false(#[future] db: DatabaseRouter) {
    let exists = Book::objects(&db).exists().await.unwrap();
    assert!(!exists);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_exists_true(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, _author) = db_with_author;
    let exists = Author::objects(&db).exists().await.unwrap();
    assert!(exists);
}

// ============================================================================
// Get Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_by_id_success(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let fetched = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched.id, author.id);
    assert_eq!(fetched.name, author.name);
    assert_eq!(fetched.email, author.email);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_by_id_not_found(#[future] db: DatabaseRouter) {
    let result = Author::objects(&db).get(999).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_first_success(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let first = Author::objects(&db).first().await.unwrap();
    assert_eq!(first.name, "Alice");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_first_empty(#[future] db: DatabaseRouter) {
    let result = Book::objects(&db).first().await;
    assert!(result.is_err());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_last_success(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let last = Author::objects(&db).last().await.unwrap();
    assert_eq!(last.name, "Charlie");
}

// ============================================================================
// Filtering
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_eq(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Name.eq("Bob")).all().await.unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Bob");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_ne(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Name.ne("Bob")).all().await.unwrap();
    assert_eq!(authors.len(), 2);
    assert!(!authors.iter().any(|a| a.name == "Bob"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_gt(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Age.gt(28)).all().await.unwrap();
    assert_eq!(authors.len(), 2);
    assert!(authors.iter().all(|a| a.age > 28));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_gte(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Age.gte(30)).all().await.unwrap();
    assert_eq!(authors.len(), 2);
    assert!(authors.iter().all(|a| a.age >= 30));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_lt(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Age.lt(30)).all().await.unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Alice");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_lte(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Age.lte(30)).all().await.unwrap();
    assert_eq!(authors.len(), 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_contains(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Name.contains("li")).all().await.unwrap();
    assert_eq!(authors.len(), 2); // Alice and Charlie
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_starts_with(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Name.starts_with("C")).all().await.unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Charlie");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_ends_with(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).filter(Author::Name.ends_with("e")).all().await.unwrap();
    assert_eq!(authors.len(), 2); // Alice and Charlie
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_in_values(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db)
        .filter(Author::Name.in_values(vec!["Alice", "Charlie"]))
        .all()
        .await
        .unwrap();
    assert_eq!(authors.len(), 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_chained(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .filter(Author::Age.lte(30))
        .all()
        .await
        .unwrap();
    assert_eq!(authors.len(), 2); // Alice (25) and Bob (30)
}

// ============================================================================
// Exclude Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_exclude(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).exclude(Author::Name.eq("Bob")).all().await.unwrap();
    assert_eq!(authors.len(), 2);
    assert!(!authors.iter().any(|a| a.name == "Bob"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_and_exclude(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .exclude(Author::Name.eq("Bob"))
        .all()
        .await
        .unwrap();
    assert_eq!(authors.len(), 2); // Alice and Charlie
}

// ============================================================================
// Ordering
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_order_by_asc(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).order_by_asc(Author::Age).all().await.unwrap();
    assert_eq!(authors[0].name, "Alice");
    assert_eq!(authors[1].name, "Bob");
    assert_eq!(authors[2].name, "Charlie");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_order_by_desc(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).order_by_desc(Author::Age).all().await.unwrap();
    assert_eq!(authors[0].name, "Charlie");
    assert_eq!(authors[1].name, "Bob");
    assert_eq!(authors[2].name, "Alice");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_earliest(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let author = Author::objects(&db).earliest(Author::Age).await.unwrap();
    assert_eq!(author.name, "Alice");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_latest(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let author = Author::objects(&db).latest(Author::Age).await.unwrap();
    assert_eq!(author.name, "Charlie");
}

// ============================================================================
// Pagination
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_limit(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).limit(2).all().await.unwrap();
    assert_eq!(authors.len(), 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_offset(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db)
        .order_by_asc(Author::Name)
        .limit(10) // SQLite requires LIMIT when using OFFSET
        .offset(1)
        .all()
        .await
        .unwrap();
    assert_eq!(authors.len(), 2);
    assert_eq!(authors[0].name, "Bob");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_limit_and_offset(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db)
        .order_by_asc(Author::Name)
        .limit(1)
        .offset(1)
        .all()
        .await
        .unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Bob");
}

// ============================================================================
// Distinct
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_distinct(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let authors = Author::objects(&db).distinct().all().await.unwrap();
    assert_eq!(authors.len(), 3);
}

// ============================================================================
// Query Caching
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_query_caching(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let qs = Author::objects(&db).filter(Author::Id.eq(author.id));

    // First call - hits DB
    let result1 = qs.all().await.unwrap();
    assert_eq!(result1.len(), 1);

    // Second call on same QuerySet - uses cache
    let result2 = qs.all().await.unwrap();
    assert_eq!(result2.len(), 1);
    assert_eq!(result1[0].id, result2[0].id);
}

// ============================================================================
// Complex Queries
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_complex_filter_chain(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let books = Book::objects(&db)
        .filter(Book::Price.gte(1000))
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::Price)
        .limit(3)
        .all()
        .await
        .unwrap();

    assert!(books.len() <= 3);
    assert!(books.iter().all(|b| b.price >= 1000 && b.published));
}

#[rstest]
#[awt]
#[case(5)]
#[case(10)]
#[case(20)]
#[tokio::test]
async fn test_pagination_various_sizes(#[future] db: DatabaseRouter, #[case] count: usize) {
    // Create N authors
    for i in 0..count {
        Author::objects(&db)
            .create(Author {
                id: 0,
                name: format!("Author {}", i + 1),
                email: format!("author{}@example.com", i + 1),
                age: 25 + (i as i32),
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let page_size = 3;
    let page1 = Author::objects(&db)
        .order_by_asc(Author::Id)
        .limit(page_size as u64)
        .all()
        .await
        .unwrap();

    let page2 = Author::objects(&db)
        .order_by_asc(Author::Id)
        .limit(page_size as u64)
        .offset(page_size as u64)
        .all()
        .await
        .unwrap();

    assert_eq!(page1.len(), page_size.min(count));
    if count > page_size {
        assert!(page2.len() > 0);
        assert_eq!(page2.len(), (page_size).min(count - page_size));
    }
}
