//! Test error handling in lifecycle hooks

use seaorm_django::prelude::*;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "error_hook_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub should_fail: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl LifecycleHooks for Model {
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            if self.should_fail {
                return Err(DjangoOrmError::Custom("Validation failed in before_create".to_string()));
            }
            self.name = format!("Processed: {}", self.name);
            Ok(())
        })
    }

    fn after_create(&self, _db: &DatabaseConnection) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            if self.should_fail {
                return Err(DjangoOrmError::Custom("Failed to send notification".to_string()));
            }
            Ok(())
        })
    }

    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            if self.should_fail {
                return Err(DjangoOrmError::Custom("Update validation failed".to_string()));
            }
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_before_create_error_propagates() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        should_fail: true,
    };

    let result = user.before_create().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Validation failed"));
}

#[tokio::test]
async fn test_after_create_error_propagates() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    
    let user = Model {
        id: 1,
        name: "Test".to_string(),
        should_fail: true,
    };

    let result = user.after_create(&db).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to send notification"));
}

#[tokio::test]
async fn test_before_update_error_propagates() {
    let mut user = Model {
        id: 1,
        name: "Test".to_string(),
        should_fail: true,
    };

    let result = user.before_update().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Update validation failed"));
}

#[tokio::test]
async fn test_hook_success_when_should_fail_is_false() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        should_fail: false,
    };

    // All hooks should succeed
    assert!(user.before_create().await.is_ok());
    assert_eq!(user.name, "Processed: Test");
    
    let db = Database::connect("sqlite::memory:").await.unwrap();
    assert!(user.after_create(&db).await.is_ok());
    assert!(user.before_update().await.is_ok());
}

#[tokio::test]
async fn test_hook_modifies_model_before_error() {
    let mut user = Model {
        id: 0,
        name: "Original".to_string(),
        should_fail: false,
    };

    user.before_create().await.unwrap();
    assert_eq!(user.name, "Processed: Original");
}

#[tokio::test]
async fn test_multiple_hook_calls_with_errors() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        should_fail: true,
    };

    // First call fails
    assert!(user.before_create().await.is_err());
    
    // Change state and retry
    user.should_fail = false;
    assert!(user.before_create().await.is_ok());
}
