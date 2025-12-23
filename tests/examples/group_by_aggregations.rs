//! Group By and Aggregation Projection Examples
//!
//! Demonstrates type-safe aggregation queries with GROUP BY and custom DTOs.

use ormada::prelude::*;
use sea_orm::FromQueryResult;

pub mod models {
    pub mod author {
        use ormada::prelude::*;

        #[ormada_model(table = "grp_authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,
            pub name: String,
        }
    }

    pub mod book {

        use ormada::prelude::*;

        #[ormada_model(table = "grp_books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author)]
            pub author_id: i32,
            pub title: String,
            pub price: i32,
            pub sales: i32,
            pub published: bool,
        }
    }
}

pub use models::author::Author;
pub use models::book::Book;

#[derive(Debug, Clone, FromQueryResult)]
pub struct AuthorBookStats {
    pub author_id: i32,
    pub book_count: i64,
    pub total_sales: i64,
    pub avg_price: f64,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct PublishedStats {
    pub published: bool,
    pub count: i64,
    pub total_revenue: i64,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Ok(router)
}

async fn seed_data(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let alice = Author::objects(db)
        .create(Author {
            name: "Alice".into(),
            ..Default::default()
        })
        .await?;

    let bob = Author::objects(db)
        .create(Author { name: "Bob".into(), ..Default::default() })
        .await?;

    for (author_id, title, price, sales, published) in [
        (alice.id, "Rust Basics", 2999, 1000, true),
        (alice.id, "Advanced Rust", 4999, 500, true),
        (alice.id, "Rust Draft", 1999, 0, false),
        (bob.id, "Python Guide", 1999, 2000, true),
        (bob.id, "Go Handbook", 2499, 800, true),
    ] {
        Book::objects(db)
            .create(Book {
                author_id,
                title: title.into(),
                price,
                sales,
                published,
                ..Default::default()
            })
            .await?;
    }

    Ok(())
}

/// Group by `author_id` with multiple aggregations
pub async fn example_group_by_author(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    let stats: Vec<AuthorBookStats> = Book::objects(db)
        .group_by(Book::AuthorId)
        .annotate([
            ("book_count", Aggregation::count_all()),
            ("total_sales", Aggregation::sum(Book::Sales)),
            ("avg_price", Aggregation::avg(Book::Price)),
        ])
        .project::<AuthorBookStats>()
        .await?;

    assert_eq!(stats.len(), 2);

    for stat in &stats {
        assert!(stat.book_count > 0);
        assert!(stat.avg_price > 0.0);
    }

    Ok(())
}

/// Group by boolean field with aggregations
pub async fn example_group_by_published(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    let stats: Vec<PublishedStats> = Book::objects(db)
        .group_by(Book::Published)
        .annotate([
            ("count", Aggregation::count_all()),
            ("total_revenue", Aggregation::sum(Book::Sales)),
        ])
        .project::<PublishedStats>()
        .await?;

    assert_eq!(stats.len(), 2);

    let published_stats = stats.iter().find(|s| s.published).unwrap();
    let unpublished_stats = stats.iter().find(|s| !s.published).unwrap();

    assert_eq!(published_stats.count, 4);
    assert_eq!(unpublished_stats.count, 1);

    Ok(())
}

/// Filter before group by
pub async fn example_filter_then_group(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    let stats: Vec<AuthorBookStats> = Book::objects(db)
        .filter(Book::Published.eq(true))
        .group_by(Book::AuthorId)
        .annotate([
            ("book_count", Aggregation::count_all()),
            ("total_sales", Aggregation::sum(Book::Sales)),
            ("avg_price", Aggregation::avg(Book::Price)),
        ])
        .project::<AuthorBookStats>()
        .await?;

    assert_eq!(stats.len(), 2);

    for stat in &stats {
        assert!(stat.book_count >= 1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_group_by_author() {
        let db = setup_db().await.unwrap();
        example_group_by_author(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_group_by_published() {
        let db = setup_db().await.unwrap();
        example_group_by_published(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_filter_then_group() {
        let db = setup_db().await.unwrap();
        example_filter_then_group(&db).await.unwrap();
    }
}
