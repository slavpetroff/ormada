//! Many-to-Many Relationship Examples
//!
//! Demonstrates M:N relationships using the `#[many_to_many]` decorator
//! with a through table (join model).
//!
//! ## Pattern: Article <-> ArticleTag <-> Tag
//!
//! - Article has many Tags through ArticleTag
//! - Tag has many Articles through ArticleTag
//!
//! ## Key Decorators:
//!
//! - `#[many_to_many(Tag, through = ArticleTag)]` - Declares M:N on Article
//! - `#[foreign_key(Article)]` / `#[foreign_key(Tag)]` - On the join table
//!
//! The `#[many_to_many]` decorator generates helper methods like `get_tags()`
//! for ergonomic M:N queries.

use ormada::prelude::*;

pub mod models {
    pub mod tag {
        use ormada::prelude::*;

        #[ormada_model(table = "m2m_tags")]
        pub struct Tag {
            #[primary_key]
            pub id: i32,
            pub name: String,
        }
    }

    pub mod article_tag {
        use ormada::prelude::*;

        /// Through table for M:N relationship between Article and Tag.
        /// Uses `#[foreign_key]` to both sides to establish the relationship.
        #[ormada_model(table = "m2m_article_tags")]
        pub struct ArticleTag {
            #[primary_key]
            pub id: i32,
            #[foreign_key(super::article::Article)]
            pub article_id: i32,
            #[foreign_key(super::tag::Tag)]
            pub tag_id: i32,
        }
    }

    pub mod article {

        use ormada::prelude::*;

        /// Article model with M:N relationship to Tag through ArticleTag.
        /// The `#[many_to_many]` decorator generates `get_tags()` helper method.
        #[ormada_model(table = "m2m_articles")]
        pub struct Article {
            #[primary_key]
            pub id: i32,
            pub title: String,
            pub content: String,

            /// M:N relationship: Article has many Tags through ArticleTag
            #[many_to_many(Tag, through = ArticleTag)]
            pub tags: Vec<i32>,
        }
    }
}

pub use models::article::Article;
pub use models::article_tag::ArticleTag;
pub use models::tag::Tag;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Tag::create_table(&router).await?;
    Article::create_table(&router).await?;
    ArticleTag::create_table(&router).await?;
    Ok(router)
}

async fn seed_data(db: &DatabaseRouter) -> Result<(Vec<Tag>, Vec<Article>), OrmadaError> {
    let mut tags = Vec::new();
    for name in ["Rust", "Programming", "Database", "ORM"] {
        let tag = Tag::objects(db).create(Tag { name: name.into(), ..Default::default() }).await?;
        tags.push(tag);
    }

    let mut articles = Vec::new();
    for (title, content) in [
        ("Intro to Rust", "Learn Rust basics..."),
        ("Advanced ORM", "Deep dive into ORMs..."),
        ("Database Design", "Best practices for DB design..."),
    ] {
        let article = Article::objects(db)
            .create(Article {
                title: title.into(),
                content: content.into(),
                ..Default::default()
            })
            .await?;
        articles.push(article);
    }

    Ok((tags, articles))
}

/// Create M:N relationships through the join table
pub async fn example_create_m2m(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Article 0 (Intro to Rust) has tags: Rust, Programming
    ArticleTag::objects(db)
        .create(ArticleTag {
            article_id: articles[0].id,
            tag_id: tags[0].id, // Rust
            ..Default::default()
        })
        .await?;
    ArticleTag::objects(db)
        .create(ArticleTag {
            article_id: articles[0].id,
            tag_id: tags[1].id, // Programming
            ..Default::default()
        })
        .await?;

    // Article 1 (Advanced ORM) has tags: Programming, Database, ORM
    for tag_idx in [1, 2, 3] {
        ArticleTag::objects(db)
            .create(ArticleTag {
                article_id: articles[1].id,
                tag_id: tags[tag_idx].id,
                ..Default::default()
            })
            .await?;
    }

    let all_links = ArticleTag::objects(db).all().await?;
    assert_eq!(all_links.len(), 5);

    Ok(())
}

/// Query articles by tag through the join table
pub async fn example_query_by_tag(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Link articles to tags
    ArticleTag::objects(db)
        .create(ArticleTag {
            article_id: articles[0].id,
            tag_id: tags[0].id,
            ..Default::default()
        })
        .await?;
    ArticleTag::objects(db)
        .create(ArticleTag {
            article_id: articles[1].id,
            tag_id: tags[0].id,
            ..Default::default()
        })
        .await?;

    // Find all article IDs with the "Rust" tag
    let rust_tag = &tags[0];
    let article_tags =
        ArticleTag::objects(db).filter(ArticleTag::TagId.eq(rust_tag.id)).all().await?;

    assert_eq!(article_tags.len(), 2);

    // Get the actual articles
    let article_ids: Vec<i32> = article_tags.iter().map(|at| at.article_id).collect();
    let rust_articles = Article::objects(db).filter(Article::Id.is_in(article_ids)).all().await?;

    assert_eq!(rust_articles.len(), 2);

    Ok(())
}

/// Query tags for a specific article
pub async fn example_query_tags_for_article(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Link article 0 to multiple tags
    for tag_idx in [0, 1, 2] {
        ArticleTag::objects(db)
            .create(ArticleTag {
                article_id: articles[0].id,
                tag_id: tags[tag_idx].id,
                ..Default::default()
            })
            .await?;
    }

    // Get all tags for article 0
    let article_tags = ArticleTag::objects(db)
        .filter(ArticleTag::ArticleId.eq(articles[0].id))
        .all()
        .await?;

    let tag_ids: Vec<i32> = article_tags.iter().map(|at| at.tag_id).collect();
    let article_0_tags = Tag::objects(db).filter(Tag::Id.is_in(tag_ids)).all().await?;

    assert_eq!(article_0_tags.len(), 3);

    Ok(())
}

