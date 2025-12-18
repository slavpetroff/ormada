//! Projection Examples - Type-safe DTOs instead of JSON

use ormada::prelude::*;
use sea_orm::FromQueryResult;

#[ormada_model(table = "proj_books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub price: i32,
    pub author_name: String,
    pub published: bool,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct BookSummary {
    pub title: String,
    pub price: i32,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Book::create_table(&router).await?;
    Ok(router)
}

async fn seed_books(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    for (title, price, author, published) in [
        ("Rust Programming", 2999, "Alice", true),
        ("Advanced Rust", 3999, "Alice", true),
        ("Python Basics", 1999, "Bob", true),
    ] {
        Book::objects(db)
            .create(Book {
                title: title.into(),
                price,
                author_name: author.into(),
                published,
                ..Default::default()
            })
            .await?;
    }
    Ok(())
}

/// project<T>() for type-safe results instead of JSON
pub async fn example_project_to_dto(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let summaries: Vec<BookSummary> = Book::objects(db)
        .filter(Book::Published.eq(true))
        .project::<BookSummary>()
        .await?;

    assert_eq!(summaries.len(), 3);
    for summary in &summaries {
        assert!(!summary.title.is_empty());
        assert!(summary.price > 0);
    }

    Ok(())
}

/// Comparison: project<T>() vs values()
pub async fn example_project_vs_values(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    // project<T>() - TYPE-SAFE, compile-time checked
    let summaries: Vec<BookSummary> = Book::objects(db).project::<BookSummary>().await?;
    let typed_title: &str = &summaries[0].title;
    let typed_price: i32 = summaries[0].price;

    // values() - Returns JSON (runtime field access)
    let values = Book::objects(db).values(vec![Book::Title, Book::Price]).await?;
    let json_title = values[0]["title"].as_str().unwrap();
    let json_price = values[0]["price"].as_i64().unwrap();

    assert_eq!(typed_title, json_title);
    assert_eq!(typed_price as i64, json_price);

    Ok(())
}

/// project_columns<T>() - Optimized projection with explicit column selection
///
/// Only selects the specified columns, reducing database load for large tables.
pub async fn example_project_columns_optimized(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    // Only SELECT title, price - not all columns from the table
    let summaries: Vec<BookSummary> = Book::objects(db)
        .filter(Book::Published.eq(true))
        .project_columns::<BookSummary>(&[Book::Title, Book::Price])
        .await?;

    assert!(!summaries.is_empty());
    for summary in &summaries {
        assert!(!summary.title.is_empty());
        assert!(summary.price > 0);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_project_to_dto() {
        let db = setup_db().await.unwrap();
        example_project_to_dto(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_project_columns_optimized() {
        let db = setup_db().await.unwrap();
        example_project_columns_optimized(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_project_vs_values() {
        let db = setup_db().await.unwrap();
        example_project_vs_values(&db).await.unwrap();
    }
}
