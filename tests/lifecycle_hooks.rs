// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Lifecycle hooks integration tests

mod fixtures;

use rstest::*;
use sea_orm::ConnectionTrait;
use seaorm_django::prelude::*;
use std::sync::Mutex;

// Model with tracking hooks to verify they're called
pub mod tracked_author {
    use super::*;

    #[django_model(table = "tracked_authors")]
    pub struct TrackedAuthor {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
        #[auto_now]
        pub updated_at: DateTimeWithTimeZone,
    }

    // Track hook calls
    pub static DELETE_HOOK_CALLED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    impl AsyncLifecycleHooks for Model {
        fn before_delete<C: sea_orm::ConnectionTrait>(
            &self,
            _db: &C,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), DjangoOrmError>> + Send + '_>,
        > {
            Box::pin(async move {
                DELETE_HOOK_CALLED.lock().unwrap().push(format!("before_delete:{}", self.name));
                Ok(())
            })
        }
    }
}

pub use tracked_author::TrackedAuthor;

#[fixture]
async fn db() -> DatabaseRouter {
    use sea_orm::{Database, Schema};

    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    let author_stmt = schema.create_table_from_entity(tracked_author::Entity);
    db.execute(&author_stmt).await.unwrap();

    DatabaseRouter::new_single(db)
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_calls_before_delete_hook(#[future] db: DatabaseRouter) {
    // Clear tracking
    tracked_author::DELETE_HOOK_CALLED.lock().unwrap().clear();

    // Create an author
    let author = TrackedAuthor::objects(&db)
        .create(TrackedAuthor {
            id: 0,
            name: "Hook Test".to_string(),
            email: "hook@test.com".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    let author_name = author.name.clone();

    // Delete using DeleteExt trait
    author.delete(&db).await.unwrap();

    // Verify before_delete hook was called
    let calls = tracked_author::DELETE_HOOK_CALLED.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], format!("before_delete:{author_name}"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_hook_prevents_deletion_on_error(#[future] db: DatabaseRouter) {
    // Model with hook that returns error
    mod blocking_author {
        use super::*;

        #[django_model(table = "blocking_authors")]
        pub struct BlockingAuthor {
            #[primary_key]
            pub id: i32,
            pub name: String,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
        }

        impl AsyncLifecycleHooks for Model {
            fn before_delete<C: sea_orm::ConnectionTrait>(
                &self,
                _db: &C,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), DjangoOrmError>> + Send + '_>,
            > {
                Box::pin(async move { Err(DjangoOrmError::Custom("Deletion blocked".into())) })
            }
        }
    }

    use sea_orm::{Database, Schema};
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(blocking_author::Entity);
    db.execute(&stmt).await.unwrap();
    let db_router = DatabaseRouter::new_single(db);

    use blocking_author::BlockingAuthor;

    let author = BlockingAuthor::objects(&db_router)
        .create(BlockingAuthor {
            id: 0,
            name: "Test".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    let id = author.id;

    // Attempt delete - should fail
    let result = author.delete(&db_router).await;
    assert!(result.is_err());

    // Verify author still exists
    let still_exists = BlockingAuthor::objects(&db_router).get(id).await;
    assert!(still_exists.is_ok());
}
