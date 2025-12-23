//! Upsert Operations Example - `get_or_create` and `update_or_create`

use ormada::prelude::*;

#[ormada_model(table = "upsert_authors")]
pub struct Author {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Author::create_table(&router).await?;
    Ok(router)
}

/// `get_or_create` - Get existing or create new (thread-safe)
pub async fn example_get_or_create(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    // First call creates
    let (author1, created1) = Author::objects(db)
        .filter(Author::Email.eq("alice@example.com"))
        .get_or_create(|| async {
            Ok(Author {
                name: "Alice".into(),
                email: "alice@example.com".into(),
                ..Default::default()
            })
        })
        .await?;

    assert!(created1);
    assert!(author1.id > 0);

    // Second call gets existing
    let (author2, created2) = Author::objects(db)
        .filter(Author::Email.eq("alice@example.com"))
        .get_or_create(|| async {
            Ok(Author {
                name: "Different".into(),
                email: "alice@example.com".into(),
                ..Default::default()
            })
        })
        .await?;

    assert!(!created2);
    assert_eq!(author2.id, author1.id);
    assert_eq!(author2.name, "Alice"); // Name unchanged

    Ok(())
}

/// `update_or_create` - Update existing or create new
pub async fn example_update_or_create(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    // First call creates
    let (author1, created1) = Author::objects(db)
        .filter(Author::Email.eq("bob@example.com"))
        .update_or_create(
            |mut author| async move {
                author.name = "Bob Updated".into();
                Ok(author)
            },
            || async {
                Ok(Author {
                    name: "Bob".into(),
                    email: "bob@example.com".into(),
                    ..Default::default()
                })
            },
        )
        .await?;

    assert!(created1);
    assert_eq!(author1.name, "Bob");

    // Second call updates
    let (author2, created2) = Author::objects(db)
        .filter(Author::Email.eq("bob@example.com"))
        .update_or_create(
            |mut author| async move {
                author.name = "Bob Updated".into();
                Ok(author)
            },
            || async {
                Ok(Author {
                    name: "Bob New".into(),
                    email: "bob@example.com".into(),
                    ..Default::default()
                })
            },
        )
        .await?;

    assert!(!created2);
    assert_eq!(author2.id, author1.id);
    assert_eq!(author2.name, "Bob Updated");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_or_create() {
        let db = setup_db().await.unwrap();
        example_get_or_create(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_update_or_create() {
        let db = setup_db().await.unwrap();
        example_update_or_create(&db).await.unwrap();
    }
}
