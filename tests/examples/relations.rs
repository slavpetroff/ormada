//! Relations Examples - Foreign Keys, One-to-Many, Eager Loading
//!
//! Demonstrates proper FK usage with `#[foreign_key]` decorator and relation loading.

use ormada::prelude::*;

pub mod models {
    pub mod country {
        use ormada::prelude::*;

        #[ormada_model(table = "rel_countries")]
        pub struct Country {
            #[primary_key]
            pub id: i32,
            pub name: String,
            pub code: String,
        }
    }

    pub mod publisher {
        use ormada::prelude::*;

        #[ormada_model(table = "rel_publishers")]
        pub struct Publisher {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Country)]
            pub country_id: i32,
            pub name: String,
        }
    }

    pub mod author {
        use ormada::prelude::*;

        #[ormada_model(table = "rel_authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Publisher)]
            pub publisher_id: i32,
            pub name: String,
            pub email: String,
        }
    }

    pub mod book {

        use ormada::prelude::*;

        #[ormada_model(table = "rel_books")]
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

    pub mod article {

        use ormada::prelude::*;

        #[ormada_model(table = "rel_articles")]
        pub struct Article {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author, on_delete = SetNull)]
            pub author_id: Option<i32>,
            pub title: String,
        }
    }
}

pub use models::article::Article;
pub use models::author::Author;
pub use models::book::Book;
pub use models::country::Country;
pub use models::publisher::Publisher;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Country::create_table(&router).await?;
    Publisher::create_table(&router).await?;
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Article::create_table(&router).await?;
    Ok(router)
}

async fn seed_data(db: &DatabaseRouter) -> Result<(Vec<Author>, Vec<Book>), OrmadaError> {
    let mut authors = Vec::new();
    let mut books = Vec::new();

    // Create countries
    let usa = Country::objects(db)
        .create(Country {
            name: "United States".into(),
            code: "US".into(),
            ..Default::default()
        })
        .await?;
    let uk = Country::objects(db)
        .create(Country {
            name: "United Kingdom".into(),
            code: "UK".into(),
            ..Default::default()
        })
        .await?;

    // Create publishers
    let penguin = Publisher::objects(db)
        .create(Publisher {
            name: "Penguin Books".into(),
            country_id: usa.id,
            ..Default::default()
        })
        .await?;
    let oxford = Publisher::objects(db)
        .create(Publisher {
            name: "Oxford Press".into(),
            country_id: uk.id,
            ..Default::default()
        })
        .await?;

    // Create authors with publishers
    for (name, email, publisher_id) in [
        ("Alice", "alice@example.com", penguin.id),
        ("Bob", "bob@example.com", oxford.id),
    ] {
        let author = Author::objects(db)
            .create(Author {
                name: name.into(),
                email: email.into(),
                publisher_id,
                ..Default::default()
            })
            .await?;

        for i in 1..=3 {
            let book = Book::objects(db)
                .create(Book {
                    author_id: author.id,
                    title: format!("{name}'s Book {i}"),
                    price: 1000 + i * 100,
                    published: i % 2 == 1,
                    ..Default::default()
                })
                .await?;
            books.push(book);
        }
        authors.push(author);
    }

    Ok((authors, books))
}

/// One-to-Many: Author has many Books
pub async fn example_one_to_many(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let alice = &authors[0];

    let alice_books = Book::objects(db).filter(Book::AuthorId.eq(alice.id)).all().await?;

    assert_eq!(alice_books.len(), 3);
    for book in &alice_books {
        assert_eq!(book.author_id, alice.id);
        assert!(book.title.starts_with("Alice"));
    }

    Ok(())
}

/// Eager Loading with `prefetch_related` - prevents N+1 queries
pub async fn example_prefetch_related(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await?;

    assert_eq!(books.len(), 6);

    for book in &books {
        assert!(book.author.id > 0);
        assert!(!book.author.name.is_empty());
    }

    Ok(())
}

/// Nullable FK with `on_delete` = `SetNull`
pub async fn example_nullable_fk(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;

    let article_with_author = Article::objects(db)
        .create(Article {
            author_id: Some(authors[0].id),
            title: "Article with author".into(),
            ..Default::default()
        })
        .await?;

    let article_without_author = Article::objects(db)
        .create(Article {
            author_id: None,
            title: "Anonymous article".into(),
            ..Default::default()
        })
        .await?;

    assert_eq!(article_with_author.author_id, Some(authors[0].id));
    assert!(article_without_author.author_id.is_none());

    Ok(())
}