/// Load join table with related Article using prefetch_related
pub async fn example_prefetch_article_from_join(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Create links
    for article in &articles {
        ArticleTag::objects(db)
            .create(ArticleTag {
                article_id: article.id,
                tag_id: tags[0].id,
                ..Default::default()
            })
            .await?;
    }

    // Load join table entries with their Articles eagerly loaded
    let article_tags = ArticleTag::objects(db)
        .filter(ArticleTag::TagId.eq(tags[0].id))
        .prefetch_related(relations![Article])
        .all()
        .await?;

    assert_eq!(article_tags.len(), 3);

    // Access the related Article directly - no N+1!
    for at in &article_tags {
        assert!(at.article.id > 0);
        assert!(!at.article.title.is_empty());
    }

    Ok(())
}

/// Load join table with related Tag using prefetch_related
pub async fn example_prefetch_tag_from_join(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Create links - article 0 has multiple tags
    for tag in &tags[0..3] {
        ArticleTag::objects(db)
            .create(ArticleTag {
                article_id: articles[0].id,
                tag_id: tag.id,
                ..Default::default()
            })
            .await?;
    }

    // Load join table entries with their Tags eagerly loaded
    let article_tags = ArticleTag::objects(db)
        .filter(ArticleTag::ArticleId.eq(articles[0].id))
        .prefetch_related(relations![Tag])
        .all()
        .await?;

    assert_eq!(article_tags.len(), 3);

    // Access the related Tag directly - no N+1!
    for at in &article_tags {
        assert!(at.tag.id > 0);
        assert!(!at.tag.name.is_empty());
    }

    Ok(())
}

/// Helper function to get all tags for an article (common M:N pattern)
pub async fn get_tags_for_article(
    db: &DatabaseRouter,
    article_id: i32,
) -> Result<Vec<Tag>, OrmadaError> {
    let article_tags = ArticleTag::objects(db)
        .filter(ArticleTag::ArticleId.eq(article_id))
        .prefetch_related(relations![Tag])
        .all()
        .await?;

    Ok(article_tags.into_iter().map(|at| at.tag).collect())
}

/// Helper function to get all articles for a tag (common M:N pattern)
pub async fn get_articles_for_tag(
    db: &DatabaseRouter,
    tag_id: i32,
) -> Result<Vec<Article>, OrmadaError> {
    let article_tags = ArticleTag::objects(db)
        .filter(ArticleTag::TagId.eq(tag_id))
        .prefetch_related(relations![Article])
        .all()
        .await?;

    Ok(article_tags.into_iter().map(|at| at.article).collect())
}

/// Demonstrate helper functions for M:N queries
pub async fn example_m2m_helper_functions(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Link article 0 to tags 0, 1, 2
    for tag in &tags[0..3] {
        ArticleTag::objects(db)
            .create(ArticleTag {
                article_id: articles[0].id,
                tag_id: tag.id,
                ..Default::default()
            })
            .await?;
    }

    // Link articles 0, 1 to tag 0
    ArticleTag::objects(db)
        .create(ArticleTag {
            article_id: articles[1].id,
            tag_id: tags[0].id,
            ..Default::default()
        })
        .await?;

    // Get tags for article 0
    let article_0_tags = get_tags_for_article(db, articles[0].id).await?;
    assert_eq!(article_0_tags.len(), 3);

    // Get articles for tag 0
    let tag_0_articles = get_articles_for_tag(db, tags[0].id).await?;
    assert_eq!(tag_0_articles.len(), 2);

    Ok(())
}

/// Demonstrate the generated `get_tags()` method from `#[many_to_many]` decorator
///
/// The `#[many_to_many(Tag, through = ArticleTag)]` decorator on Article
/// generates a `get_tags()` method that queries the through table automatically.
pub async fn example_m2m_decorator_method(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let (tags, articles) = seed_data(db).await?;

    // Link article 0 to tags 0, 1, 2
    for tag in &tags[0..3] {
        ArticleTag::objects(db)
            .create(ArticleTag {
                article_id: articles[0].id,
                tag_id: tag.id,
                ..Default::default()
            })
            .await?;
    }

    // Use the generated get_tags() method from the #[many_to_many] decorator
    let article = Article::objects(db).get(articles[0].id).await?;
    let article_tags = article.get_tags(db).await?;

    assert_eq!(article_tags.len(), 3);

    // Verify the tags are correct
    let tag_names: Vec<&str> = article_tags.iter().map(|t| t.name.as_str()).collect();
    assert!(tag_names.contains(&"Rust"));
    assert!(tag_names.contains(&"Programming"));
    assert!(tag_names.contains(&"Database"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_m2m() {
        let db = setup_db().await.unwrap();
        example_create_m2m(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_query_by_tag() {
        let db = setup_db().await.unwrap();
        example_query_by_tag(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_query_tags_for_article() {
        let db = setup_db().await.unwrap();
        example_query_tags_for_article(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_prefetch_article_from_join() {
        let db = setup_db().await.unwrap();
        example_prefetch_article_from_join(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_prefetch_tag_from_join() {
        let db = setup_db().await.unwrap();
        example_prefetch_tag_from_join(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_m2m_helper_functions() {
        let db = setup_db().await.unwrap();
        example_m2m_helper_functions(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_m2m_decorator_method() {
        let db = setup_db().await.unwrap();
        example_m2m_decorator_method(&db).await.unwrap();
    }
}
