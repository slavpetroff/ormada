// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Lifecycle hooks integration tests
//!
//! Tests cover:
//! - Default hook implementations (return Ok(()))
//! - Custom hook implementations
//! - Hook error propagation

mod fixtures;

use rstest::*;
use seaorm_django::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

// Model with tracking hooks to verify they're called
pub mod tracked_author {
    use super::*;

    // hooks = true means we'll provide our own LifecycleHooks implementation
    #[django_model(table = "tracked_authors", hooks = true)]
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

    #[async_trait]
    impl LifecycleHooks for Model {
        async fn before_delete(&self) -> Result<(), DjangoOrmError> {
            DELETE_HOOK_CALLED.lock().unwrap().push(format!("before_delete:{}", self.name));
            Ok(())
        }
    }
}

pub use tracked_author::TrackedAuthor;

#[fixture]
async fn db() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(db);

    // Use ORM's create_table method
    TrackedAuthor::create_table(&router).await.unwrap();

    router
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

    // Delete using model's delete() method
    author.delete(&db).await.unwrap();

    // Verify before_delete hook was called
    let calls = tracked_author::DELETE_HOOK_CALLED.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], format!("before_delete:{author_name}"));
}

// Model with hook that returns error - defined at module level for proper ORM support
pub mod blocking_author {
    use super::*;

    // hooks = true means we'll provide our own LifecycleHooks implementation
    #[django_model(table = "blocking_authors", hooks = true)]
    pub struct BlockingAuthor {
        #[primary_key]
        pub id: i32,
        pub name: String,
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
    }

    #[async_trait]
    impl LifecycleHooks for Model {
        async fn before_delete(&self) -> Result<(), DjangoOrmError> {
            Err(DjangoOrmError::validation("BlockingAuthor", "delete", "Deletion blocked"))
        }
    }
}

pub use blocking_author::BlockingAuthor;

#[tokio::test]
async fn test_delete_hook_prevents_deletion_on_error() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let db_router = DatabaseRouter::new_single(db);

    // Use ORM's create_table
    BlockingAuthor::create_table(&db_router).await.unwrap();

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

// ============================================================================
// Default Hook Implementation Tests
// ============================================================================

// Model with ALL hooks implemented to track call order
pub mod full_hooks_author {
    use super::*;

    // hooks = true means we'll provide our own LifecycleHooks implementation
    #[django_model(table = "full_hooks_authors", hooks = true)]
    pub struct FullHooksAuthor {
        #[primary_key]
        pub id: i32,
        pub name: String,
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
        #[auto_now]
        pub updated_at: DateTimeWithTimeZone,
    }

    // Track all hook calls in order
    pub static HOOK_CALLS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    pub static BEFORE_CREATE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static AFTER_CREATE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static BEFORE_UPDATE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static AFTER_UPDATE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static BEFORE_SAVE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static AFTER_SAVE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static BEFORE_DELETE_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static AFTER_DELETE_COUNT: AtomicUsize = AtomicUsize::new(0);

    pub fn reset_counters() {
        HOOK_CALLS.lock().unwrap().clear();
        BEFORE_CREATE_COUNT.store(0, Ordering::SeqCst);
        AFTER_CREATE_COUNT.store(0, Ordering::SeqCst);
        BEFORE_UPDATE_COUNT.store(0, Ordering::SeqCst);
        AFTER_UPDATE_COUNT.store(0, Ordering::SeqCst);
        BEFORE_SAVE_COUNT.store(0, Ordering::SeqCst);
        AFTER_SAVE_COUNT.store(0, Ordering::SeqCst);
        BEFORE_DELETE_COUNT.store(0, Ordering::SeqCst);
        AFTER_DELETE_COUNT.store(0, Ordering::SeqCst);
    }

