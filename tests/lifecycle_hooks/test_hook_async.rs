//! Test async lifecycle hooks

use seaorm_django::prelude::*;
use sea_orm::entity::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "async_hook_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email_sent: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Track async hook executions
static HOOK_COUNTER: Mutex<u32> = Mutex::const_new(0);

impl LifecycleHooks for Model {
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            // Simulate async operation (e.g., API call)
            sleep(Duration::from_millis(10)).await;
            let mut counter = HOOK_COUNTER.lock().await;
            *counter += 1;
            Ok(())
        })
    }

    fn after_create(&self, _db: &DatabaseConnection) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            // Simulate sending email (async operation)
            sleep(Duration::from_millis(10)).await;
            let mut counter = HOOK_COUNTER.lock().await;
            *counter += 1;
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_async_hooks_execute() {
    // Reset counter
    *HOOK_COUNTER.lock().await = 0;

    let mut user = Model {
        id: 0,
        name: "Test User".to_string(),
        email_sent: false,
    };

    // Execute async hooks
    user.before_create().await.unwrap();
    assert_eq!(*HOOK_COUNTER.lock().await, 1);

    user.after_create(&Database::connect("sqlite::memory:").await.unwrap()).await.unwrap();
    assert_eq!(*HOOK_COUNTER.lock().await, 2);
}

#[tokio::test]
async fn test_async_hooks_run_sequentially() {
    *HOOK_COUNTER.lock().await = 0;
    
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        email_sent: false,
    };

    let start = std::time::Instant::now();
    
    // Both hooks have 10ms delays
    user.before_create().await.unwrap();
    user.after_create(&Database::connect("sqlite::memory:").await.unwrap()).await.unwrap();
    
    let elapsed = start.elapsed();
    
    // Should take at least 20ms (sequential execution)
    assert!(elapsed.as_millis() >= 20);
    assert_eq!(*HOOK_COUNTER.lock().await, 2);
}

#[tokio::test]
async fn test_multiple_async_hooks_in_parallel() {
    *HOOK_COUNTER.lock().await = 0;
    
    let mut user1 = Model { id: 1, name: "User1".to_string(), email_sent: false };
    let mut user2 = Model { id: 2, name: "User2".to_string(), email_sent: false };
    let mut user3 = Model { id: 3, name: "User3".to_string(), email_sent: false };

    // Run hooks in parallel
    let (r1, r2, r3) = tokio::join!(
        user1.before_create(),
        user2.before_create(),
        user3.before_create(),
    );

    assert!(r1.is_ok());
    assert!(r2.is_ok());
    assert!(r3.is_ok());
    assert_eq!(*HOOK_COUNTER.lock().await, 3);
}
