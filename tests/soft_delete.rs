// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Soft delete integration tests

use rstest::*;
use sea_orm::Database;
use seaorm_django::prelude::*;

// Model with soft delete enabled
pub mod soft_article {
    use super::*;

    #[django_model(table = "soft_articles")]
    pub struct SoftArticle {
        #[primary_key]
        pub id: i32,
        pub title: String,
        pub content: String,
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
        #[soft_delete]
        pub deleted_at: Option<DateTimeWithTimeZone>,
    }

    #[async_trait]
    impl LifecycleHooks for Model {}
}

pub use soft_article::SoftArticle;

#[fixture]
async fn db_soft() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(db);

    SoftArticle::create_table(&router).await.unwrap();

    router
}

// ============================================================================
// Soft Delete Basic Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_soft_delete_excludes_deleted_by_default(#[future] db_soft: DatabaseRouter) {
    // Create articles
    let article1 = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Active Article".to_string(),
            content: "This is active".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    let article2 = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Another Active".to_string(),
            content: "Also active".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Soft delete one
    article1.delete(&db_soft).await.unwrap();

    // Default query should exclude deleted
    let articles = SoftArticle::objects(&db_soft).all().await.unwrap();
    assert_eq!(articles.len(), 1);
    assert_eq!(articles[0].title, "Another Active");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_with_deleted_includes_all(#[future] db_soft: DatabaseRouter) {
    // Create articles
    let article1 = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Will Delete".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Keep Active".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Soft delete
    article1.delete(&db_soft).await.unwrap();

    // with_deleted() should include all
    let all_articles = SoftArticle::objects(&db_soft).with_deleted().all().await.unwrap();
    assert_eq!(all_articles.len(), 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_only_deleted_returns_deleted_only(#[future] db_soft: DatabaseRouter) {
    // Create articles
    let article1 = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "To Delete 1".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    let article2 = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "To Delete 2".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Keep Active".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Soft delete two
    article1.delete(&db_soft).await.unwrap();
    article2.delete(&db_soft).await.unwrap();

    // only_deleted() should return only deleted articles
    let deleted = SoftArticle::objects(&db_soft).only_deleted().all().await.unwrap();
    assert_eq!(deleted.len(), 2);
    assert!(deleted.iter().all(|a| a.deleted_at.is_some()));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_restore_soft_deleted_record(#[future] db_soft: DatabaseRouter) {
    let article = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Will Restore".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Soft delete
    let deleted = article.delete(&db_soft).await.unwrap();
    assert!(deleted.deleted_at.is_some());

    // Restore
    let restored = deleted.restore(&db_soft).await.unwrap();
    assert!(restored.deleted_at.is_none());

    // Should be visible in normal queries
    let articles = SoftArticle::objects(&db_soft).all().await.unwrap();
    assert_eq!(articles.len(), 1);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_force_delete_permanently_removes(#[future] db_soft: DatabaseRouter) {
    let article = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Force Delete Me".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    let id = article.id;

    // Force delete (permanent)
    article.force_delete(&db_soft).await.unwrap();

    // Should not exist even with with_deleted()
    let all = SoftArticle::objects(&db_soft).with_deleted().all().await.unwrap();
    assert!(all.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_soft_delete_count_excludes_deleted(#[future] db_soft: DatabaseRouter) {
    let article = SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Count Test".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    SoftArticle::objects(&db_soft)
        .create(SoftArticle {
            id: 0,
            title: "Count Test 2".to_string(),
            content: "Content".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Soft delete one
    article.delete(&db_soft).await.unwrap();

    // Count should only include active
    let count = SoftArticle::objects(&db_soft).count().await.unwrap();
    assert_eq!(count, 1);

    // Count with deleted includes all
    let total = SoftArticle::objects(&db_soft).with_deleted().count().await.unwrap();
    assert_eq!(total, 2);
}
