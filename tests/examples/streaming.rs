//! Streaming and Iterator Examples

use futures::StreamExt;
use ormada::prelude::*;

#[ormada_model(table = "stream_books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub price: i32,
    pub published: bool,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Book::create_table(&router).await?;
    Ok(router)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
async fn seed_books(db: &DatabaseRouter, count: usize) -> Result<(), OrmadaError> {
    let books: Vec<Book> = (0..count)
        .map(|i| Book {
            title: format!("Book {i}"),
            price: 1000 + (i as i32 * 10),
            published: i % 2 == 0,
            ..Default::default()
        })
        .collect();
    Book::objects(db).bulk_create(books).await?;
    Ok(())
}

/// Stream full models with chunked fetching
pub async fn example_iterator(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db, 100).await?;

    let mut stream = Book::objects(db).filter(Book::Published.eq(true)).iterator(Some(10)).await?;

    let mut count = 0;
    while let Some(result) = stream.next().await {
        let book = result?;
        assert!(book.published);
        count += 1;
    }
    assert_eq!(count, 50);

    Ok(())
}

/// `earliest()` and `latest()` by field
pub async fn example_earliest_latest(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db, 20).await?;

    let earliest = Book::objects(db).earliest(Book::Id).await?;
    assert_eq!(earliest.title, "Book 0");

    let latest = Book::objects(db).latest(Book::Id).await?;
    assert_eq!(latest.title, "Book 19");

    let cheapest = Book::objects(db).earliest(Book::Price).await?;
    assert_eq!(cheapest.price, 1000);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_iterator() {
        let db = setup_db().await.unwrap();
        example_iterator(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_earliest_latest() {
        let db = setup_db().await.unwrap();
        example_earliest_latest(&db).await.unwrap();
    }
}
