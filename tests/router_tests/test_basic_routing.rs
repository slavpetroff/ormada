use seaorm_django::prelude::*;

// Test models
pub mod author {
    use super::*;
    #[django_model(table = "authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
    impl AsyncLifecycleHooks for Model {}
}
pub use author::Author;

pub mod book {
    use super::*;
    #[django_model(table = "books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub title: String,
        #[foreign_key(Author)]
        pub author_id: i32,
    }
    impl AsyncLifecycleHooks for Model {}
}
pub use book::Book;

/// Create a test database connection (not wrapped in router)
async fn setup_test_connection() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.expect("Failed to connect");
    
    // Create tables using a temporary router
    let temp_router = DatabaseRouter::new_single(db.clone());
    Author::create_table(&temp_router).await.expect("Failed to create authors table");
    Book::create_table(&temp_router).await.expect("Failed to create books table");
    
    db
}

/// Create a test database router (single connection)
async fn setup_test_router() -> DatabaseRouter {
    let db = setup_test_connection().await;
    DatabaseRouter::new_single(db)
}

#[tokio::test]
async fn test_single_database_works() {
    let router = setup_test_router().await;

    // Create author using router's write connection
    let author = Author::objects(router.write_connection())
        .create(Author {
            id: 0,
            name: "Test Author".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Read using router's read connection
    let authors = Author::objects(router.read_connection().await).all().await.unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Test Author");
    assert_eq!(authors[0].id, author.id);

    // Verify write was tracked
    assert!(router.context().has_write_occurred());
}

#[tokio::test]
async fn test_read_after_write_consistency() {
    let primary = setup_test_connection().await;
    let replica = setup_test_connection().await;
    let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

    // Initially no writes
    assert!(!router.context().has_write_occurred());

    // Create an author (write operation)
    let author = Author::objects(router.write_connection())
        .create(Author {
            id: 0,
            name: "Consistent Author".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Verify write was tracked
    assert!(router.context().has_write_occurred());

    // Read immediately after write - should use primary for consistency
    let read_conn = router.read_connection().await;
    let read_author = Author::objects(read_conn).get(author.id).await.unwrap();

    assert_eq!(read_author.name, "Consistent Author");
    assert_eq!(read_author.id, author.id);
}

#[tokio::test]
async fn test_pure_reads_use_replica() {
    let primary = setup_test_connection().await;
    let replica = setup_test_connection().await;
    let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

    // Pure read - no writes in this context
    assert!(!router.context().has_write_occurred());

    // Read connection should return replica (or primary if no replicas)
    let read_conn = router.read_connection().await;

    // Verify still no writes tracked
    assert!(!router.context().has_write_occurred());

    // Verify we can get connections
    let _primary = router.primary_connection();
    let _read = router.read_connection().await;
}

#[tokio::test]
async fn test_write_connection_marks_context() {
    let primary = setup_test_connection().await;
    let router = DatabaseRouter::new_single(primary);

    // Initially no writes
    assert!(!router.context().has_write_occurred());

    // Get write connection
    let write_conn = router.write_connection();

    // Should mark context
    assert!(router.context().has_write_occurred());

    // Create using write connection
    let author = Author::objects(write_conn)
        .create(Author {
            id: 0,
            name: "Write Test".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(author.name, "Write Test");
}

#[tokio::test]
async fn test_filter_and_query_operations() {
    let router = setup_test_router().await;

    let write_conn = router.write_connection();

    // Create test data
    for i in 1..=3 {
        Author::objects(write_conn)
            .create(Author {
                id: 0,
                name: format!("Author {}", i),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let read_conn = router.read_connection().await;

    // Filter query
    let authors = Author::objects(read_conn)
        .filter(Author::Name.contains("Author"))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 3);

    // Order by
    let sorted = Author::objects(read_conn).order_by_asc(Author::Name).all().await.unwrap();

    assert_eq!(sorted[0].name, "Author 1");
    assert_eq!(sorted[1].name, "Author 2");
    assert_eq!(sorted[2].name, "Author 3");
}

#[tokio::test]
async fn test_consistency_context_reset() {
    let router = setup_test_router().await;

    // Perform write
    Author::objects(router.write_connection())
        .create(Author {
            id: 0,
            name: "Test".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(router.context().has_write_occurred());

    // Reset context
    router.reset_context();

    assert!(!router.context().has_write_occurred());
}

#[tokio::test]
async fn test_foreign_key_with_router() {
    let router = setup_test_router().await;
    let write_conn = router.write_connection();

    // Create author
    let author = Author::objects(write_conn)
        .create(Author {
            id: 0,
            name: "Book Author".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create book with foreign key
    let book = Book::objects(write_conn)
        .create(Book {
            id: 0,
            title: "Test Book".to_string(),
            author_id: author.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(book.author_id, author.id);

    // Query books
    let books = Book::objects(router.read_connection().await)
        .filter(Book::AuthorId.eq(author.id))
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "Test Book");
}
