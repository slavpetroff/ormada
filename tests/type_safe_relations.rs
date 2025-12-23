// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::items_after_statements)]

//! Type-Safe Relation Loading Tests
//!
//! Tests for the phantom type relation loading feature where:
//! - `Model` (from create/update/queries without prefetch) has NO relation fields
//! - `ModelWithRelations` (from `prefetch_related`) HAS relation fields
//!
//! This provides compile-time safety: you can't accidentally access unloaded relations.
//!
//! ## Key Type Safety Features Tested:
//! 1. Model after `create()` has no relation fields
//! 2. Model after `update()` has no relation fields
//! 3. Model from queries without prefetch has no relation fields
//! 4. `ModelWithRelations` from `prefetch_related()` has relation fields
//! 5. `ModelWithRelations` implements Deref to Model for base field access
//! 6. Conversion from Model to `ModelWithRelations` works correctly

mod fixtures;

use fixtures::*;
use ormada::prelude::*;
use rstest::*;

// ============================================================================
// Happy Path: Model (Base Type) Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_after_create_has_only_db_fields(#[future] db: DatabaseRouter) {
    let author = Author::objects(&db)
        .create(Author {
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    // Model has all DB fields
    assert!(author.id > 0);
    assert_eq!(author.name, "Test Author");
    assert_eq!(author.email, "test@example.com");
    assert_eq!(author.age, 30);

    // Create a book - returns Model (not ModelWithRelations)
    let book = Book::objects(&db)
        .create(Book {
            author_id: author.id,
            title: "Test Book".to_string(),
            price: 1999,
            published: true,
            ..Default::default()
        })
        .await
        .unwrap();

    // Book Model has all DB fields
    assert!(book.id > 0);
    assert_eq!(book.author_id, author.id);
    assert_eq!(book.title, "Test Book");
    assert_eq!(book.price, 1999);
    assert!(book.published);

    // COMPILE-TIME SAFETY: book.author does NOT exist on Model
    // See tests/ui/access_relation_on_model.rs for compile-fail test
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_from_query_without_prefetch_has_only_db_fields(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    // Query without prefetch_related returns Model
    let book = Book::objects(&db).filter(Book::AuthorId.eq(author.id)).first().await.unwrap();

    // Book Model has all DB fields
    assert!(book.id > 0);
    assert_eq!(book.author_id, author.id);

    // COMPILE-TIME SAFETY: book.author does NOT exist on Model
    // See tests/ui/access_relation_after_first.rs for compile-fail test
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_from_all_query_without_prefetch(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    // all() without prefetch_related returns Vec<Model>
    let books = Book::objects(&db).all().await.unwrap();

    assert!(!books.is_empty());

    for book in &books {
        // Each book is a Model with DB fields only
        assert!(book.id > 0);
        assert!(book.author_id > 0);
        assert!(!book.title.is_empty());

        // COMPILE-TIME SAFETY: book.author does NOT exist
        // See tests/ui/access_relation_after_all.rs for compile-fail test
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_from_first_query_without_prefetch(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    // first() without prefetch_related returns Model
    let book = Book::objects(&db).first().await.unwrap();

    assert!(book.id > 0);
    assert!(book.author_id > 0);

    // COMPILE-TIME SAFETY: book.author does NOT exist on Model
    // See tests/ui/access_relation_after_first.rs for compile-fail test
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_from_last_query_without_prefetch(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    // last() without prefetch_related returns Model
    let book = Book::objects(&db).last().await.unwrap();

    assert!(book.id > 0);
    assert!(book.author_id > 0);

    // COMPILE-TIME SAFETY: book.author does NOT exist on Model
    // See tests/ui/access_relation_after_first.rs for compile-fail test
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_from_get_query_without_prefetch(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, books)) = db_with_author_with_books;

    let book_id = books[0].id;

    // get() without prefetch_related returns Model
    let book = Book::objects(&db).get(book_id).await.unwrap();

    assert_eq!(book.id, book_id);
    assert!(book.author_id > 0);

    // COMPILE-TIME SAFETY: book.author does NOT exist on Model
}

// ============================================================================
// Happy Path: ModelWithRelations Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_from_prefetch_has_relation_fields(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    // prefetch_related returns ModelWithRelations
    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    // ModelWithRelations has relation fields
    assert_eq!(book.author.id, author.id);
    assert_eq!(book.author.name, author.name);
    assert_eq!(book.author.email, author.email);

    // Also has base Model fields via Deref
    assert!(book.id > 0);
    assert_eq!(book.author_id, author.id);
    assert!(!book.title.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_all_query(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    // all() with prefetch_related returns Vec<ModelWithRelations>
    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert!(!books.is_empty());

    for book in &books {
        // Each book is ModelWithRelations with relation fields
        assert_eq!(book.author.id, author.id);
        assert_eq!(book.author.name, author.name);

        // Also has base fields via Deref
        assert!(book.id > 0);
        assert_eq!(book.author_id, author.id);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_last_query(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    // last() with prefetch_related returns ModelWithRelations
    let book = Book::objects(&db).prefetch_related(relations![Author]).last().await.unwrap();

    assert_eq!(book.author.id, author.id);
    assert!(book.id > 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_with_filter(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, books)) = db_with_author_with_books;

    let target_book = &books[0];

    // Filter + prefetch_related returns ModelWithRelations
    let book = Book::objects(&db)
        .filter(Book::Id.eq(target_book.id))
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    assert_eq!(book.id, target_book.id);
    assert_eq!(book.author.id, author.id);
}

// ============================================================================
// Deref Behavior Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_deref_to_model(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    // Access base Model fields via Deref (no explicit .inner needed)
    assert!(book.id > 0);
    assert_eq!(book.author_id, author.id);
    assert!(!book.title.is_empty());
    assert!(book.price > 0);

    // Access relation fields directly
    assert_eq!(book.author.id, author.id);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_inner_field_access(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    // Can also access inner Model directly
    assert_eq!(book.inner.id, book.id);
    assert_eq!(book.inner.author_id, author.id);
    assert_eq!(book.inner.title, book.title.clone());
}

// ============================================================================
// Nullable FK with ModelWithRelations Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_model_with_relations_some(
    #[future] db_with_articles_all_with_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_all_with_authors;

    // Get articles with authors loaded
    let articles = Article::objects(&db)
        .filter(Article::AuthorId.is_not_null())
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for article in &articles {
        // Nullable FK: relation is Option<Author>
        assert!(article.author.is_some());
        let loaded_author = article.author.as_ref().unwrap();
        assert_eq!(loaded_author.id, author.id);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_model_with_relations_none(
    #[future] db_with_articles_mixed_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, _author, _articles) = db_with_articles_mixed_authors;

    // Get articles without authors
    let articles = Article::objects(&db)
        .filter(Article::AuthorId.is_null())
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for article in &articles {
        // Nullable FK with NULL value: relation is None
        assert!(article.author.is_none());
        assert!(article.author_id.is_none());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_model_without_prefetch(#[future] db: DatabaseRouter) {
    let article = Article::objects(&db)
        .create(Article {
            author_id: None,
            title: "Orphan Article".to_string(),
            content: "Content".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Model has FK field but NOT relation field
    assert!(article.author_id.is_none());
    assert!(article.id > 0);

    // COMPILE-TIME SAFETY: article.author does NOT exist on Model
    // See tests/ui/access_nullable_relation_on_model.rs for compile-fail test
}

// ============================================================================
// Conversion Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_to_model_with_relations_conversion(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    // Get Model (without prefetch)
    let book_model = Book::objects(&db).first().await.unwrap();

    // Convert to ModelWithRelations using From trait
    let book_with_relations: models::book::ModelWithRelations = book_model.into();

    // Base fields are preserved
    assert!(book_with_relations.id > 0);
    assert!(book_with_relations.author_id > 0);

    // Relation field exists but is default (not loaded)
    // For non-nullable FK, author is Default::default()
    assert_eq!(book_with_relations.author.id, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_model_to_model_with_relations_conversion(#[future] db: DatabaseRouter) {
    // Create article without author
    let article_model = Article::objects(&db)
        .create(Article {
            author_id: None,
            title: "Test Article".to_string(),
            content: "Content".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Convert to ModelWithRelations
    let article_with_relations: models::article::ModelWithRelations = article_model.into();

    // Base fields preserved
    assert!(article_with_relations.id > 0);
    assert!(article_with_relations.author_id.is_none());

    // Nullable FK: relation is None (default)
    assert!(article_with_relations.author.is_none());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_empty_result_set_with_prefetch(#[future] db: DatabaseRouter) {
    // Query that returns no results
    let books = Book::objects(&db)
        .filter(Book::Id.eq(99999))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_multiple_books_same_author_with_prefetch(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    // All books have the same author
    for book in &books {
        assert_eq!(book.author.id, author.id);
        assert_eq!(book.author.name, author.name);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_ordering(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .order_by_desc(Book::Id)
        .all()
        .await
        .unwrap();

    // Verify ordering
    for i in 1..books.len() {
        assert!(books[i - 1].id > books[i].id);
    }

    // Relations still loaded correctly
    for book in &books {
        assert_eq!(book.author.id, author.id);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_limit(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .limit(2)
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 2);

    for book in &books {
        assert_eq!(book.author.id, author.id);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_limit_and_offset(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let all_books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    // SQLite requires LIMIT when using OFFSET
    let offset_books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .limit(100)
        .offset(1)
        .all()
        .await
        .unwrap();

    assert_eq!(offset_books.len(), all_books.len() - 1);

    for book in &offset_books {
        assert_eq!(book.author.id, author.id);
    }
}

// ============================================================================
// Type System Verification Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_and_model_with_relations_are_different_types(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    // Get Model
    let book_model = Book::objects(&db).first().await.unwrap();

    // Get ModelWithRelations
    let book_with_relations =
        Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    // They have the same base data
    assert_eq!(book_model.id, book_with_relations.id);
    assert_eq!(book_model.title, book_with_relations.title.clone());
    assert_eq!(book_model.author_id, book_with_relations.author_id);

    // But ModelWithRelations has additional relation field
    assert!(book_with_relations.author.id > 0);

    // Type check: these are different types
    fn takes_model(_: &models::book::Model) {}
    fn takes_model_with_relations(_: &models::book::ModelWithRelations) {}

    takes_model(&book_model);
    takes_model_with_relations(&book_with_relations);

    // ModelWithRelations can be used where Model is expected via Deref
    takes_model(&book_with_relations);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_author_model_without_relations(#[future] db: DatabaseRouter) {
    // Author has no FKs, so Model and ModelWithRelations are structurally similar
    let author = Author::objects(&db)
        .create(Author {
            name: "Test".to_string(),
            email: "test@test.com".to_string(),
            age: 25,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(author.id > 0);
    assert_eq!(author.name, "Test");

    // Author Model works normally
    let fetched = Author::objects(&db).first().await.unwrap();
    assert_eq!(fetched.id, author.id);
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_serialization(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).first().await.unwrap();

    // Model can be serialized
    let json = serde_json::to_string(&book).unwrap();
    assert!(json.contains("\"id\""));
    assert!(json.contains("\"title\""));
    assert!(json.contains("\"author_id\""));

    // No author field in serialized Model
    assert!(!json.contains("\"author\":{"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_serialization(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    // ModelWithRelations can be serialized
    let json = serde_json::to_string(&book).unwrap();
    assert!(json.contains("\"id\""));
    assert!(json.contains("\"title\""));

    // Has author field in serialized output
    assert!(json.contains("\"author\""));
    assert!(json.contains(&format!("\"name\":\"{}\"", author.name)));
}

// ============================================================================
// Clone and Default Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_clone(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).first().await.unwrap();
    let cloned = book.clone();

    assert_eq!(book.id, cloned.id);
    assert_eq!(book.title, cloned.title);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_model_with_relations_clone(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    let cloned = book.clone();

    assert_eq!(book.id, cloned.id);
    assert_eq!(book.author.id, cloned.author.id);
    assert_eq!(book.author.name, author.name);
}

#[test]
fn test_model_default() {
    let book: models::book::Model = Default::default();

    assert_eq!(book.id, 0);
    assert_eq!(book.author_id, 0);
    assert!(book.title.is_empty());
    assert_eq!(book.price, 0);
    assert!(!book.published);
}

#[test]
fn test_model_with_relations_default() {
    let book: models::book::ModelWithRelations = Default::default();

    // Base fields are default
    assert_eq!(book.id, 0);
    assert_eq!(book.author_id, 0);

    // Relation field is default (for non-nullable FK)
    assert_eq!(book.author.id, 0);
}

#[test]
fn test_nullable_fk_model_with_relations_default() {
    let article: models::article::ModelWithRelations = Default::default();

    // Base fields are default
    assert_eq!(article.id, 0);
    assert!(article.author_id.is_none());

    // Nullable relation is None
    assert!(article.author.is_none());
}

// ============================================================================
// FK Validation Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_validation_rejects_zero(#[future] db: DatabaseRouter) {
    // Try to create a book with author_id = 0 (default value)
    let result = Book::objects(&db)
        .create(Book {
            title: "Test Book".to_string(),
            price: 1999,
            published: true,
            ..Default::default() // author_id will be 0
        })
        .await;

    // Should fail with validation error
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    assert!(err_str.contains("foreign key cannot be the default value"));
    assert!(err_str.contains("author_id"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_validation_accepts_valid_value(#[future] db: DatabaseRouter) {
    // Create an author first
    let author = Author::objects(&db)
        .create(Author {
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    // Create a book with valid author_id
    let result = Book::objects(&db)
        .create(Book {
            author_id: author.id,
            title: "Test Book".to_string(),
            price: 1999,
            published: true,
            ..Default::default()
        })
        .await;

    // Should succeed
    assert!(result.is_ok());
    let book = result.unwrap();
    assert_eq!(book.author_id, author.id);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_validation_accepts_none(#[future] db: DatabaseRouter) {
    // Create an article with author_id = None (nullable FK)
    let result = Article::objects(&db)
        .create(Article {
            author_id: None,
            title: "Orphan Article".to_string(),
            content: "Content".to_string(),
            ..Default::default()
        })
        .await;

    // Should succeed - nullable FK can be None
    assert!(result.is_ok());
    let article = result.unwrap();
    assert!(article.author_id.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_validation_accepts_valid_value(
    #[future] db_with_articles_all_with_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_all_with_authors;

    // Create an article with valid author_id
    let result = Article::objects(&db)
        .create(Article {
            author_id: Some(author.id),
            title: "Article with Author".to_string(),
            content: "Content".to_string(),
            ..Default::default()
        })
        .await;

    // Should succeed
    assert!(result.is_ok());
    let article = result.unwrap();
    assert_eq!(article.author_id, Some(author.id));
}
