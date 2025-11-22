//! Test ergonomic lifecycle hooks with #[django_hooks] macro

use sea_orm::ConnectionTrait;
use seaorm_django::prelude::*;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::Mutex;

// Test model
#[django_model(table = "ergonomic_users")]
pub struct User {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_count: i32,
}

// Counter for testing hooks
static HOOK_COUNTER: Mutex<i32> = Mutex::const_new(0);
// Mutex to serialize tests that use the shared HOOK_COUNTER
static TEST_SERIALIZER: Mutex<()> = Mutex::const_new(());

// CLEAN ERGONOMIC HOOKS - Users implement AsyncLifecycleHooks, write async fn bodies!
impl AsyncLifecycleHooks for Model {
    fn before_save(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            println!(
                "before_save called: created_count={} -> {}",
                self.created_count,
                self.created_count + 1
            );
            self.created_count += 1;
            Ok(())
        })
    }

    fn before_create(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            println!("before_create called");
            let mut counter = HOOK_COUNTER.lock().await;
            *counter += 1;
            Ok(())
        })
    }

    fn after_create<C: sea_orm::ConnectionTrait>(
        &self,
        _db: &C,
    ) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async move {
            println!("after_create called");
            let mut counter = HOOK_COUNTER.lock().await;
            *counter += 10;
            Ok(())
        })
    }
}

impl Model {
    pub fn get_display_name(&self) -> String {
        format!("{} <{}>", self.name, self.email)
    }
}

#[tokio::test]
async fn test_hooks_with_create_operation() {
    let _guard = TEST_SERIALIZER.lock().await;
    *HOOK_COUNTER.lock().await = 0;

    // Use OUR ORM API!
    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create table using OUR schema builder
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute_unprepared(&stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder))
        .await
        .unwrap();

    // Use OUR ORM .objects().create() - hooks fire automatically!
    let user = User::objects(&db)
        .create(Model {
            id: 0,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            created_count: 0,
        })
        .await
        .unwrap();

    println!("User created_count: {}", user.created_count);
    println!("HOOK_COUNTER: {}", *HOOK_COUNTER.lock().await);

    // Hooks fired: before_save(+1) + before_create(+1) + after_create(+10)
    assert_eq!(user.created_count, 1, "before_save should have incremented created_count");
    assert_eq!(
        *HOOK_COUNTER.lock().await,
        11,
        "before_create (+1) + after_create (+10) should equal 11"
    );
}

#[tokio::test]
async fn test_hooks_with_save_operation() {
    let _guard = TEST_SERIALIZER.lock().await;
    *HOOK_COUNTER.lock().await = 0;

    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute_unprepared(&stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder))
        .await
        .unwrap();

    // Create first
    let user = User::objects(&db)
        .create(Model {
            id: 0,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            created_count: 0,
        })
        .await
        .unwrap();

    // Reset counter
    *HOOK_COUNTER.lock().await = 0;

    // Use OUR ORM .save() - hooks fire automatically!
    let updated = user.save(&db).await.unwrap();

    // Fetch from DB to verify persistence
    let fetched = User::objects(&db).get(updated.id).await.unwrap();
    println!("Fetched from DB created_count: {}", fetched.created_count);

    // before_save fired again
    assert_eq!(
        updated.created_count, 2,
        "before_save should have incremented again (returned model)"
    );
    assert_eq!(
        fetched.created_count, 2,
        "before_save should have incremented again (db record)"
    );
}

#[tokio::test]
async fn test_regular_methods_work() {
    let user = Model {
        id: 1,
        name: "John".to_string(),
        email: "john@example.com".to_string(),
        created_count: 0,
    };

    assert_eq!(user.get_display_name(), "John <john@example.com>");
}

#[tokio::test]
async fn test_delete_with_hooks() {
    let _guard = TEST_SERIALIZER.lock().await;

    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);
    db.execute_unprepared(&stmt.to_string(sea_orm::sea_query::SqliteQueryBuilder))
        .await
        .unwrap();

    let user = User::objects(&db)
        .create(Model {
            id: 0,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            created_count: 0,
        })
        .await
        .unwrap();

    // Use OUR ORM .delete() - before_delete hook fires!
    user.delete(&db).await.unwrap();
}
