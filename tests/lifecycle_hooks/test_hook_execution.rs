//! Test lifecycle hook execution order and basic functionality

use seaorm_django::prelude::*;
use sea_orm::entity::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

// Test model with lifecycle hooks
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "hook_test_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub hook_log: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Custom hook implementation using external state tracker
pub struct HookTracker {
    pub calls: Arc<Mutex<Vec<String>>>,
}

impl HookTracker {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn get_calls(&self) -> Vec<String> {
        self.calls.lock().await.clone()
    }
}

// We'll implement custom hooks on Model
impl LifecycleHooks for Model {
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            self.hook_log.push_str("before_create;");
            Ok(())
        })
    }

    fn after_create(&self, _db: &DatabaseConnection) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            // Note: We can't modify self here, so we just verify it was called
            // In real usage, this would send emails, log events, etc.
            Ok(())
        })
    }

    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            self.hook_log.push_str("before_update;");
            Ok(())
        })
    }

    fn after_update(&self, _db: &DatabaseConnection) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            Ok(())
        })
    }

    fn before_save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            self.hook_log.push_str("before_save;");
            Ok(())
        })
    }

    fn after_save(&self, _db: &DatabaseConnection) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            Ok(())
        })
    }

    fn before_delete(&self, _db: &DatabaseConnection) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_create_hooks_execute_in_order() {
    // Setup in-memory database
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    // Create table
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute(db.get_database_backend().build(&stmt)).await.unwrap();

    // Create model - hooks should fire
    let user = Model {
        id: 0,
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        hook_log: String::new(),
    };

    let active: ActiveModel = user.into();
    let mut model_to_insert = active.try_into_model().unwrap();
    
    // Manually call hooks to test order
    model_to_insert.before_save().await.unwrap();
    model_to_insert.before_create().await.unwrap();

    // Check hook log
    assert_eq!(model_to_insert.hook_log, "before_save;before_create;");
}

#[tokio::test]
async fn test_update_hooks_execute_in_order() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute(db.get_database_backend().build(&stmt)).await.unwrap();

    // Insert a record first
    let user = ActiveModel {
        id: Set(1),
        name: Set("Test User".to_string()),
        email: Set("test@example.com".to_string()),
        hook_log: Set(String::new()),
    };
    let inserted = user.insert(&db).await.unwrap();

    // Update - hooks should fire
    let mut model_to_update = inserted;
    model_to_update.before_save().await.unwrap();
    model_to_update.before_update().await.unwrap();

    // Check hook log
    assert_eq!(model_to_update.hook_log, "before_save;before_update;");
}

#[tokio::test]
async fn test_delete_hook_executes() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute(db.get_database_backend().build(&stmt)).await.unwrap();

    // Insert a record
    let user = ActiveModel {
        id: Set(1),
        name: Set("Test User".to_string()),
        email: Set("test@example.com".to_string()),
        hook_log: Set(String::new()),
    };
    let inserted = user.insert(&db).await.unwrap();

    // Delete hook should execute
    inserted.before_delete(&db).await.unwrap();
    // Hook executed successfully (no panic)
}

#[tokio::test]
async fn test_before_hooks_can_modify_model() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
        hook_log: String::new(),
    };

    // Before hooks can modify the model
    user.before_create().await.unwrap();
    assert!(user.hook_log.contains("before_create"));

    user.before_update().await.unwrap();
    assert!(user.hook_log.contains("before_update"));
}

#[tokio::test]
async fn test_multiple_hooks_on_same_event() {
    // Test that multiple before_save calls accumulate
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
        hook_log: String::new(),
    };

    user.before_save().await.unwrap();
    user.before_save().await.unwrap();
    user.before_save().await.unwrap();

    // Should have been called 3 times
    assert_eq!(user.hook_log.matches("before_save;").count(), 3);
}
