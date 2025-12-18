//! Transaction Examples - tx! macro

use ormada::prelude::*;

mod author {
    use ormada::prelude::*;

    #[ormada_model(table = "tx_authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
}

mod book {
    use ormada::prelude::*;

    #[ormada_model(table = "tx_books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub author_id: i32,
        pub title: String,
    }
}

pub use author::Author;
pub use book::Book;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Ok(router)
}

/// tx! macro for atomic operations
pub async fn example_tx_macro(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (author, book) = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "Alice".into(),
                ..Default::default()
            })
            .await?;

        let book = Book::objects(txn)
            .create(Book {
                author_id: author.id,
                title: "Alice's Book".into(),
                ..Default::default()
            })
            .await?;

        Ok((author, book))
    })
    .await?;

    assert!(author.id > 0);
    assert_eq!(book.author_id, author.id);
    assert_eq!(Author::objects(db).count().await?, 1);
    assert_eq!(Book::objects(db).count().await?, 1);

    Ok(())
}

/// Transaction rollback on error
pub async fn example_tx_rollback(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let initial = Author::objects(db).count().await?;

    let result: Result<(), OrmadaError> = tx!(db, |txn| async move {
        Author::objects(txn)
            .create(Author {
                name: "Will Rollback".into(),
                ..Default::default()
            })
            .await?;

        Err(OrmadaError::validation_error("test", "field", "Intentional error"))
    })
    .await;

    assert!(result.is_err());
    assert_eq!(Author::objects(db).count().await?, initial, "Should rollback");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tx_macro() {
        let db = setup_db().await.unwrap();
        example_tx_macro(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_tx_rollback() {
        let db = setup_db().await.unwrap();
        example_tx_rollback(&db).await.unwrap();
    }
}
