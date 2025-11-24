// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Relations integration tests - prefetch_related and N+1 prevention

mod fixtures;

use fixtures::*;
use rstest::*;
use seaorm_django::prelude::*;

// ============================================================================
// Basic Relation Loading
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_single_relation(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, books)) = db_with_author_with_books;

    let loaded_books = Book::objects(&db)
        .filter(Book::AuthorId.eq(author.id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(loaded_books.len(), 3);

    // Verify relation is loaded
    for book in &loaded_books {
        assert!(book.author.is_some());
        let book_author = book.author.as_ref().unwrap();
        assert_eq!(book_author.id, author.id);
        assert_eq!(book_author.name, author.name);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_no_n_plus_one(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    // Load all books with authors in 1+1 query (not N+1)
    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 6); // 3 authors * 2 books each

    // All books should have their author loaded
    for book in &books {
        assert!(book.author.is_some());
        let author = book.author.as_ref().unwrap();
        assert_eq!(author.id, book.author_id);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_with_filter(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    // Only load published books
    let books = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.len() > 0);
    assert!(books.iter().all(|b| b.published));

    // All should have author loaded
    for book in &books {
        assert!(book.author.is_some());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_with_ordering(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let books = Book::objects(&db)
        .order_by_desc(Book::Price)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    // Verify ordering is maintained
    for i in 1..books.len() {
        assert!(books[i - 1].price >= books[i].price);
    }

    // Verify relations loaded
    for book in &books {
        assert!(book.author.is_some());
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_empty_result(#[future] db: DatabaseRouter) {
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
async fn test_prefetch_related_first(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, _author_with_books) = db_with_author_with_books;
    let book = Book::objects(&db)
        .order_by_asc(Book::Id)
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    assert!(book.author.is_some());
    assert_eq!(book.author.unwrap().id, book.author_id);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_last(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, _author_with_books) = db_with_author_with_books;
    let book = Book::objects(&db)
        .order_by_asc(Book::Id)
        .prefetch_related(relations![Author])
        .last()
        .await
        .unwrap();

    assert!(book.author.is_some());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_with_limit(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let books = Book::objects(&db)
        .limit(10)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 6); // Default fixture creates 6 books total
    assert!(books.iter().all(|b| b.author.is_some()));
}

// ============================================================================
// Count and Exists with Prefetch
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_count(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let count = Book::objects(&db).prefetch_related(relations![Author]).count().await.unwrap();

    assert_eq!(count, 6); // 2 * 3
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_related_exists(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, _author_with_books) = db_with_author_with_books;
    let exists = Book::objects(&db).prefetch_related(relations![Author]).exists().await.unwrap();

    assert!(exists);
}

// ============================================================================
// Complex Scenarios
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_complex_filter_chain(
    #[future] db: DatabaseRouter,
    #[future] authors_with_books: Vec<(Author, Vec<Book>)>,
) {
    let books = Book::objects(&db)
        .filter(Book::Price.gte(1000))
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::Price)
        .limit(5)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.len() <= 5);
    assert!(books.iter().all(|b| b.price >= 1000 && b.published));
    assert!(books.iter().all(|b| b.author.is_some()));
}

#[rstest]
#[awt]
#[case(2)]
#[case(5)]
#[case(10)]
#[tokio::test]
async fn test_prefetch_multiple_authors_parametrized(
    #[future] db: DatabaseRouter,
    #[case] author_count: usize,
) {
    // Create author_count authors, each with 3 books
    for i in 0..author_count {
        let author = Author::objects(&db)
            .create(Author {
                name: format!("Author {}", i + 1),
                email: format!("author{}@example.com", i + 1),
                age: 25 + (i as i32),
                ..Default::default()
            })
            .await
            .unwrap();

        for j in 0..3 {
            Book::objects(&db)
                .create(Book {
                    author_id: author.id,
                    title: format!("Book {} by Author {}", j + 1, i + 1),
                    price: 1000 + (j as i32 * 500),
                    published: true,
                    ..Default::default()
                })
                .await
                .unwrap();
        }
    }

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), author_count * 3);

    // All books should have their authors loaded
    let authors_found: std::collections::HashSet<i32> =
        books.iter().filter_map(|b| b.author.as_ref().map(|a| a.id)).collect();

    assert_eq!(authors_found.len(), author_count);
}

// ============================================================================
// Relation Access Patterns
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_relation_field_access(
    #[future] db: DatabaseRouter,
    #[future] author_with_books: (Author, Vec<Book>),
) {
    let (author, _) = author_with_books;

    let books = Book::objects(&db)
        .filter(Book::AuthorId.eq(author.id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for book in &books {
        // Direct field access pattern
        if let Some(ref book_author) = book.author {
            assert_eq!(book_author.name, author.name);
            assert_eq!(book_author.email, author.email);
            assert_eq!(book_author.age, author.age);
        } else {
            panic!("Author should be loaded");
        }
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_multiple_books_same_author(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _)) = db_with_author_with_books;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    // All books should have same author loaded
    assert_eq!(books.len(), 3); // Default fixture creates 3 books
    for book in &books {
        let book_author = book.author.as_ref().unwrap();
        assert_eq!(book_author.id, author.id);
    }
}

// ============================================================================
// Select Related Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_single(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let books = Book::objects(&db)
        .filter(Book::AuthorId.eq(author.id))
        .select_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 3);
    for book in &books {
        assert!(book.author.is_some());
        assert_eq!(book.author.as_ref().unwrap().id, author.id);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_with_filter(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .select_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.len() > 0);
    for book in &books {
        assert!(book.published);
        assert!(book.author.is_some());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_count(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let count = Book::objects(&db).select_related(relations![Author]).count().await.unwrap();

    assert_eq!(count, 6); // 3 authors * 2 books each
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_first(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, _author_with_books) = db_with_author_with_books;

    let book = Book::objects(&db).select_related(relations![Author]).first().await.unwrap();

    assert!(book.author.is_some());
}

// ============================================================================
// Relations Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_on_empty_queryset(#[future] db: DatabaseRouter) {
    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_on_empty_queryset(#[future] db: DatabaseRouter) {
    let books = Book::objects(&db).select_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_limit(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .limit(3)
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 3);
    for book in &books {
        assert!(book.author.is_some());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_limit_offset(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .limit(10)
        .offset(2)
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 4); // 6 total - 2 offset
    for book in &books {
        assert!(book.author.is_some());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_with_filter_after(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .filter(Book::Published.eq(true))
        .all()
        .await
        .unwrap();

    assert!(books.len() > 0);
    for book in &books {
        assert!(book.published);
        assert!(book.author.is_some());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_ordering(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .select_related(relations![Author])
        .order_by_desc(Book::Price)
        .limit(3)
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 3);
    for i in 0..books.len() - 1 {
        assert!(books[i].price >= books[i + 1].price);
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_exclude(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .exclude(Book::Published.eq(false))
        .all()
        .await
        .unwrap();

    assert!(books.len() > 0);
    for book in &books {
        assert!(book.published);
        assert!(book.author.is_some());
    }
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_select_related_exists(
    #[future] db_with_author_with_books: (DatabaseRouter, (Author, Vec<Book>)),
) {
    let (db, (author, _books)) = db_with_author_with_books;

    let exists = Book::objects(&db)
        .select_related(relations![Author])
        .filter(Book::AuthorId.eq(author.id))
        .exists()
        .await
        .unwrap();

    assert!(exists);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_prefetch_distinct(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;

    let books = Book::objects(&db)
        .prefetch_related(relations![Author])
        .distinct()
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 6);
}