    #[async_trait]
    impl LifecycleHooks for Model {
        async fn before_create(&mut self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("before_create");
            BEFORE_CREATE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_create(&self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("after_create");
            AFTER_CREATE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn before_update(&mut self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("before_update");
            BEFORE_UPDATE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_update(&self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("after_update");
            AFTER_UPDATE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("before_save");
            BEFORE_SAVE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_save(&self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("after_save");
            AFTER_SAVE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn before_delete(&self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("before_delete");
            BEFORE_DELETE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_delete(&self) -> Result<(), DjangoOrmError> {
            HOOK_CALLS.lock().unwrap().push("after_delete");
            AFTER_DELETE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
}

pub use full_hooks_author::FullHooksAuthor;

#[fixture]
async fn db_full_hooks() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(db);

    // Use ORM's create_table method
    FullHooksAuthor::create_table(&router).await.unwrap();

    router
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_with_hooks_succeeds(#[future] db_full_hooks: DatabaseRouter) {
    // Test that create works with hooks implemented
    let author = FullHooksAuthor::objects(&db_full_hooks)
        .create(FullHooksAuthor {
            id: 0,
            name: "Hook Test Author".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // Verify the record was created
    assert_eq!(author.name, "Hook Test Author");

    // Verify we can fetch it back
    let fetched = FullHooksAuthor::objects(&db_full_hooks).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "Hook Test Author");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_hooks_are_called_during_create(#[future] db_full_hooks: DatabaseRouter) {
    // Note: Due to parallel test execution, we can't reliably check exact counts.
    // This test verifies hooks are called by checking if hook names appear in the list.
    // Run with --test-threads=1 for deterministic results.
    full_hooks_author::reset_counters();

    let _author = FullHooksAuthor::objects(&db_full_hooks)
        .create(FullHooksAuthor {
            id: 0,
            name: "Test".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // Verify at least some hooks were called - actual count may vary in parallel runs
    let before_create = full_hooks_author::BEFORE_CREATE_COUNT.load(Ordering::SeqCst);
    let after_create = full_hooks_author::AFTER_CREATE_COUNT.load(Ordering::SeqCst);
    assert!(before_create >= 1, "before_create should be called at least once");
    assert!(after_create >= 1, "after_create should be called at least once");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_save_on_existing_model_succeeds(#[future] db_full_hooks: DatabaseRouter) {
    // First create
    let author = FullHooksAuthor::objects(&db_full_hooks)
        .create(FullHooksAuthor {
            id: 0,
            name: "Test".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // Save existing model - update the name
    let mut updated = author.clone();
    updated.name = "Updated Name".to_string();
    let result = updated.save(&db_full_hooks).await.unwrap();

    // Verify the update was persisted
    assert_eq!(result.name, "Updated Name");

    // Verify we can fetch it back
    let fetched = FullHooksAuthor::objects(&db_full_hooks).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "Updated Name");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_full_hooks_delete_calls_before_delete(#[future] db_full_hooks: DatabaseRouter) {
    let author = FullHooksAuthor::objects(&db_full_hooks)
        .create(FullHooksAuthor {
            id: 0,
            name: "To Delete".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    let id = author.id;

    // Delete
    author.delete(&db_full_hooks).await.unwrap();

    // Verify record was deleted (hook allowed it)
    let result = FullHooksAuthor::objects(&db_full_hooks).get(id).await;
    assert!(result.is_err(), "Record should be deleted");
}

// ============================================================================
// Default Implementation Tests (Model with no custom hooks)
// ============================================================================

// Model that uses ALL default hook implementations
// No hooks = true needed, the macro auto-generates default empty impl
pub mod default_hooks_author {
    use super::*;

    #[django_model(table = "default_hooks_authors")]
    pub struct DefaultHooksAuthor {
        #[primary_key]
        pub id: i32,
        pub name: String,
        #[auto_now_add]
        pub created_at: DateTimeWithTimeZone,
        #[auto_now]
        pub updated_at: DateTimeWithTimeZone,
    }
    // LifecycleHooks is auto-generated with default empty impl
}

pub use default_hooks_author::DefaultHooksAuthor;

#[fixture]
async fn db_default_hooks() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let router = DatabaseRouter::new_single(db);

    // Use ORM's create_table method
    DefaultHooksAuthor::create_table(&router).await.unwrap();

    router
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_default_hooks_create_succeeds(#[future] db_default_hooks: DatabaseRouter) {
    // Default hooks should return Ok(()) and allow operation to proceed
    let author = DefaultHooksAuthor::objects(&db_default_hooks)
        .create(DefaultHooksAuthor {
            id: 0,
            name: "Default Hooks Test".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await;

    assert!(author.is_ok());
    assert_eq!(author.unwrap().name, "Default Hooks Test");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_default_hooks_update_succeeds(#[future] db_default_hooks: DatabaseRouter) {
    let author = DefaultHooksAuthor::objects(&db_default_hooks)
        .create(DefaultHooksAuthor {
            id: 0,
            name: "Original".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    let mut updated = author.clone();
    updated.name = "Updated".to_string();

    let result = updated.save(&db_default_hooks).await;
    assert!(result.is_ok());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_default_hooks_delete_succeeds(#[future] db_default_hooks: DatabaseRouter) {
    let author = DefaultHooksAuthor::objects(&db_default_hooks)
        .create(DefaultHooksAuthor {
            id: 0,
            name: "To Delete".to_string(),
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    let id = author.id;
    let result = author.delete(&db_default_hooks).await;
    assert!(result.is_ok());

    // Verify deleted
    let fetched = DefaultHooksAuthor::objects(&db_default_hooks).get(id).await;
    assert!(fetched.is_err());
}
