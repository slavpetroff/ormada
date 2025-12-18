//! Aggregation Examples - COUNT, SUM, AVG, MIN, MAX

use ormada::prelude::*;

#[ormada_model(table = "agg_books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub price: i32,
    pub sales: i32,
    pub published: bool,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Book::create_table(&router).await?;
    Ok(router)
}

async fn seed_books(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    for (title, price, sales, published) in [
        ("Book A", 1000, 100, true),
        ("Book B", 2000, 200, true),
        ("Book C", 3000, 300, true),
        ("Book D", 4000, 400, false),
        ("Book E", 5000, 500, false),
    ] {
        Book::objects(db)
            .create(Book {
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

pub async fn example_count(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let total = Book::objects(db).count().await?;
    assert_eq!(total, 5);

    let published = Book::objects(db).filter(Book::Published.eq(true)).count().await?;
    assert_eq!(published, 3);

    Ok(())
}

pub async fn example_sum(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let total = Book::objects(db).aggregate_sum(Book::Price).await?;
    assert_eq!(total, Some(15000.0));

    Ok(())
}

pub async fn example_avg(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let avg = Book::objects(db).aggregate_avg(Book::Price).await?;
    assert_eq!(avg, Some(3000.0));

    Ok(())
}

pub async fn example_min_max(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    let min = Book::objects(db).aggregate_min(Book::Price).await?;
    assert_eq!(min, Some(1000.0));

    let max = Book::objects(db).aggregate_max(Book::Price).await?;
    assert_eq!(max, Some(5000.0));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_count() {
        let db = setup_db().await.unwrap();
        example_count(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_sum() {
        let db = setup_db().await.unwrap();
        example_sum(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_avg() {
        let db = setup_db().await.unwrap();
        example_avg(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_min_max() {
        let db = setup_db().await.unwrap();
        example_min_max(&db).await.unwrap();
    }
}
