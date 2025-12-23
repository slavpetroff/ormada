//! Soft Delete Example

use ormada::prelude::*;

#[ormada_model(table = "sd_articles")]
pub struct Article {
    #[primary_key]
    pub id: i32,
    pub title: String,
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Article::create_table(&router).await?;
    Ok(router)
}

async fn seed_articles(db: &DatabaseRouter) -> Result<Vec<Article>, OrmadaError> {
    let mut articles = Vec::new();
    for i in 1..=5 {
        let article = Article::objects(db)
            .create(Article {
                title: format!("Article {i}"),
                deleted_at: None,
                ..Default::default()
            })
            .await?;
        articles.push(article);
    }
    Ok(articles)
}

pub async fn example_soft_delete(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let articles = seed_articles(db).await?;

    // Use model's delete method for soft delete (clone to take ownership)
    articles[0].clone().delete(db).await?;

    let visible = Article::objects(db).all().await?;
    assert_eq!(visible.len(), 4, "Soft-deleted excluded by default");

    Ok(())
}

pub async fn example_with_deleted(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let articles = seed_articles(db).await?;

    // Soft delete 2 articles using the model's delete method
    articles[0].clone().delete(db).await?;
    articles[1].clone().delete(db).await?;

    let visible = Article::objects(db).all().await?;
    assert_eq!(visible.len(), 3, "Default query excludes soft-deleted");

    let all = Article::objects(db).with_deleted().all().await?;
    assert_eq!(all.len(), 5, "with_deleted() includes soft-deleted");

    Ok(())
}

pub async fn example_restore(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let articles = seed_articles(db).await?;
    let article_id = articles[0].id;

    // Soft delete
    articles[0].clone().delete(db).await?;
    assert_eq!(Article::objects(db).all().await?.len(), 4);

    // Restore by updating the deleted record
    Article::objects(db)
        .with_deleted()
        .filter(Article::Id.eq(article_id))
        .update(|mut a| async move {
            a.deleted_at = None;
            Ok(a)
        })
        .await?;

    assert_eq!(Article::objects(db).all().await?.len(), 5, "Restored");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_soft_delete() {
        let db = setup_db().await.unwrap();
        example_soft_delete(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_with_deleted() {
        let db = setup_db().await.unwrap();
        example_with_deleted(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_restore() {
        let db = setup_db().await.unwrap();
        example_restore(&db).await.unwrap();
    }
}
