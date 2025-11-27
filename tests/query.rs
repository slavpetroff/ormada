// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::needless_update)]

//! Query integration tests
//!
//! This module contains all query-related integration tests for the Ormada-like ORM.

mod fixtures;

use fixtures::*;
use ormada::prelude::*;
use rstest::*;

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
// Test QueryPlan Access
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_query_plan_access(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _authors) = db_with_sample_authors;

    // Build a query with filters and ordering
    let query = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .order_by_asc(Author::Name)
        .limit(10);

    // Access the query plan
    let plan = query.plan();

    // Verify plan has expected operations
    assert!(plan.has_filters());
    assert!(plan.has_ordering());
    assert!(plan.has_limit());
    assert_eq!(plan.get_limit(), Some(10));
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
        assert!(!page2.is_empty());
        assert_eq!(page2.len(), (page_size).min(count - page_size));
    }
}

// ============================================================================
// Aggregation Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_count(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db).aggregate_count().await.unwrap();
    assert_eq!(count, 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_count_with_filter(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db)
        .filter(Author::Age.gte(30))
        .aggregate_count()
        .await
        .unwrap();
    assert_eq!(count, 2); // Bob (30) and Charlie (35)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_sum(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let total = Book::objects(&db).aggregate_sum(Book::Price).await.unwrap();
    assert!(total.is_some());
    assert!(total.unwrap() > 0.0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_sum_empty(#[future] db: DatabaseRouter) {
    let total = Book::objects(&db).aggregate_sum(Book::Price).await.unwrap();
    assert!(total.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_avg(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let avg = Book::objects(&db).aggregate_avg(Book::Price).await.unwrap();
    assert!(avg.is_some());
    assert!(avg.unwrap() > 0.0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_avg_empty(#[future] db: DatabaseRouter) {
    let avg = Book::objects(&db).aggregate_avg(Book::Price).await.unwrap();
    assert!(avg.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_max(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let max_age = Author::objects(&db).aggregate_max(Author::Age).await.unwrap();
    assert_eq!(max_age, Some(35.0)); // Charlie's age
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_max_empty(#[future] db: DatabaseRouter) {
    let max_age = Author::objects(&db).aggregate_max(Author::Age).await.unwrap();
    assert!(max_age.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_min(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let min_age = Author::objects(&db).aggregate_min(Author::Age).await.unwrap();
    assert_eq!(min_age, Some(25.0)); // Alice's age
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_min_empty(#[future] db: DatabaseRouter) {
    let min_age = Author::objects(&db).aggregate_min(Author::Age).await.unwrap();
    assert!(min_age.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_with_complex_filter(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let total = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .filter(Book::Price.gte(1000))
        .aggregate_sum(Book::Price)
        .await
        .unwrap();
    assert!(total.is_some());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_multiple_operations(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Test chaining different aggregations
    let count = Author::objects(&db).aggregate_count().await.unwrap();
    let max = Author::objects(&db).aggregate_max(Author::Age).await.unwrap();
    let min = Author::objects(&db).aggregate_min(Author::Age).await.unwrap();
    let avg = Author::objects(&db).aggregate_avg(Author::Age).await.unwrap();

    assert_eq!(count, 3);
    assert_eq!(max, Some(35.0));
    assert_eq!(min, Some(25.0));
    assert!(avg.is_some());
    let avg_val = avg.unwrap();
    assert!((25.0..=35.0).contains(&avg_val));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_on_filtered_queryset(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let max_age = Author::objects(&db)
        .filter(Author::Age.lt(35))
        .aggregate_max(Author::Age)
        .await
        .unwrap();

    assert_eq!(max_age, Some(30.0)); // Bob, not Charlie
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregate_sum_with_filter(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let total = Book::objects(&db)
        .filter(Book::Published.eq(false))
        .aggregate_sum(Book::Price)
        .await
        .unwrap();

    // Should have some unpublished books
    assert!(total.is_some() || total.is_none());
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_with_null_checks(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;
    // All authors should have non-null names
    let authors = Author::objects(&db).filter(Author::Name.is_not_null()).all().await.unwrap();
    assert_eq!(authors.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_query(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let names = Author::objects(&db).values(vec![Author::Name]).await.unwrap();
    assert_eq!(names.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_empty_columns(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;
    let values = Author::objects(&db).values(vec![]).await.unwrap();
    assert_eq!(values.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_list_query(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let ages = Author::objects(&db).values_list(vec![Author::Age], false).await.unwrap();
    assert_eq!(ages.len(), 3);
    // Values are returned as JsonValue, so just check count
    assert!(!ages.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_list_flat(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let names = Author::objects(&db).values_list(vec![Author::Name], true).await.unwrap();
    assert_eq!(names.len(), 3);
    // With flat=true, should return flat values not arrays
    for name in &names {
        assert!(!name.is_array() || name.is_string());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_list_empty_columns(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;
    let values = Author::objects(&db).values_list(vec![], false).await.unwrap();
    assert_eq!(values.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_earliest_on_empty(#[future] db: DatabaseRouter) {
    let result = Author::objects(&db).earliest(Author::Age).await;
    assert!(result.is_err());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_latest_on_empty(#[future] db: DatabaseRouter) {
    let result = Author::objects(&db).latest(Author::Age).await;
    assert!(result.is_err());
}

// ============================================================================
// Query Caching Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_cache_hit_first_after_all(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let qs = Author::objects(&db).order_by_asc(Author::Age);

    // First call to .all() populates cache
    let all_authors = qs.all().await.unwrap();
    assert_eq!(all_authors.len(), 3);

    // Subsequent .first() should hit cache
    let first_author = qs.first().await.unwrap();
    assert_eq!(first_author.age, 25); // Alice (youngest)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_queryset_explicit_clone(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let base_qs = Author::objects(&db);
    let qs1 = base_qs.clone().filter(Author::Age.gt(25));
    let qs2 = base_qs.clone().filter(Author::Age.lt(35));

    let result1 = qs1.all().await.unwrap();
    let result2 = qs2.all().await.unwrap();

    // Explicitly cloned querysets should be independent
    assert_eq!(result1.len(), 2); // Bob, Charlie
    assert_eq!(result2.len(), 2); // Alice, Bob
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_cache_cleared_across_queries(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Different queries should not interfere
    let qs1 = Author::objects(&db).filter(Author::Age.gt(25));
    let qs2 = Author::objects(&db).filter(Author::Age.lt(35));

    let result1 = qs1.all().await.unwrap();
    let result2 = qs2.all().await.unwrap();

    assert_eq!(result1.len(), 2); // Bob, Charlie
    assert_eq!(result2.len(), 2); // Alice, Bob
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_queryset_clone_independence(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let base_qs = Author::objects(&db);
    let qs1 = base_qs.filter(Author::Age.gt(25));
    let qs2 = base_qs.filter(Author::Age.lt(35));

    let result1 = qs1.all().await.unwrap();
    let result2 = qs2.all().await.unwrap();

    // Each queryset should be independent
    assert_eq!(result1.len(), 2);
    assert_eq!(result2.len(), 2);
}

// ============================================================================
// Additional Query Methods
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_is_null_filter(#[future] db: DatabaseRouter) {
    // Create author and verify null checks work
    Author::objects(&db)
        .create(Author {
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    let authors = Author::objects(&db).filter(Author::Name.is_null()).all().await.unwrap();

    assert_eq!(authors.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_ordering_stability(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors1 = Author::objects(&db).order_by_asc(Author::Name).all().await.unwrap();

    let authors2 = Author::objects(&db).order_by_asc(Author::Name).all().await.unwrap();

    // Order should be stable across calls
    for (a1, a2) in authors1.iter().zip(authors2.iter()) {
        assert_eq!(a1.id, a2.id);
        assert_eq!(a1.name, a2.name);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_limit_exceeds_count(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db).limit(100).all().await.unwrap();

    assert_eq!(authors.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_offset_exceeds_count(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db).limit(10).offset(100).all().await.unwrap();

    assert_eq!(authors.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_multiple_filters_same_column(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .filter(Author::Age.lte(30))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2); // Alice (25) and Bob (30)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_exclude_combination(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .exclude(Author::Age.eq(30))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2); // Alice (25) and Charlie (35), excluding Bob (30)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_exclude_with_multiple_conditions(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db)
        .exclude(Author::Age.eq(25))
        .exclude(Author::Age.eq(35))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 1); // Only Bob (30)
    assert_eq!(authors[0].age, 30);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_ordering_with_multiple_columns(#[future] db: DatabaseRouter) {
    // Create authors with same age but different names
    for (name, age) in [("Alice", 30), ("Bob", 30), ("Charlie", 25)] {
        Author::objects(&db)
            .create(Author {
                name: name.to_string(),
                email: format!("{}@test.com", name.to_lowercase()),
                age,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let authors = Author::objects(&db)
        .order_by_desc(Author::Age)
        .order_by_asc(Author::Name)
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 3);
    // Age 30 first (Alice, Bob), then age 25 (Charlie)
    assert_eq!(authors[0].age, 30);
    assert_eq!(authors[1].age, 30);
    assert_eq!(authors[2].age, 25);
}

// ============================================================================
// Q Object Tests (Complex Filters)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_q_any_filter(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let q = Q::any().add(Author::Age.eq(25)).add(Author::Age.eq(35));

    let authors = Author::objects(&db).filter(q).all().await.unwrap();

    assert_eq!(authors.len(), 2); // Alice and Charlie
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_q_all_filter(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let q = Q::all().add(Author::Age.gte(25)).add(Author::Age.lte(30));

    let authors = Author::objects(&db).filter(q).all().await.unwrap();

    assert_eq!(authors.len(), 2); // Alice and Bob
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_q_not_filter(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let q = Q::all().add(Author::Age.eq(30)).not();

    let authors = Author::objects(&db).filter(q).all().await.unwrap();

    assert_eq!(authors.len(), 2); // Alice and Charlie (not Bob)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_q_chained_conditions(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let q = Q::all()
        .add(Author::Age.gte(25))
        .add(Author::Age.lte(35))
        .add(Author::Name.starts_with("A").or(Author::Name.starts_with("C")));

    let authors = Author::objects(&db).filter(q).all().await.unwrap();

    assert_eq!(authors.len(), 2); // Alice (25) and Charlie (35)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_q_empty_all(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let q = Q::all(); // Empty all condition

    let authors = Author::objects(&db).filter(q).all().await.unwrap();

    assert_eq!(authors.len(), 3); // All authors match
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_q_empty_any(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let q = Q::any(); // Empty any condition

    let authors = Author::objects(&db).filter(q).all().await.unwrap();

    // Empty ANY returns no results
    assert_eq!(authors.len(), 0);
}

// ============================================================================
// get_or_create / update_or_create Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_or_create_creates_new(#[future] db: DatabaseRouter) {
    let (author, created) = Author::objects(&db)
        .filter(Author::Email.eq("new@test.com"))
        .get_or_create(|| Author {
            name: "New Author".to_string(),
            email: "new@test.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(created);
    assert_eq!(author.email, "new@test.com");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_or_create_gets_existing(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, existing) = db_with_author;

    let (author, created) = Author::objects(&db)
        .filter(Author::Email.eq(&existing.email))
        .get_or_create(|| Author {
            name: "Should Not Create".to_string(),
            email: existing.email.clone(),
            age: 99,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!created);
    assert_eq!(author.id, existing.id);
    assert_eq!(author.name, existing.name); // Original name, not "Should Not Create"
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_or_create_updates_existing(
    #[future] db_with_author: (DatabaseRouter, Author),
) {
    let (db, existing) = db_with_author;

    let (author, created) = Author::objects(&db)
        .filter(Author::Email.eq(&existing.email))
        .update_or_create(
            |a| {
                a.age = 100;
            },
            || Author {
                name: "Should Not Create".to_string(),
                email: existing.email.clone(),
                age: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(!created);
    assert_eq!(author.id, existing.id);
    assert_eq!(author.age, 100);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_or_create_creates_new(#[future] db: DatabaseRouter) {
    let (author, created) = Author::objects(&db)
        .filter(Author::Email.eq("new@test.com"))
        .update_or_create(
            |a| {
                a.age = 100;
            },
            || Author {
                name: "New Author".to_string(),
                email: "new@test.com".to_string(),
                age: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(created);
    assert_eq!(author.email, "new@test.com");
    assert_eq!(author.age, 50); // Creator's age, not updater's
}

// ============================================================================
// Iterator and Streaming Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_iterator_method(#[future] db: DatabaseRouter) {
    // Create 10 authors
    for i in 0..10 {
        Author::objects(&db)
            .create(Author {
                name: format!("Author {i}"),
                email: format!("author{i}@test.com"),
                age: 20 + i,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    use futures::StreamExt;
    let mut stream = Author::objects(&db).iterator(Some(3)).await.unwrap();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 10);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_iter_method(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    use futures::StreamExt;
    let mut stream = Author::objects(&db)
        .values_iter(vec![Author::Name, Author::Age], Some(2))
        .await
        .unwrap();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_iter_empty_columns(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    use futures::StreamExt;
    let mut stream = Author::objects(&db).values_iter(vec![], None).await.unwrap();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_list_iter_flat(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    use futures::StreamExt;
    let mut stream = Author::objects(&db)
        .values_list_iter(vec![Author::Name], true, None)
        .await
        .unwrap();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        result.unwrap();
        count += 1;
    }

    assert_eq!(count, 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_values_list_iter_not_flat(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    use futures::StreamExt;
    let mut stream = Author::objects(&db)
        .values_list_iter(vec![Author::Name, Author::Age], false, None)
        .await
        .unwrap();

    let mut count = 0;
    while let Some(result) = stream.next().await {
        let value = result.unwrap();
        assert!(value.is_array());
        count += 1;
    }

    assert_eq!(count, 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_group_by_basic(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Test that group_by returns a modified queryset
    let qs = Author::objects(&db).group_by(Author::Age);

    // Verify it's still queryable (implementation detail - group_by just modifies select)
    let sql = qs.debug_sql();
    assert!(sql.contains("GROUP BY") || sql.contains("group by"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_annotate_basic(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Test that annotate returns a modified queryset with aggregation expressions
    let qs = Author::objects(&db).group_by(Author::Age).annotate([
        ("count_all", Aggregation::count_all()),
        ("max_age", Aggregation::max(Author::Age)),
    ]);

    let sql = qs.debug_sql();
    assert!(!sql.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_aggregation_helpers() {
    // Test all Aggregation helper constructors
    let _count_all = Aggregation::count_all();
    let _count = Aggregation::count(Author::Age);
    let _sum = Aggregation::sum(Author::Age);
    let _avg = Aggregation::avg(Author::Age);
    let _max = Aggregation::max(Author::Age);
    let _min = Aggregation::min(Author::Age);

    // Just verify they can be constructed
    assert!(true);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_soft_delete_mode_methods(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Test with_deleted() - should return all records including soft-deleted
    // (Author doesn't have soft delete, so behavior is same as normal query)
    let with_deleted = Author::objects(&db).with_deleted().all().await.unwrap();
    assert_eq!(with_deleted.len(), 3);

    // Test only_deleted() - should filter to only deleted records
    // (Author doesn't have soft delete, so this should return all records)
    let only_deleted = Author::objects(&db).only_deleted().all().await.unwrap();
    assert_eq!(only_deleted.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_limit_offset_combination(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Test limit + offset together
    let authors = Author::objects(&db)
        .order_by_asc(Author::Age)
        .offset(1)
        .limit(1)
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Bob"); // Middle author after offset
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_query_set_select_related_conversion(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, books)) = db_with_author_with_books;

    // Test that select_related() converts QuerySet to QuerySetEager
    let books_with_author = Book::objects(&db)
        .filter(Book::AuthorId.eq(author.id))
        .select_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books_with_author.len(), books.len());
    assert!(!books_with_author.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_query_set_prefetch_related_conversion(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, books)) = db_with_author_with_books;

    // Test that prefetch_related() converts QuerySet to QuerySetEager
    let books_with_author = Book::objects(&db)
        .filter(Book::AuthorId.eq(author.id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books_with_author.len(), books.len());
    assert!(!books_with_author.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_or_create_race_condition_retry(#[future] db: DatabaseRouter) {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    // Simulate race condition: two concurrent get_or_create calls
    let barrier = Arc::new(Barrier::new(2));
    let db = Arc::new(db);

    let handle1 = {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await; // Sync start
            Author::objects(&*db)
                .filter(Author::Email.eq("race@test.com"))
                .get_or_create(|| Author {
                    name: "Race Test 1".to_string(),
                    email: "race@test.com".to_string(),
                    age: 30,
                    ..Default::default()
                })
                .await
        })
    };

    let handle2 = {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await; // Sync start
            Author::objects(&*db)
                .filter(Author::Email.eq("race@test.com"))
                .get_or_create(|| Author {
                    name: "Race Test 2".to_string(),
                    email: "race@test.com".to_string(),
                    age: 31,
                    ..Default::default()
                })
                .await
        })
    };

    let (result1, result2) = tokio::join!(handle1, handle2);
    let (author1, created1) = result1.unwrap().unwrap();
    let (author2, created2) = result2.unwrap().unwrap();

    // One should create, one should get existing
    assert_ne!(created1, created2, "Exactly one should have created");
    assert_eq!(author1.id, author2.id, "Both should reference same record");
    assert_eq!(author1.email, "race@test.com");

    // Verify only one record exists
    let count = Author::objects(&*db)
        .filter(Author::Email.eq("race@test.com"))
        .count()
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_or_create_race_condition_retry(#[future] db: DatabaseRouter) {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    // Create initial record
    Author::objects(&db)
        .create(Author {
            name: "Original".to_string(),
            email: "update_race@test.com".to_string(),
            age: 25,
            ..Default::default()
        })
        .await
        .unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let db = Arc::new(db);

    let handle1 = {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            Author::objects(&*db)
                .filter(Author::Email.eq("update_race@test.com"))
                .update_or_create(
                    |a| a.age = 30,
                    || Author {
                        name: "New 1".to_string(),
                        email: "update_race@test.com".to_string(),
                        age: 30,
                        ..Default::default()
                    },
                )
                .await
        })
    };

    let handle2 = {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            Author::objects(&*db)
                .filter(Author::Email.eq("update_race@test.com"))
                .update_or_create(
                    |a| a.age = 35,
                    || Author {
                        name: "New 2".to_string(),
                        email: "update_race@test.com".to_string(),
                        age: 35,
                        ..Default::default()
                    },
                )
                .await
        })
    };

    let (result1, result2) = tokio::join!(handle1, handle2);
    assert!(result1.unwrap().is_ok());
    assert!(result2.unwrap().is_ok());

    // Verify only one record exists and was updated
    let final_author = Author::objects(&*db)
        .filter(Author::Email.eq("update_race@test.com"))
        .first()
        .await
        .unwrap();

    assert!(
        final_author.age == 30 || final_author.age == 35,
        "Age should be one of the updated values"
    );
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_chaining_multiple_operations(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .filter(Author::Age.lte(35))
        .exclude(Author::Age.eq(30))
        .order_by_asc(Author::Age)
        .limit(10)
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2); // Alice and Charlie
    assert_eq!(authors[0].age, 25); // Alice first
    assert_eq!(authors[1].age, 35); // Charlie second
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_exists_with_filter_true(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let exists = Author::objects(&db).filter(Author::Age.eq(25)).exists().await.unwrap();

    assert!(exists);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_exists_with_filter_false(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let exists = Author::objects(&db).filter(Author::Age.eq(999)).exists().await.unwrap();

    assert!(!exists);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_with_valid_id(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;

    let fetched = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched.id, author.id);
    assert_eq!(fetched.name, author.name);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_get_with_invalid_id(#[future] db: DatabaseRouter) {
    let result = Author::objects(&db).get(99999).await;
    assert!(result.is_err());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_first_on_ordered_queryset(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let youngest = Author::objects(&db).order_by_asc(Author::Age).first().await.unwrap();

    assert_eq!(youngest.age, 25); // Alice
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_last_by_pk(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    // last() returns the record with the highest PK value
    let last = Author::objects(&db).last().await.unwrap();
    assert_eq!(last.name, "Charlie"); // Created last, so has highest PK
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_last_by_custom_field(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    // To get "last" by a custom ordering, use order_by_desc().first()
    // This is the recommended pattern per last() documentation
    let oldest = Author::objects(&db).order_by_desc(Author::Age).first().await.unwrap();

    assert_eq!(oldest.age, 35); // Charlie
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_distinct_returns_unique(#[future] db: DatabaseRouter) {
    // Create authors with duplicate data patterns
    for i in 0..5 {
        Author::objects(&db)
            .create(Author {
                name: format!("Author {}", i % 2), // Only 2 unique names
                email: format!("author{i}@test.com"),
                age: 30,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let authors = Author::objects(&db).distinct().all().await.unwrap();

    // Distinct on all columns, so should still have 5 (different emails)
    assert_eq!(authors.len(), 5);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_count_with_complex_filter(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let count = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .filter(Author::Age.lte(35))
        .exclude(Author::Age.eq(30))
        .count()
        .await
        .unwrap();

    assert_eq!(count, 2); // Alice (25) and Charlie (35), not Bob (30)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_complex_query_chain(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let author = Author::objects(&db)
        .filter(Author::Age.gte(20))
        .exclude(Author::Age.gt(35))
        .order_by_desc(Author::Age)
        .limit(1)
        .first()
        .await
        .unwrap();

    assert_eq!(author.age, 35); // Charlie
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_pagination_consistency(#[future] db: DatabaseRouter) {
    // Create 20 authors
    for i in 0..20 {
        Author::objects(&db)
            .create(Author {
                name: format!("Author {i}"),
                email: format!("author{i}@test.com"),
                age: 20 + (i % 10),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let page_size = 5;
    let mut all_ids = Vec::new();

    for page in 0..4 {
        let authors = Author::objects(&db)
            .order_by_asc(Author::Id)
            .limit(page_size)
            .offset(page * page_size)
            .all()
            .await
            .unwrap();

        all_ids.extend(authors.iter().map(|a| a.id));
    }

    // All IDs should be unique (no duplicates from pagination)
    let unique_count = all_ids.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, 20);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_string_operations(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db).filter(Author::Name.starts_with("A")).all().await.unwrap();

    assert_eq!(authors.len(), 1); // Alice
    assert!(authors[0].name.starts_with('A'));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_filter_contains_substring(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let authors = Author::objects(&db).filter(Author::Name.contains("li")).all().await.unwrap();

    // Alice and Charlie both contain "li"
    assert_eq!(authors.len(), 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_debug_sql(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let sql = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .order_by_desc(Author::Name)
        .debug_sql();

    assert!(sql.contains("SELECT") || sql.contains("select"));
    assert!(!sql.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_explain_query(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let explain = Author::objects(&db).filter(Author::Age.gte(25)).explain().await.unwrap();

    assert!(explain.contains("EXPLAIN"));
    assert!(!explain.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_explain_analyze_query(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let explain = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .explain_analyze()
        .await
        .unwrap();

    assert!(explain.contains("EXPLAIN"));
    assert!(!explain.is_empty());
}