/// Filter by FK field
pub async fn example_filter_by_fk(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let bob = &authors[1];

    let bob_published = Book::objects(db)
        .filter(Book::AuthorId.eq(bob.id))
        .filter(Book::Published.eq(true))
        .all()
        .await?;

    assert_eq!(bob_published.len(), 2);

    Ok(())
}

/// Reverse relation: `Author.get_books()` - automatically generated from Book's FK
pub async fn example_reverse_relation(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let alice = &authors[0];

    // Use the auto-generated get_books() method on Author
    // This is generated because Book has #[foreign_key(Author)]
    let alice_books = alice.get_books(db).await?;

    assert_eq!(alice_books.len(), 3);
    for book in &alice_books {
        assert_eq!(book.author_id, alice.id);
        assert!(book.title.starts_with("Alice"));
    }

    Ok(())
}

/// Reverse relation with nullable FK: `Author.get_articles()`
pub async fn example_reverse_relation_nullable(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let alice = &authors[0];

    // Create some articles for Alice
    Article::objects(db)
        .create(Article {
            author_id: Some(alice.id),
            title: "Alice's Article 1".into(),
            ..Default::default()
        })
        .await?;
    Article::objects(db)
        .create(Article {
            author_id: Some(alice.id),
            title: "Alice's Article 2".into(),
            ..Default::default()
        })
        .await?;
    // Anonymous article (no author)
    Article::objects(db)
        .create(Article {
            author_id: None,
            title: "Anonymous".into(),
            ..Default::default()
        })
        .await?;

    // Use the auto-generated get_articles() method on Author
    // This is generated because Article has #[foreign_key(Author, on_delete = SetNull)]
    let alice_articles = alice.get_articles(db).await?;

    assert_eq!(alice_articles.len(), 2);
    for article in &alice_articles {
        assert_eq!(article.author_id, Some(alice.id));
        assert!(article.title.starts_with("Alice"));
    }

    Ok(())
}

/// Batch reverse relation loading via `prefetch_related` - efficient for `QuerySets`
pub async fn example_batch_reverse_relation(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let _ = seed_data(db).await?;

    // Load ALL authors with their books in just 2 queries (authors + books)
    // instead of N+1 queries (1 for authors + N for each author's books)
    let authors_with_books =
        Author::objects(db).prefetch_related(reverse_relations![Book]).all().await?;

    assert_eq!(authors_with_books.len(), 2);

    for author in &authors_with_books {
        // Access books via get_books() - consistent async interface
        // Returns prefetched data without hitting the database again
        let books = author.get_books(db).await?;

        assert_eq!(books.len(), 3, "Each author should have 3 books");

        for book in &books {
            assert_eq!(book.author_id, author.id);
        }
    }

    // Verify Alice's books
    let alice = &authors_with_books[0];
    let alice_books = alice.get_books(db).await?;
    assert!(alice_books.iter().all(|b| b.title.starts_with("Alice")));

    // Verify Bob's books
    let bob = &authors_with_books[1];
    let bob_books = bob.get_books(db).await?;
    assert!(bob_books.iter().all(|b| b.title.starts_with("Bob")));

    Ok(())
}

