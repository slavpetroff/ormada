// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::indexing_slicing)]

//! FK Nullability Tests
//!
//! Tests for the new FK nullability feature where:
//! - Non-nullable FK (`author_id: i32`) generates direct relation field (`author: Author`)
//! - Nullable FK (`author_id: Option<i32>`) generates optional relation field (`author: Option<Author>`)

mod fixtures;

use fixtures::*;
use ormada::prelude::*;
use rstest::*;

// ============================================================================
// Non-Nullable FK Tests (Book -> Author)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_direct_field_access(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 3);

    for book in &books {
        assert_eq!(book.author.id, author.id);
        assert_eq!(book.author.name, author.name);
        assert_eq!(book.author.email, author.email);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_field_is_not_option(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    let author_name: String = book.author.name.clone();
    assert_eq!(author_name, author.name);

    let author_id: i32 = book.author.id;
    assert_eq!(author_id, author.id);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_multiple_books_same_author(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    let unique_author_ids: std::collections::HashSet<i32> =
        books.iter().map(|b| b.author.id).collect();

    assert_eq!(unique_author_ids.len(), 1);
    assert!(unique_author_ids.contains(&author.id));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_with_select_related(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db).select_related(relations![Author]).all().await.unwrap();

    for book in &books {
        assert_eq!(book.author.id, author.id);
    }
}

// ============================================================================
// Nullable FK Tests (Article -> Author)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_with_author_is_some(
    #[future] db_with_articles_all_with_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_all_with_authors;

    let articles = Article::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(articles.len(), 3);

    for article in &articles {
        assert!(article.author.is_some());
        let article_author = article.author.as_ref().unwrap();
        assert_eq!(article_author.id, author.id);
        assert_eq!(article_author.name, author.name);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_without_author_is_none(
    #[future] db_with_orphan_articles: (DatabaseRouter, Vec<Article>),
) {
    let (db, _articles) = db_with_orphan_articles;

    let articles = Article::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(articles.len(), 3);

    for article in &articles {
        assert!(article.author.is_none());
        assert!(article.author_id.is_none());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_mixed_some_and_none(
    #[future] db_with_articles_mixed_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_mixed_authors;

    let articles = Article::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(articles.len(), 3);

    let with_author: Vec<_> = articles.iter().filter(|a| a.author.is_some()).collect();
    let without_author: Vec<_> = articles.iter().filter(|a| a.author.is_none()).collect();

    assert_eq!(with_author.len(), 2);
    assert_eq!(without_author.len(), 1);

    for article in with_author {
        assert_eq!(article.author.as_ref().unwrap().id, author.id);
    }

    for article in without_author {
        assert!(article.author_id.is_none());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_option_methods_work(
    #[future] db_with_articles_mixed_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_mixed_authors;

    let articles = Article::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    for article in &articles {
        if let Some(article_author) = &article.author {
            assert_eq!(article_author.id, author.id);
        }

        let author_name = article.author.as_ref().map(|a| a.name.clone());
        if article.author_id.is_some() {
            assert_eq!(author_name, Some(author.name.clone()));
        } else {
            assert!(author_name.is_none());
        }
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_with_select_related(
    #[future] db_with_articles_mixed_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_mixed_authors;

    let articles = Article::objects(&db).select_related(relations![Author]).all().await.unwrap();

    assert_eq!(articles.len(), 3);

    for article in &articles {
        match (&article.author_id, &article.author) {
            (Some(fk_id), Some(loaded_author)) => {
                assert_eq!(*fk_id, author.id);
                assert_eq!(loaded_author.id, author.id);
            }
            (None, None) => {
                // Orphan article - both FK and relation are None
            }
            _ => panic!("FK and relation should be consistent"),
        }
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_default_value_before_prefetch(#[future] db: DatabaseRouter) {
    let author = Author::objects(&db)
        .create(Author {
            name: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    let book = Book::objects(&db)
        .create(Book {
            author_id: author.id,
            title: "Test Book".to_string(),
            price: 1000,
            published: true,
            ..Default::default()
        })
        .await
        .unwrap();

    // After create(), book is a Model which does NOT have relation fields
    // This is the key type safety feature - you can't accidentally access unloaded relations
    assert_eq!(book.author_id, author.id); // FK is available
                                           // book.author would be a compile error! (no such field on Model)

    // To get relations, must use prefetch_related which returns ModelWithRelations
    let book_with_author = Book::objects(&db)
        .filter(Book::Id.eq(book.id))
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    // Now we have ModelWithRelations which HAS the author field
    assert_eq!(book_with_author.author.id, author.id);
    assert_eq!(book_with_author.author.name, author.name);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_empty_queryset(#[future] db: DatabaseRouter) {
    let articles = Article::objects(&db)
        .filter(Article::Id.eq(99999))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(articles.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_empty_queryset(#[future] db: DatabaseRouter) {
    let books = Book::objects(&db)
        .filter(Book::Id.eq(99999))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_first_with_author(
    #[future] db_with_articles_all_with_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_all_with_authors;

    let article = Article::objects(&db)
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    assert!(article.author.is_some());
    assert_eq!(article.author.unwrap().id, author.id);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_first_without_author(
    #[future] db_with_orphan_articles: (DatabaseRouter, Vec<Article>),
) {
    let (db, _articles) = db_with_orphan_articles;

    let article = Article::objects(&db)
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    assert!(article.author.is_none());
}

// ============================================================================
// Type Safety Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_type_is_model_not_option(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (_author, _books)) = db_with_author_with_books;

    let book = Book::objects(&db).prefetch_related(relations![Author]).first().await.unwrap();

    fn accepts_author(_author: &Author) {}
    accepts_author(&book.author);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_type_is_option_model(
    #[future] db_with_articles_all_with_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, _author, _articles) = db_with_articles_all_with_authors;

    let article = Article::objects(&db)
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    fn accepts_option_author(_author: &Option<Author>) {}
    accepts_option_author(&article.author);
}

// ============================================================================
// Filter and Order Tests with Relations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_non_nullable_fk_with_filter_and_order(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::Price)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(!books.is_empty());

    for i in 0..books.len().saturating_sub(1) {
        assert!(books[i].price >= books[i + 1].price);
    }

    for book in &books {
        assert!(book.published);
        assert!(book.author.id > 0);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nullable_fk_with_filter_on_fk(
    #[future] db_with_articles_mixed_authors: (DatabaseRouter, Author, Vec<Article>),
) {
    let (db, author, _articles) = db_with_articles_mixed_authors;

    let articles_with_author = Article::objects(&db)
        .filter(Article::AuthorId.eq(Some(author.id)))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(articles_with_author.len(), 2);

    for article in &articles_with_author {
        assert!(article.author.is_some());
        assert_eq!(article.author.as_ref().unwrap().id, author.id);
    }
}
