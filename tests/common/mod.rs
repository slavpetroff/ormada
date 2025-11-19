//! Common test utilities and shared test entities

use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, DbBackend, Schema};
use seaorm_django::prelude::{DateTimeWithTimeZone, DjangoOrmError}; // Explicit imports to avoid ambiguity
use seaorm_django::query::QueryExt; // For .objects() method
use seaorm_django_derive::DjangoModel;

/// Test Author entity module
pub mod author {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Default, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "authors")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub age: i32,

        #[django(auto_now_add)]
        pub created_at: DateTimeWithTimeZone,

        #[django(auto_now)]
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Test Book entity module with relations
pub mod book {
    use super::*;

    // Note: Temporarily not using DjangoModel to avoid macro issues
    // We'll fix the DjangoModel macro separately
    #[derive(Clone, Debug, PartialEq, Eq, Default, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "books")]
    #[django(relations(author = "super::author::Entity"))]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub author_id: i32,
        pub price: i32, // in cents
        pub published: bool,

        #[django(auto_now_add)]
        pub created_at: DateTimeWithTimeZone,

        #[django(auto_now)]
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::author::Entity",
            from = "Column::AuthorId",
            to = "super::author::Column::Id"
        )]
        Author,
    }

    impl Related<super::author::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Author.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// Re-export entity types for convenience in tests
pub use author::Entity as Author;
pub use book::Entity as Book;

/// Setup in-memory SQLite database for testing
pub async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    // Create schema
    let schema = Schema::new(DbBackend::Sqlite);

    let author_stmt = schema.create_table_from_entity(author::Entity);
    let book_stmt = schema.create_table_from_entity(book::Entity);

    db.execute(&author_stmt)
        .await
        .expect("Failed to create authors table");

    db.execute(&book_stmt)
        .await
        .expect("Failed to create books table");

    db
}

/// Create sample authors for testing
pub async fn create_sample_authors(db: &DatabaseConnection) -> Vec<author::Model> {
    let mut authors = Vec::new();

    let author1 = author::Entity::objects(db)
        .create(author::Model {
            name: "Alice Johnson".to_string(),
            email: "alice@example.com".to_string(),
            age: 35,
            ..Default::default()
        })
        .await
        .expect("Failed to insert author");
    authors.push(author1);

    let author2 = author::Entity::objects(db)
        .create(author::Model {
            name: "Bob Smith".to_string(),
            email: "bob@example.com".to_string(),
            age: 42,
            ..Default::default()
        })
        .await
        .expect("Failed to insert author");
    authors.push(author2);

    let author3 = author::Entity::objects(db)
        .create(author::Model {
            name: "Charlie Brown".to_string(),
            email: "charlie@example.com".to_string(),
            age: 28,
            ..Default::default()
        })
        .await
        .expect("Failed to insert author");
    authors.push(author3);

    authors
}

/// Create sample books for testing
pub async fn create_sample_books(db: &DatabaseConnection) -> Vec<book::Model> {
    let mut books = Vec::new();

    // Ensure we have authors first? No, tests usually call create_sample_authors first.
    // But book models need author_id.
    // The existing implementation assumes author_id 1 and 2 exist.

    let book1 = book::Entity::objects(db)
        .create(book::Model {
            title: "Rust Programming".to_string(),
            author_id: 1,
            price: 4999, // $49.99
            published: true,
            ..Default::default()
        })
        .await
        .expect("Failed to insert book");
    books.push(book1);

    let book2 = book::Entity::objects(db)
        .create(book::Model {
            title: "Advanced Rust".to_string(),
            author_id: 1,
            price: 5999,
            published: true,
            ..Default::default()
        })
        .await
        .expect("Failed to insert book");
    books.push(book2);

    let book3 = book::Entity::objects(db)
        .create(book::Model {
            title: "Web Development".to_string(),
            author_id: 2,
            price: 3999,
            published: false,
            ..Default::default()
        })
        .await
        .expect("Failed to insert book");
    books.push(book3);

    books
}