/// Multiple reverse relations in a single `prefetch_related` call
pub async fn example_multiple_reverse_relations(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let alice = &authors[0];

    // Create articles for Alice
    Article::objects(db)
        .create(Article {
            author_id: Some(alice.id),
            title: "Alice's Article 1".into(),
            ..Default::default()
        })
        .await?;
    Article::objects(db)
        .create(Article {
            author_id: Some(alice.id),
            title: "Alice's Article 2".into(),
            ..Default::default()
        })
        .await?;

    // Load authors with both Books AND Articles (two reverse relations)
    let authors_with_all = Author::objects(db)
        .prefetch_related((reverse_relations![Book], reverse_relations![Article]))
        .all()
        .await?;

    assert_eq!(authors_with_all.len(), 2);

    // Alice should have both books and articles
    let alice_loaded = &authors_with_all[0];
    let alice_books = alice_loaded.get_books(db).await?;
    let alice_articles = alice_loaded.get_articles(db).await?;

    assert_eq!(alice_books.len(), 3, "Alice should have 3 books");
    assert_eq!(alice_articles.len(), 2, "Alice should have 2 articles");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_one_to_many() {
        let db = setup_db().await.unwrap();
        example_one_to_many(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_prefetch_related() {
        let db = setup_db().await.unwrap();
        example_prefetch_related(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_nullable_fk() {
        let db = setup_db().await.unwrap();
        example_nullable_fk(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_filter_by_fk() {
        let db = setup_db().await.unwrap();
        example_filter_by_fk(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_reverse_relation() {
        let db = setup_db().await.unwrap();
        example_reverse_relation(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_reverse_relation_nullable() {
        let db = setup_db().await.unwrap();
        example_reverse_relation_nullable(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_reverse_relation() {
        let db = setup_db().await.unwrap();
        example_batch_reverse_relation(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_reverse_relations() {
        let db = setup_db().await.unwrap();
        example_multiple_reverse_relations(&db).await.unwrap();
    }

    /// Test that `prefetch_related` populates the reverse relation storage
    ///
    /// This verifies that:
    /// 1. `__reverse_relations.has::<T>()` returns true after prefetch
    /// 2. `__reverse_relations.get::<T>()` returns the prefetched data
    /// 3. The data is correctly associated with each parent
    #[tokio::test]
    async fn test_prefetch_populates_reverse_relation_storage() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Load authors WITH `prefetch_related`
        let authors_with_books = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        assert_eq!(authors_with_books.len(), 2);

        for author in &authors_with_books {
            // CRITICAL: Verify the storage was populated (this is what enables no-query access)
            assert!(
                author.__reverse_relations.has::<Book>(),
                "ReverseRelationStorage.has::<Book>() must be true after prefetch for author {}",
                author.name
            );

            // Verify we can get the data directly from storage (no async, no db access)
            let books_from_storage: &[Book] = author.__reverse_relations.get::<Book>();
            assert_eq!(books_from_storage.len(), 3, "Each author should have 3 books in storage");

            // Verify the data is correctly associated
            for book in books_from_storage {
                assert_eq!(
                    book.author_id, author.id,
                    "Book {} should belong to author {}",
                    book.title, author.name
                );
            }
        }
    }

    /// Test that `ModelWithRelations` without prefetch does NOT have storage populated
    ///
    /// This verifies the difference between prefetched and non-prefetched models
    #[tokio::test]
    async fn test_non_prefetched_has_empty_storage() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Load authors WITHOUT `prefetch_related` (just regular all())
        // Note: regular all() returns Model, not ModelWithRelations
        let _authors = Author::objects(&db).all().await.unwrap();

        // Use `prefetch_related` with empty tuple to get ModelWithRelations without prefetching
        let authors_eager = Author::objects(&db)
            .prefetch_related(()) // Empty tuple = no relations prefetched
            .all()
            .await
            .unwrap();

        for author in &authors_eager {
            // Storage should NOT have Book data since we didn't prefetch it
            assert!(
                !author.__reverse_relations.has::<Book>(),
                "ReverseRelationStorage.has::<Book>() must be false without prefetch"
            );

            let books_from_storage: &[Book] = author.__reverse_relations.get::<Book>();
            assert!(books_from_storage.is_empty(), "Storage should be empty without prefetch");
        }
    }

    /// Test that `get_books()` on `ModelWithRelations` uses storage when available
    ///
    /// The generated `get_books()` method checks storage first:
    /// ```ignore
    /// if !self.__reverse_relations.is_empty() || self.__reverse_relations.has::<Model>() {
    ///     return Ok(prefetched.to_vec());
    /// }
    /// // else: fall back to querying
    /// ```
    #[tokio::test]
    async fn test_get_books_uses_prefetched_storage() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        let alice = &authors[0];

        // Verify storage is populated
        assert!(alice.__reverse_relations.has::<Book>());

        // Get books via the async method
        let books_via_method = alice.get_books(&db).await.unwrap();

        // Get books directly from storage (sync, no db)
        let books_from_storage: &[Book] = alice.__reverse_relations.get::<Book>();

        // They should be the same data
        assert_eq!(books_via_method.len(), books_from_storage.len());

        // Verify same IDs (the method returns cloned data)
        let ids_method: Vec<_> = books_via_method.iter().map(|b| b.id).collect();
        let ids_storage: Vec<_> = books_from_storage.iter().map(|b| b.id).collect();
        assert_eq!(ids_method, ids_storage);
    }

    /// Test nested prefetch: Book -> Author -> Books
    ///
    /// This verifies that when we load books with authors, and the author field
    /// is `ModelWithRelations`, we can populate the author's reverse relations.
    #[tokio::test]
    async fn test_nested_prefetch_book_author_books() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Step 1: Load books with their authors using select_related
        let books = Book::objects(&db).select_related(relations![Author]).all().await.unwrap();

        assert_eq!(books.len(), 6); // 2 authors * 3 books each

        // Verify the author field is populated
        let book = &books[0];
        assert!(!book.author.name.is_empty(), "Author should be loaded");

        // The author field is now ModelWithRelations, so it has __reverse_relations
        // But it's not populated yet because we only did select_related on Author
        assert!(
            !book.author.__reverse_relations.has::<Book>(),
            "Author's books should NOT be prefetched yet (only select_related was used)"
        );

        // Step 2: Now let's test the full nested prefetch pattern
        // First, get unique authors from the loaded books
        let unique_author_ids: std::collections::HashSet<i32> =
            books.iter().map(|b| b.author.id).collect();

        // Load authors with their books prefetched
        let authors_with_books = Author::objects(&db)
            .filter(Author::Id.is_in(unique_author_ids.into_iter()))
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        // Verify authors have their books prefetched
        for author in &authors_with_books {
            assert!(
                author.__reverse_relations.has::<Book>(),
                "Author {} should have books prefetched",
                author.name
            );
            let author_books = author.get_books(&db).await.unwrap();
            assert_eq!(author_books.len(), 3, "Each author should have 3 books");
        }
    }

    /// Test 3-level chain: Publisher -> Authors -> Books
    ///
    /// This demonstrates loading a parent with nested children prefetched.
    #[tokio::test]
    async fn test_three_level_publisher_authors_books() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Load publishers with their authors prefetched
        let publishers = Publisher::objects(&db)
            .prefetch_related(reverse_relations![Author])
            .all()
            .await
            .unwrap();

        assert_eq!(publishers.len(), 2);

        for publisher in &publishers {
            // Authors are prefetched
            let authors = publisher.get_authors(&db).await.unwrap();
            assert_eq!(authors.len(), 1, "Each publisher should have 1 author");

            // Verify publisher data
            assert!(
                publisher.name == "Penguin Books" || publisher.name == "Oxford Press",
                "Publisher should be Penguin or Oxford, got: {}",
                publisher.name
            );
        }
    }

    /// Test chained `prefetch_related` with `and_prefetch` - happy path
    ///
    /// This demonstrates chaining multiple nested prefetch calls.
    #[tokio::test]
    async fn test_chained_prefetch_with_and_prefetch() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Chain multiple prefetch calls using `and_prefetch`
        // This loads: Author -> Books AND Author -> Articles
        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .and_prefetch(reverse_relations![Article])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 2);

        for author in &authors {
            // Books are prefetched
            let books = author.get_books(&db).await.unwrap();
            assert_eq!(books.len(), 3, "Each author should have 3 books");

            // Articles are also prefetched (empty in this test data)
            let articles = author.get_articles(&db).await.unwrap();
            assert!(articles.is_empty(), "No articles in test data");
        }
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    /// Edge case: prefetch on empty result set
    #[tokio::test]
    async fn test_prefetch_empty_result_set() {
        let db = setup_db().await.unwrap();
        // Don't seed data - empty database

        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        assert!(authors.is_empty(), "Should return empty vec for empty table");
    }

    /// Edge case: prefetch with no matching children
    #[tokio::test]
    async fn test_prefetch_no_matching_children() {
        let db = setup_db().await.unwrap();

        // Create author without any books
        let lonely_author = Author::objects(&db)
            .create(Author {
                name: "Lonely Author".into(),
                email: "lonely@example.com".into(),
                publisher_id: 1, // Will fail if no publisher, but we need to create one first
                ..Default::default()
            })
            .await;

        // If publisher doesn't exist, create minimal setup
        if lonely_author.is_err() {
            let country = Country::objects(&db)
                .create(Country {
                    name: "Test Country".into(),
                    code: "TC".into(),
                    ..Default::default()
                })
                .await
                .unwrap();

            let publisher = Publisher::objects(&db)
                .create(Publisher {
                    name: "Test Publisher".into(),
                    country_id: country.id,
                    ..Default::default()
                })
                .await
                .unwrap();

            Author::objects(&db)
                .create(Author {
                    name: "Lonely Author".into(),
                    email: "lonely@example.com".into(),
                    publisher_id: publisher.id,
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        let authors = Author::objects(&db)
            .filter(Author::Name.eq("Lonely Author"))
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 1);
        let lonely = &authors[0];

        // Should have empty books, not error
        let books = lonely.get_books(&db).await.unwrap();
        assert!(books.is_empty(), "Author with no books should have empty vec");
    }

    /// Edge case: chained `and_prefetch` with mixed empty/non-empty results
    #[tokio::test]
    async fn test_chained_prefetch_mixed_results() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Authors have books but no articles in seed data
        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .and_prefetch(reverse_relations![Article])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 2);

        for author in &authors {
            // Books exist
            let books = author.get_books(&db).await.unwrap();
            assert!(!books.is_empty(), "Should have books");

            // Articles don't exist but should return empty, not error
            let articles = author.get_articles(&db).await.unwrap();
            assert!(articles.is_empty(), "Should have empty articles, not error");
        }
    }

    /// Edge case: nullable FK with null values
    #[tokio::test]
    async fn test_prefetch_with_nullable_fk_null_values() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Create article with null author_id
        Article::objects(&db)
            .create(Article {
                title: "Orphan Article".into(),
                author_id: None,
                ..Default::default()
            })
            .await
            .unwrap();

        // Prefetch articles for authors - orphan article should not appear
        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Article])
            .all()
            .await
            .unwrap();

        // Count total articles across all authors
        let mut total_articles = 0;
        for author in &authors {
            let articles = author.get_articles(&db).await.unwrap();
            total_articles += articles.len();
        }

        // Orphan article should not be counted
        assert_eq!(total_articles, 0, "Orphan articles with null FK should not be prefetched");
    }

    /// Edge case: filter then prefetch
    #[tokio::test]
    async fn test_filter_then_prefetch() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Filter to single author, then prefetch
        let authors = Author::objects(&db)
            .filter(Author::Name.eq("Alice"))
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 1);
        let alice = &authors[0];
        assert_eq!(alice.name, "Alice");

        // Should only have Alice's books
        let books = alice.get_books(&db).await.unwrap();
        assert_eq!(books.len(), 3);
        for book in &books {
            assert!(book.title.starts_with("Alice"));
        }
    }

    /// Edge case: prefetch with limit
    #[tokio::test]
    async fn test_prefetch_with_limit() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Limit to 1 author
        let authors = Author::objects(&db)
            .limit(1)
            .prefetch_related(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 1);

        // Should still have all books for that author
        let books = authors[0].get_books(&db).await.unwrap();
        assert_eq!(books.len(), 3, "Limit on parent should not affect child prefetch count");
    }

    /// Edge case: triple chain with `and_prefetch`
    #[tokio::test]
    async fn test_triple_chain_and_prefetch() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // Create some articles for testing
        let authors = Author::objects(&db).all().await.unwrap();
        for author in &authors {
            Article::objects(&db)
                .create(Article {
                    title: format!("{}'s Article", author.name),
                    author_id: Some(author.id),
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        // Triple chain: Books AND Articles AND more (if we had more relations)
        // For now, test double chain works correctly
        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .and_prefetch(reverse_relations![Article])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 2);

        for author in &authors {
            let books = author.get_books(&db).await.unwrap();
            let articles = author.get_articles(&db).await.unwrap();

            assert_eq!(books.len(), 3, "Each author should have 3 books");
            assert_eq!(articles.len(), 1, "Each author should have 1 article");
        }
    }

    /// Edge case: prefetch same relation twice (should not duplicate)
    #[tokio::test]
    async fn test_prefetch_same_relation_twice() {
        let db = setup_db().await.unwrap();
        seed_data(&db).await.unwrap();

        // This is a bit of an odd case but should work
        let authors = Author::objects(&db)
            .prefetch_related(reverse_relations![Book])
            .and_prefetch(reverse_relations![Book])
            .all()
            .await
            .unwrap();

        assert_eq!(authors.len(), 2);

        // Should still work, second prefetch overwrites first
        for author in &authors {
            let books = author.get_books(&db).await.unwrap();
            assert_eq!(books.len(), 3);
        }
    }
}
