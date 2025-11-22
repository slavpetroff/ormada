use sea_orm::Database;
use seaorm_django::prelude::*;

/// Test that specifying a non-existent column in ordering causes a compile-time error
/// This test file itself demonstrates the compile-time safety
#[tokio::test]
async fn test_ordering_with_valid_column() {
    pub mod valid_post {
        use super::*;
        #[django_model(table = "valid_posts", ordering = "-created_at")]
        pub struct ValidPost {
            #[primary_key]
            pub id: i32,
            pub title: String,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create table
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(valid_post::Entity);
    use sea_orm::ConnectionTrait;
    let sql = stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql).await.unwrap();

    // This should compile and work fine
    let posts = valid_post::ValidPost::default_ordering(&db).all().await.unwrap();
    assert_eq!(posts.len(), 0);
}

#[tokio::test]
async fn test_ordering_ascending() {
    pub mod asc_post {
        use super::*;
        #[django_model(table = "asc_posts", ordering = "title")]
        pub struct AscPost {
            #[primary_key]
            pub id: i32,
            pub title: String,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(asc_post::Entity);
    use sea_orm::ConnectionTrait;
    let sql = stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql).await.unwrap();

    // Create posts in reverse alphabetical order
    for title in ["Zebra", "Mango", "Apple"] {
        asc_post::AscPost::objects(&db)
            .create(asc_post::AscPost {
                id: 0,
                title: title.to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // default_ordering should return in ASC order by title
    let posts = asc_post::AscPost::default_ordering(&db).all().await.unwrap();
    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0].title, "Apple");
    assert_eq!(posts[1].title, "Mango");
    assert_eq!(posts[2].title, "Zebra");
}

#[tokio::test]
async fn test_ordering_descending() {
    pub mod desc_post {
        use super::*;
        #[django_model(table = "desc_posts", ordering = "-views")]
        pub struct DescPost {
            #[primary_key]
            pub id: i32,
            pub title: String,
            pub views: i32,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(desc_post::Entity);
    use sea_orm::ConnectionTrait;
    let sql = stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql).await.unwrap();

    // Create posts with different view counts
    for (title, views) in [("Low", 10), ("High", 100), ("Medium", 50)] {
        desc_post::DescPost::objects(&db)
            .create(desc_post::DescPost {
                id: 0,
                title: title.to_string(),
                views,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // default_ordering should return in DESC order by views
    let posts = desc_post::DescPost::default_ordering(&db).all().await.unwrap();
    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0].views, 100);
    assert_eq!(posts[1].views, 50);
    assert_eq!(posts[2].views, 10);
}

#[tokio::test]
async fn test_no_ordering_specified() {
    pub mod no_order_post {
        use super::*;
        #[django_model(table = "no_order_posts")]
        pub struct NoOrderPost {
            #[primary_key]
            pub id: i32,
            pub title: String,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create table
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(no_order_post::Entity);
    use sea_orm::ConnectionTrait;
    let sql = stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql).await.unwrap();

    // Models without ordering don't have default_ordering method
    // This is a compile-time check - if we tried to call default_ordering() here,
    // it would fail to compile with "method not found"

    // We can still use objects() normally
    let posts = no_order_post::NoOrderPost::objects(&db).all().await.unwrap();
    assert_eq!(posts.len(), 0);
}

#[tokio::test]
async fn test_ordering_with_filter() {
    pub mod filterable_post {
        use super::*;
        #[django_model(table = "filterable_posts", ordering = "-created_at")]
        pub struct FilterablePost {
            #[primary_key]
            pub id: i32,
            pub title: String,
            pub published: bool,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(filterable_post::Entity);
    use sea_orm::ConnectionTrait;
    let sql = stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql).await.unwrap();

    // Create posts
    for i in 1..=3 {
        filterable_post::FilterablePost::objects(&db)
            .create(filterable_post::FilterablePost {
                id: 0,
                title: format!("Post {}", i),
                published: i % 2 == 0, // Only post 2 is published
                ..Default::default()
            })
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Combine default ordering with filter
    let published = filterable_post::FilterablePost::default_ordering(&db)
        .filter(filterable_post::FilterablePost::Published.eq(true))
        .all()
        .await
        .unwrap();

    assert_eq!(published.len(), 1);
    assert_eq!(published[0].title, "Post 2");
}

#[tokio::test]
async fn test_ordering_override() {
    pub mod override_post {
        use super::*;
        #[django_model(table = "override_posts", ordering = "-created_at")]
        pub struct OverridePost {
            #[primary_key]
            pub id: i32,
            pub title: String,
            pub priority: i32,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();
    use sea_orm::Schema;
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(override_post::Entity);
    use sea_orm::ConnectionTrait;
    let sql = stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder);
    db.execute_unprepared(&sql).await.unwrap();

    // Create posts
    for (title, priority) in [("Low", 1), ("High", 3), ("Medium", 2)] {
        override_post::OverridePost::objects(&db)
            .create(override_post::OverridePost {
                id: 0,
                title: title.to_string(),
                priority,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Default ordering by created_at DESC (newest first)
    let by_default = override_post::OverridePost::default_ordering(&db).all().await.unwrap();
    assert_eq!(by_default[0].title, "Medium");

    // Override with explicit ordering by priority
    let by_priority = override_post::OverridePost::objects(&db)
        .order_by_desc(override_post::OverridePost::Priority)
        .all()
        .await
        .unwrap();
    assert_eq!(by_priority[0].title, "High");
    assert_eq!(by_priority[1].title, "Medium");
    assert_eq!(by_priority[2].title, "Low");
}

// COMPILE-TIME ERROR TEST EXAMPLES (commented out to avoid build failures):
//
// Uncommenting this will cause a COMPILE ERROR because "nonexistent" field doesn't exist:
/*
#[tokio::test]
async fn test_invalid_column_name() {
    pub mod invalid_post {
        use super::*;
        #[django_model(table = "invalid_posts", ordering = "-nonexistent")]
        pub struct InvalidPost {
            #[primary_key]
            pub id: i32,
            pub title: String,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();
    // ERROR: no associated item named `Nonexistent` found for struct `Model`
    let posts = invalid_post::InvalidPost::default_ordering(&db).all().await.unwrap();
}
*/

// This would also cause a compile error due to typo:
/*
#[tokio::test]
async fn test_typo_in_column() {
    pub mod typo_post {
        use super::*;
        #[django_model(table = "typo_posts", ordering = "titel")]  // typo: "titel" instead of "title"
        pub struct TypoPost {
            #[primary_key]
            pub id: i32,
            pub title: String,  // correct spelling
        }
        impl AsyncLifecycleHooks for Model {}
    }

    // ERROR: no associated item named `Titel` found
}
*/
