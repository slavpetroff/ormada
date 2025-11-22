use sea_orm::{Database, DatabaseConnection};
use seaorm_django::prelude::*;

#[django_model(table = "items")]
pub struct Item {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub active: bool,
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

impl AsyncLifecycleHooks for Model {}

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    let schema = r#"
        CREATE TABLE items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            active INTEGER NOT NULL,
            deleted_at TEXT
        );
    "#;

    use sea_orm::ConnectionTrait;
    db.execute_unprepared(schema).await.expect("Failed to create table");

    db
}

async fn create_test_items(db: &DatabaseConnection) -> Vec<Model> {
    let mut items = vec![];

    for i in 1..=6 {
        let item = Entity::objects(db)
            .create(Model {
                id: 0,
                name: format!("Item {}", i),
                active: i % 2 == 0, // Even items are active
                deleted_at: None,
            })
            .await
            .unwrap();
        items.push(item);
    }

    items
}

#[tokio::test]
async fn test_with_deleted_includes_all() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let items = create_test_items(db).await;

    // Delete half of them
    items[0].clone().delete(db).await.unwrap();
    items[2].clone().delete(db).await.unwrap();
    items[4].clone().delete(db).await.unwrap();

    // Normal query excludes deleted
    let visible = Entity::objects(db).all().await.unwrap();
    assert_eq!(visible.len(), 3);

    // with_deleted includes all
    let all = Entity::objects(db).with_deleted().all().await.unwrap();
    assert_eq!(all.len(), 6);
}

#[tokio::test]
async fn test_only_deleted_shows_deleted_only() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let items = create_test_items(db).await;

    // Delete some items
    items[1].clone().delete(db).await.unwrap();
    items[3].clone().delete(db).await.unwrap();

    // only_deleted should show only the 2 deleted
    let deleted = Entity::objects(db).only_deleted().all().await.unwrap();
    assert_eq!(deleted.len(), 2);

    // Verify they are the right ones
    let deleted_ids: Vec<i32> = deleted.iter().map(|i| i.id).collect();
    assert!(deleted_ids.contains(&items[1].id));
    assert!(deleted_ids.contains(&items[3].id));
}

#[tokio::test]
async fn test_filter_with_soft_delete() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let items = create_test_items(db).await;

    // Delete some active items
    items[1].clone().delete(db).await.unwrap(); // Active item
    items[3].clone().delete(db).await.unwrap(); // Active item

    // Filter for active items (should exclude deleted)
    let active = Entity::objects(db).filter(Column::Active.eq(true)).all().await.unwrap();

    assert_eq!(active.len(), 1); // Only item 6 is active and not deleted

    // with_deleted should show all active (including deleted)
    let all_active = Entity::objects(db)
        .with_deleted()
        .filter(Column::Active.eq(true))
        .all()
        .await
        .unwrap();

    assert_eq!(all_active.len(), 3); // Items 2, 4, 6
}

#[tokio::test]
async fn test_only_deleted_with_filter() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let items = create_test_items(db).await;

    // Delete various items
    items[0].clone().delete(db).await.unwrap(); // Inactive
    items[1].clone().delete(db).await.unwrap(); // Active
    items[2].clone().delete(db).await.unwrap(); // Inactive
    items[3].clone().delete(db).await.unwrap(); // Active

    // Get only deleted active items
    let deleted_active = Entity::objects(db)
        .only_deleted()
        .filter(Column::Active.eq(true))
        .all()
        .await
        .unwrap();

    assert_eq!(deleted_active.len(), 2); // Items 2 and 4
}

#[tokio::test]
async fn test_count_respects_soft_delete() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let items = create_test_items(db).await;

    // Initial count
    let initial_count = Entity::objects(db).count().await.unwrap();
    assert_eq!(initial_count, 6);

    // Delete some
    items[0].clone().delete(db).await.unwrap();
    items[2].clone().delete(db).await.unwrap();

    // Count should exclude deleted
    let count = Entity::objects(db).count().await.unwrap();
    assert_eq!(count, 4);

    // with_deleted count should include all
    let all_count = Entity::objects(db).with_deleted().count().await.unwrap();
    assert_eq!(all_count, 6);

    // only_deleted count
    let deleted_count = Entity::objects(db).only_deleted().count().await.unwrap();
    assert_eq!(deleted_count, 2);
}

#[tokio::test]
async fn test_exists_respects_soft_delete() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let items = create_test_items(db).await;
    let item_id = items[0].id;

    // Should exist initially
    let exists = Entity::objects(db).filter(Column::Id.eq(item_id)).exists().await.unwrap();
    assert!(exists);

    // Soft delete it
    items[0].clone().delete(db).await.unwrap();

    // Should not exist in normal query
    let exists_after = Entity::objects(db).filter(Column::Id.eq(item_id)).exists().await.unwrap();
    assert!(!exists_after);

    // But should exist with_deleted
    let exists_with_deleted = Entity::objects(db)
        .with_deleted()
        .filter(Column::Id.eq(item_id))
        .exists()
        .await
        .unwrap();
    assert!(exists_with_deleted);
}
