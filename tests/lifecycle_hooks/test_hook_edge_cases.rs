//! Test edge cases for lifecycle hooks

use seaorm_django::prelude::*;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "edge_case_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub call_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl LifecycleHooks for Model {
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            self.call_count += 1;
            
            // Prevent infinite recursion
            if self.call_count > 10 {
                return Err(DjangoOrmError::Custom("Recursion limit exceeded".to_string()));
            }
            
            Ok(())
        })
    }

    fn before_save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            self.call_count += 100;
            Ok(())
        })
    }

    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            self.call_count += 1000;
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_hook_recursion_prevention() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        call_count: 0,
    };

    // Call hook multiple times
    for i in 1..=10 {
        user.before_create().await.unwrap();
        assert_eq!(user.call_count, i);
    }

    // 11th call should fail
    let result = user.before_create().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Recursion limit"));
}

#[tokio::test]
async fn test_multiple_hook_types_on_same_model() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        call_count: 0,
    };

    // Call different hooks - each should increment differently
    user.before_save().await.unwrap();
    assert_eq!(user.call_count, 100);

    user.before_create().await.unwrap();
    assert_eq!(user.call_count, 101); // 100 + 1

    user.before_update().await.unwrap();
    assert_eq!(user.call_count, 1101); // 101 + 1000
}

#[tokio::test]
async fn test_hook_with_empty_model_name() {
    let mut user = Model {
        id: 0,
        name: String::new(),
        call_count: 0,
    };

    // Hook should work even with empty strings
    assert!(user.before_create().await.is_ok());
    assert_eq!(user.call_count, 1);
}

#[tokio::test]
async fn test_hook_with_large_id() {
    let mut user = Model {
        id: i32::MAX,
        name: "Test".to_string(),
        call_count: 0,
    };

    assert!(user.before_create().await.is_ok());
    assert_eq!(user.call_count, 1);
}

#[tokio::test]
async fn test_hook_with_negative_id() {
    let mut user = Model {
        id: -1,
        name: "Test".to_string(),
        call_count: 0,
    };

    assert!(user.before_create().await.is_ok());
    assert_eq!(user.call_count, 1);
}

#[tokio::test]
async fn test_hooks_with_unicode_names() {
    let mut user = Model {
        id: 0,
        name: "Test 测试 テスト 🚀".to_string(),
        call_count: 0,
    };

    assert!(user.before_create().await.is_ok());
    assert_eq!(user.call_count, 1);
    assert_eq!(user.name, "Test 测试 テスト 🚀");
}

#[tokio::test]
async fn test_hook_called_on_cloned_model() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        call_count: 0,
    };

    user.before_create().await.unwrap();
    assert_eq!(user.call_count, 1);

    // Clone and call hook again
    let mut cloned = user.clone();
    cloned.before_create().await.unwrap();
    assert_eq!(cloned.call_count, 2);
    
    // Original unchanged
    assert_eq!(user.call_count, 1);
}

#[tokio::test]
async fn test_sequential_hook_calls() {
    let mut user = Model {
        id: 0,
        name: "Test".to_string(),
        call_count: 0,
    };

    // Simulate create flow: before_save -> before_create
    user.before_save().await.unwrap();
    user.before_create().await.unwrap();
    
    // Should be 100 (before_save) + 1 (before_create)
    assert_eq!(user.call_count, 101);
}

#[tokio::test]
async fn test_update_flow_hook_calls() {
    let mut user = Model {
        id: 1,
        name: "Test".to_string(),
        call_count: 0,
    };

    // Simulate update flow: before_save -> before_update
    user.before_save().await.unwrap();
    user.before_update().await.unwrap();
    
    // Should be 100 (before_save) + 1000 (before_update)
    assert_eq!(user.call_count, 1100);
}
