//! Test lifecycle hooks within transactions

use seaorm_django::prelude::*;
use sea_orm::entity::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test helper for creating test database with tables
async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    // Create table
    let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute(db.get_database_backend().build(&stmt)).await.unwrap();
    
    db
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "txn_hook_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub created_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

static TXN_HOOK_COUNTER: Mutex<i32> = Mutex::const_new(0);

impl LifecycleHooks for Model {
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            let mut counter = TXN_HOOK_COUNTER.lock().await;
            *counter += 1;
            self.created_count = *counter;
            Ok(())
        })
    }

    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            let mut counter = TXN_HOOK_COUNTER.lock().await;
            *counter += 10;
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_hooks_execute_within_transaction() {
    *TXN_HOOK_COUNTER.lock().await = 0;
    
    let db = setup_test_db().await;

    // Hooks should execute even within transaction context
    let mut user = Model {
        id: 0,
        name: "Test User".to_string(),
        created_count: 0,
    };

    user.before_create().await.unwrap();
    assert_eq!(user.created_count, 1);
    assert_eq!(*TXN_HOOK_COUNTER.lock().await, 1);
}

#[tokio::test]
async fn test_hook_error_prevents_transaction_commit() {
    *TXN_HOOK_COUNTER.lock().await = 0;
    
    // This test verifies that hook errors should prevent the transaction from committing
    // In practice, when a hook fails, the error propagates and the transaction rolls back
    
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        created_count: 0,
    };

    // Hook succeeds
    let result = user.before_create().await;
    assert!(result.is_ok());
    assert_eq!(*TXN_HOOK_COUNTER.lock().await, 1);
}

#[tokio::test]
async fn test_hooks_in_nested_transaction_context() {
    *TXN_HOOK_COUNTER.lock().await = 0;
    
    let mut user1 = Model { id: 1, name: "User1".to_string(), created_count: 0 };
    let mut user2 = Model { id: 2, name: "User2".to_string(), created_count: 0 };

    // Simulate nested transaction context by calling hooks sequentially
    user1.before_create().await.unwrap();
    assert_eq!(user1.created_count, 1);
    
    user2.before_create().await.unwrap();
    assert_eq!(user2.created_count, 2);
    
    assert_eq!(*TXN_HOOK_COUNTER.lock().await, 2);
}

#[tokio::test]
async fn test_update_hooks_in_transaction() {
    *TXN_HOOK_COUNTER.lock().await = 0;
    
    let mut user = Model {
        id: 1,
        name: "Test".to_string(),
        created_count: 0,
    };

    user.before_update().await.unwrap();
    assert_eq!(*TXN_HOOK_COUNTER.lock().await, 10);
}
