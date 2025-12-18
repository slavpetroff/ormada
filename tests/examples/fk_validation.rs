//! FK Validation Example
//!
//! - **Compile-time**: Typestate pattern prevents wrong query order
//! - **Runtime**: FK default value validation at create time

use ormada::prelude::*;

mod author {
    use ormada::prelude::*;

    #[ormada_model(table = "fk_authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
}

mod book {
    use ormada::prelude::*;

    #[ormada_model(table = "fk_books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub author_id: i32,
        pub title: String,
    }
}

mod article {
    use ormada::prelude::*;

    #[ormada_model(table = "fk_articles")]
    pub struct Article {
        #[primary_key]
        pub id: i32,
        pub author_id: Option<i32>,
        pub title: String,
    }
}

pub use article::Article;
pub use author::Author;
pub use book::Book;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Article::create_table(&router).await?;
    Ok(router)
}

/// Valid FK usage - always explicitly set FK values
pub async fn example_valid_fk(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let author = Author::objects(db)
        .create(Author {
            name: "Alice".into(),
            ..Default::default()
        })
        .await?;

    let book = Book::objects(db)
        .create(Book {
            author_id: author.id,
            title: "Alice's Book".into(),
            ..Default::default()
        })
        .await?;

    assert_eq!(book.author_id, author.id);
    Ok(())
}

/// Optional FK can be None
pub async fn example_optional_fk(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let article = Article::objects(db)
        .create(Article {
            author_id: None,
            title: "Anonymous".into(),
            ..Default::default()
        })
        .await?;

    assert!(article.author_id.is_none());

    let author = Author::objects(db)
        .create(Author { name: "Bob".into(), ..Default::default() })
        .await?;

    let article_with_author = Article::objects(db)
        .create(Article {
            author_id: Some(author.id),
            title: "Bob's Article".into(),
            ..Default::default()
        })
        .await?;

    assert_eq!(article_with_author.author_id, Some(author.id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_fk() {
        let db = setup_db().await.unwrap();
        example_valid_fk(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_optional_fk() {
        let db = setup_db().await.unwrap();
        example_optional_fk(&db).await.unwrap();
    }
}
