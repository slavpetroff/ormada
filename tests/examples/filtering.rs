//! Filtering and Query Examples

use ormada::prelude::*;

#[ormada_model(table = "filter_books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub price: i32,
    pub published: bool,
    pub in_stock: bool,
    pub featured: bool,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Book::create_table(&router).await?;
    Ok(router)
}

async fn seed_books(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let books = vec![
        ("Rust Programming", 2999, true, true, true),
        ("Advanced Rust", 3999, true, false, false),
        ("Python Basics", 1999, true, true, false),
        ("The Rust Guide", 4999, false, true, true),
        ("Learning Go", 2499, true, true, false),
    ];

    for (title, price, published, in_stock, featured) in books {
        Book::objects(db)
            .create(Book {
                title: title.into(),
                price,
                published,
                in_stock,
                featured,
                ..Default::default()
            })
            .await?;
    }
    Ok(())
}

pub async fn example_basic_filters(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let expensive = Book::objects(db).filter(Book::Price.gt(3000)).all().await?;
    assert_eq!(expensive.len(), 2);

    let cheap = Book::objects(db).filter(Book::Price.lt(2500)).all().await?;
    assert_eq!(cheap.len(), 2);

    Ok(())
}

pub async fn example_string_filters(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let contains_rust = Book::objects(db).filter(Book::Title.contains("Rust")).all().await?;
    assert_eq!(contains_rust.len(), 3);

    let starts_the = Book::objects(db).filter(Book::Title.starts_with("The")).all().await?;
    assert_eq!(starts_the.len(), 1);

    Ok(())
}

pub async fn example_q_objects(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    // OR conditions
    let q = Q::any().add(Book::Title.contains("Rust")).add(Book::Title.contains("Python"));
    let result = Book::objects(db).filter(q).all().await?;
    assert_eq!(result.len(), 4);

    // AND conditions
    let q = Q::all().add(Book::Published.eq(true)).add(Book::InStock.eq(true));
    let available = Book::objects(db).filter(q).all().await?;
    assert_eq!(available.len(), 3);

    Ok(())
}

pub async fn example_ordering(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let by_price_asc = Book::objects(db).order_by_asc(Book::Price).all().await?;
    assert_eq!(by_price_asc[0].price, 1999);

    let by_price_desc = Book::objects(db).order_by_desc(Book::Price).all().await?;
    assert_eq!(by_price_desc[0].price, 4999);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_filters() {
        let db = setup_db().await.unwrap();
        example_basic_filters(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_string_filters() {
        let db = setup_db().await.unwrap();
        example_string_filters(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_q_objects() {
        let db = setup_db().await.unwrap();
        example_q_objects(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_ordering() {
        let db = setup_db().await.unwrap();
        example_ordering(&db).await.unwrap();
    }
}
