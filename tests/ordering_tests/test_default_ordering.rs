use chrono::{DateTime, FixedOffset, Utc};
use seaorm_django::prelude::*;

// Test model WITH default ordering
pub mod post {
    use super::*;
    #[django_model(table = "posts", ordering = "-created_at")]
    pub struct Post {
        #[primary_key]
        pub id: i32,
        pub title: String,
        pub views: i32,
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
    }
    impl AsyncLifecycleHooks for Model {}
}
pub use post::Post;

// Test model WITHOUT default ordering
pub mod comment {
    use super::*;
    #[django_model(table = "comments")]
    pub struct Comment {
        #[primary_key]
        pub id: i32,
        pub text: String,
        pub post_id: i32,
    }
    impl AsyncLifecycleHooks for Model {}
}
pub use comment::Comment;

async fn setup_test_db() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");
    let router = DatabaseRouter::new_single(db);

    // Create tables using our ORM interface
    Post::create_table(&router).await.expect("Failed to create posts table");
    Comment::create_table(&router).await.expect("Failed to create comments table");

    router
}

#[tokio::test]
async fn test_default_ordering_method_exists() {
    let db = setup_test_db().await;

    // Create posts with different timestamps
    for i in 1..=3 {
        Post::objects(&db)
            .create(Post {
                id: i,
                title: format!("Post {}", i),
                views: i * 10,
                ..Default::default()
            })
            .await
            .unwrap();

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Use default_ordering() method
    let posts = Post::default_ordering(&db).all().await.unwrap();

    // Should be ordered by created_at DESC (newest first)
    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0].id, 3, "Newest post should be first");
    assert_eq!(posts[1].id, 2);
    assert_eq!(posts[2].id, 1, "Oldest post should be last");
}

#[tokio::test]
async fn test_default_ordering_with_filter() {
    let db = setup_test_db().await;

    // Create posts
    for i in 1..=5 {
        Post::objects(&db)
            .create(Post {
                id: i,
                title: format!("Post {}", i),
                views: i * 10,
                ..Default::default()
            })
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Default ordering with filter
    let posts = Post::default_ordering(&db).filter(Post::Views.gte(30)).all().await.unwrap();

    // Should have posts 3,4,5 in DESC order (5,4,3)
    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0].id, 5);
    assert_eq!(posts[1].id, 4);
    assert_eq!(posts[2].id, 3);
}

#[tokio::test]
async fn test_explicit_ordering_overrides_default() {
    let db = setup_test_db().await;

    // Create posts
    for i in 1..=3 {
        Post::objects(&db)
            .create(Post {
                id: i,
                title: format!("Post {}", i),
                views: (4 - i) * 10, // Reverse views: 30, 20, 10
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Explicit ordering should override default
    let posts = Post::objects(&db).order_by_asc(Post::Views).all().await.unwrap();

    // Should be ordered by views ASC
    assert_eq!(posts[0].views, 10);
    assert_eq!(posts[1].views, 20);
    assert_eq!(posts[2].views, 30);
}

#[tokio::test]
async fn test_model_without_default_ordering() {
    let db = setup_test_db().await;

    // Create comments
    for i in 1..=3 {
        Comment::objects(&db)
            .create(Comment {
                id: i,
                text: format!("Comment {}", i),
                post_id: 1,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Model without default ordering - just returns objects
    let comments = Comment::objects(&db).all().await.unwrap();

    // No guaranteed order (database default)
    assert_eq!(comments.len(), 3);
}

#[tokio::test]
async fn test_ascending_default_ordering() {
    // Create a model with ASC ordering
    pub mod article {
        use super::*;
        #[django_model(table = "articles", ordering = "title")] // No '-' means ASC
        pub struct Article {
            #[primary_key]
            pub id: i32,
            pub title: String,
        }
        impl AsyncLifecycleHooks for Model {}
    }

    let db = Database::connect("sqlite::memory:").await.unwrap();
    let db = DatabaseRouter::new_single(db);
    article::Article::create_table(&db).await.unwrap();

    // Create articles in random order
    for title in ["Zebra", "Apple", "Mango"] {
        article::Article::objects(&db)
            .create(article::Article {
                id: 0,
                title: title.to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Use default ordering (should be ASC by title)
    let articles = article::Article::default_ordering(&db).all().await.unwrap();

    assert_eq!(articles[0].title, "Apple");
    assert_eq!(articles[1].title, "Mango");
    assert_eq!(articles[2].title, "Zebra");
}
