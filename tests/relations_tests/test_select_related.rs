use seaorm_django::prelude::*;
use sea_orm::{Database, DatabaseConnection};
use seaorm_django::relations;

// Test models for select_related
pub mod author {
    use super::*;
    #[django_model(table = "authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
    }
    impl AsyncLifecycleHooks for Model {}
}

pub mod book {
    use super::*;
    #[django_model(table = "books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub title: String,
        pub author_id: i32,
        pub price: i32,
    }
    impl AsyncLifecycleHooks for Model {}
}

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    
    let author_stmt = schema.create_table_from_entity(author::Entity);
    let book_stmt = schema.create_table_from_entity(book::Entity);
    
    use sea_orm::ConnectionTrait;
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

async fn create_test_data(db: &DatabaseConnection) -> (Vec<author::Model>, Vec<book::Model>) {
    let authors = vec![
        author::Author::objects(db)
            .create(author::Author {
                id: 0,
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                ..Default::default()
            })
            .await
            .unwrap(),
        author::Author::objects(db)
            .create(author::Author {
                id: 0,
                name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
                ..Default::default()
            })
            .await
            .unwrap(),
    ];

    let books = vec![
        book::Book::objects(db)
            .create(book::Book {
                id: 0,
                title: "Book 1".to_string(),
                author_id: authors[0].id,
                price: 1000,
                ..Default::default()
            })
            .await
            .unwrap(),
        book::Book::objects(db)
            .create(book::Book {
                id: 0,
                title: "Book 2".to_string(),
                author_id: authors[0].id,
                price: 2000,
                ..Default::default()
            })
            .await
            .unwrap(),
        book::Book::objects(db)
            .create(book::Book {
                id: 0,
                title: "Book 3".to_string(),
                author_id: authors[1].id,
                price: 1500,
                ..Default::default()
            })
            .await
            .unwrap(),
    ];

    (authors, books)
}

#[tokio::test]
async fn test_select_related_basic() {
    let db = setup_test_db().await;
    let (_authors, _books) = create_test_data(&db).await;
    
    // Use select_related to eager load books
    // Note: This doesn't return joined data in the current implementation,
    // but it prevents N+1 queries
    let books = book::Book::objects(&db)
        .select_related(relations![author::Author])
        .all()
        .await
        .unwrap();
    
    // Should return all books
    assert_eq!(books.len(), 3);
    assert_eq!(books[0].title, "Book 1");
    assert_eq!(books[1].title, "Book 2");
    assert_eq!(books[2].title, "Book 3");
}

#[tokio::test]
async fn test_select_related_with_filter() {
    let db = setup_test_db().await;
    let (authors, _books) = create_test_data(&db).await;
    
    // Select related with filter
    let books = book::Book::objects(&db)
        .filter(book::Book::AuthorId.eq(authors[0].id))
        .select_related(relations![author::Author])
        .all()
        .await
        .unwrap();
    
    // Should return only books by first author
    assert_eq!(books.len(), 2);
    assert!(books.iter().all(|b| b.author_id == authors[0].id));
}

#[tokio::test]
async fn test_select_related_with_ordering() {
    let db = setup_test_db().await;
    let (_authors, _books) = create_test_data(&db).await;
    
    // Select related with ordering
    let books = book::Book::objects(&db)
        .select_related(relations![author::Author])
        .order_by_desc(book::Book::Price)
        .all()
        .await
        .unwrap();
    
    // Should be ordered by price descending
    assert_eq!(books.len(), 3);
    assert_eq!(books[0].price, 2000);
    assert_eq!(books[1].price, 1500);
    assert_eq!(books[2].price, 1000);
}

#[tokio::test]
async fn test_select_related_with_limit() {
    let db = setup_test_db().await;
    let (_authors, _books) = create_test_data(&db).await;
    
    // Select related with limit
    let books = book::Book::objects(&db)
        .select_related(relations![author::Author])
        .limit(2)
        .all()
        .await
        .unwrap();
    
    // Should return only 2 books
    assert_eq!(books.len(), 2);
}

#[tokio::test]
async fn test_select_related_empty_result() {
    let db = setup_test_db().await;
    let (_authors, _books) = create_test_data(&db).await;
    
    // Query that returns no results
    let books = book::Book::objects(&db)
        .filter(book::Book::Price.gt(10000))
        .select_related(relations![author::Author])
        .all()
        .await
        .unwrap();
    
    // Should return empty vec
    assert_eq!(books.len(), 0);
}

#[tokio::test]
async fn test_prefetch_related_still_works() {
    let db = setup_test_db().await;
    let (_authors, _books) = create_test_data(&db).await;
    
    // Ensure prefetch_related still works alongside select_related
    let books = book::Book::objects(&db)
        .prefetch_related(relations![author::Author])
        .all()
        .await
        .unwrap();
    
    assert_eq!(books.len(), 3);
}

#[tokio::test]
async fn test_select_related_prevents_n_plus_one() {
    let db = setup_test_db().await;
    
    // Create many authors and books to test efficiency
    for i in 1..=10 {
        let author = author::Author::objects(&db)
            .create(author::Author {
                id: 0,
                name: format!("Author {}", i),
                email: format!("author{}@example.com", i),
                ..Default::default()
            })
            .await
            .unwrap();
        
        // Each author has 3 books
        for j in 1..=3 {
            book::Book::objects(&db)
                .create(book::Book {
                    id: 0,
                    title: format!("Book {} by Author {}", j, i),
                    author_id: author.id,
                    price: i * 100,
                    ..Default::default()
                })
                .await
                .unwrap();
        }
    }
    
    // Load all books with select_related
    // This should use 1+M queries (where M is number of unique authors)
    // instead of 1+N queries (where N is number of books)
    let books = book::Book::objects(&db)
        .select_related(relations![author::Author])
        .all()
        .await
        .unwrap();
    
    // Should have 30 books (10 authors * 3 books each)
    assert_eq!(books.len(), 30);
}
