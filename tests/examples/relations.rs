//! Relations Examples - Foreign Keys, One-to-Many, Eager Loading
//!
//! Demonstrates proper FK usage with `#[foreign_key]` decorator and relation loading.

use ormada::prelude::*;

pub mod models {
    pub mod author {
        use ormada::prelude::*;

        #[ormada_model(table = "rel_authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,
            pub name: String,
            pub email: String,
        }
    }

    pub mod book {

        use ormada::prelude::*;

        #[ormada_model(table = "rel_books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author)]
            pub author_id: i32,
            pub title: String,
            pub price: i32,
            pub published: bool,
        }
    }

    pub mod article {

        use ormada::prelude::*;

        #[ormada_model(table = "rel_articles")]
        pub struct Article {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author, on_delete = SetNull)]
            pub author_id: Option<i32>,
            pub title: String,
        }
    }
}

pub use models::article::Article;
pub use models::author::Author;
pub use models::book::Book;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Book::create_table(&router).await?;
    Article::create_table(&router).await?;
    Ok(router)
}

async fn seed_data(db: &DatabaseRouter) -> Result<(Vec<Author>, Vec<Book>), OrmadaError> {
    let mut authors = Vec::new();
    let mut books = Vec::new();

    for (name, email) in [("Alice", "alice@example.com"), ("Bob", "bob@example.com")] {
        let author = Author::objects(db)
            .create(Author {
                name: name.into(),
                email: email.into(),
                ..Default::default()
            })
            .await?;

        for i in 1..=3 {
            let book = Book::objects(db)
                .create(Book {
                    author_id: author.id,
                    title: format!("{name}'s Book {i}"),
                    price: 1000 + i * 100,
                    published: i % 2 == 1,
                    ..Default::default()
                })
                .await?;
            books.push(book);
        }
        authors.push(author);
    }

    Ok((authors, books))
}

/// One-to-Many: Author has many Books
pub async fn example_one_to_many(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let alice = &authors[0];

    let alice_books = Book::objects(db).filter(Book::AuthorId.eq(alice.id)).all().await?;

    assert_eq!(alice_books.len(), 3);
    for book in &alice_books {
        assert_eq!(book.author_id, alice.id);
        assert!(book.title.starts_with("Alice"));
    }

    Ok(())
}

/// Eager Loading with `prefetch_related` - prevents N+1 queries
pub async fn example_prefetch_related(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_data(db).await?;

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await?;

    assert_eq!(books.len(), 6);

    for book in &books {
        assert!(book.author.id > 0);
        assert!(!book.author.name.is_empty());
    }

    Ok(())
}

/// Nullable FK with `on_delete` = `SetNull`
pub async fn example_nullable_fk(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;

    let article_with_author = Article::objects(db)
        .create(Article {
            author_id: Some(authors[0].id),
            title: "Article with author".into(),
            ..Default::default()
        })
        .await?;

    let article_without_author = Article::objects(db)
        .create(Article {
            author_id: None,
            title: "Anonymous article".into(),
            ..Default::default()
        })
        .await?;

    assert_eq!(article_with_author.author_id, Some(authors[0].id));
    assert!(article_without_author.author_id.is_none());

    Ok(())
}

/// Filter by FK field
pub async fn example_filter_by_fk(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (authors, _) = seed_data(db).await?;
    let bob = &authors[1];

    let bob_published = Book::objects(db)
        .filter(Book::AuthorId.eq(bob.id))
        .filter(Book::Published.eq(true))
        .all()
        .await?;

    assert_eq!(bob_published.len(), 2);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_one_to_many() {
        let db = setup_db().await.unwrap();
        example_one_to_many(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_prefetch_related() {
        let db = setup_db().await.unwrap();
        example_prefetch_related(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_nullable_fk() {
        let db = setup_db().await.unwrap();
        example_nullable_fk(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_filter_by_fk() {
        let db = setup_db().await.unwrap();
        example_filter_by_fk(&db).await.unwrap();
    }
}
