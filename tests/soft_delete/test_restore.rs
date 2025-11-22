use sea_orm::{Database, DatabaseConnection};
use seaorm_django::prelude::*;

#[django_model(table = "documents")]
pub struct Document {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub content: String,
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

impl AsyncLifecycleHooks for Model {}

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    let schema = r#"
        CREATE TABLE documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            deleted_at TEXT
        );
    "#;

    use sea_orm::ConnectionTrait;
    db.execute_unprepared(schema).await.expect("Failed to create table");

    db
}

#[tokio::test]
async fn test_restore_basic() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create and soft delete a document
    let doc = Entity::objects(db)
        .create(Model {
            id: 0,
            title: "Test Doc".to_string(),
            content: "Content".to_string(),
            deleted_at: None,
        })
        .await
        .unwrap();

    let doc_id = doc.id;
    let deleted = doc.delete(db).await.unwrap();

    // Verify it's deleted
    assert!(deleted.deleted_at.is_some());
    let visible = Entity::objects(db).all().await.unwrap();
    assert_eq!(visible.len(), 0);

    // Restore it
    let restored = deleted.restore(db).await.unwrap();

    // Verify it's restored
    assert!(restored.deleted_at.is_none());
    assert_eq!(restored.title, "Test Doc");

    // Should be visible again
    let visible_after = Entity::objects(db).all().await.unwrap();
    assert_eq!(visible_after.len(), 1);
    assert_eq!(visible_after[0].id, doc_id);
}

#[tokio::test]
async fn test_restore_preserves_data() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let original = Entity::objects(db)
        .create(Model {
            id: 0,
            title: "Important Doc".to_string(),
            content: "Critical content here".to_string(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Delete and restore
    let deleted = original.delete(db).await.unwrap();
    let restored = deleted.restore(db).await.unwrap();

    // All data should be preserved
    assert_eq!(restored.title, "Important Doc");
    assert_eq!(restored.content, "Critical content here");
    assert!(restored.deleted_at.is_none());
}

#[tokio::test]
async fn test_multiple_delete_restore_cycles() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let doc = Entity::objects(db)
        .create(Model {
            id: 0,
            title: "Cycle Test".to_string(),
            content: "Content".to_string(),
            deleted_at: None,
        })
        .await
        .unwrap();

    // Delete -> Restore -> Delete -> Restore
    let mut current = doc;

    for _ in 0..3 {
        // Delete
        current = current.delete(db).await.unwrap();
        assert!(current.deleted_at.is_some());

        let count_deleted = Entity::objects(db).count().await.unwrap();
        assert_eq!(count_deleted, 0);

        // Restore
        current = current.restore(db).await.unwrap();
        assert!(current.deleted_at.is_none());

        let count_restored = Entity::objects(db).count().await.unwrap();
        assert_eq!(count_restored, 1);
    }
}

#[tokio::test]
async fn test_restore_from_only_deleted_query() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create and delete multiple documents
    for i in 1..=3 {
        let doc = Entity::objects(db)
            .create(Model {
                id: 0,
                title: format!("Doc {}", i),
                content: "Content".to_string(),
                deleted_at: None,
            })
            .await
            .unwrap();

        doc.delete(db).await.unwrap();
    }

    // Get all deleted
    let deleted_docs = Entity::objects(db).only_deleted().all().await.unwrap();
    assert_eq!(deleted_docs.len(), 3);

    // Restore one
    let to_restore = deleted_docs[1].clone();
    to_restore.restore(db).await.unwrap();

    // Now there should be 1 visible and 2 deleted
    let visible = Entity::objects(db).all().await.unwrap();
    assert_eq!(visible.len(), 1);

    let still_deleted = Entity::objects(db).only_deleted().all().await.unwrap();
    assert_eq!(still_deleted.len(), 2);
}

#[tokio::test]
async fn test_cannot_restore_force_deleted() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let doc = Entity::objects(db)
        .create(Model {
            id: 0,
            title: "To be removed".to_string(),
            content: "Content".to_string(),
            deleted_at: None,
        })
        .await
        .unwrap();

    let doc_id = doc.id;

    // Force delete (permanent)
    doc.force_delete(db).await.unwrap();

    // Try to find it even with with_deleted
    let result = Entity::objects(db)
        .with_deleted()
        .filter(Column::Id.eq(doc_id))
        .all()
        .await
        .unwrap();

    assert_eq!(result.len(), 0); // Gone forever
}

#[tokio::test]
async fn test_restore_makes_record_queryable() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let doc = Entity::objects(db)
        .create(Model {
            id: 0,
            title: "Queryable Test".to_string(),
            content: "Content".to_string(),
            deleted_at: None,
        })
        .await
        .unwrap();

    let doc_id = doc.id;

    // Delete it
    let deleted = doc.delete(db).await.unwrap();

    // Can't query with normal filter
    let result = Entity::objects(db)
        .filter(Column::Title.eq("Queryable Test"))
        .all()
        .await
        .unwrap();
    assert_eq!(result.len(), 0);

    // Restore
    deleted.restore(db).await.unwrap();

    // Now it should be queryable
    let after_restore = Entity::objects(db)
        .filter(Column::Title.eq("Queryable Test"))
        .all()
        .await
        .unwrap();
    assert_eq!(after_restore.len(), 1);
    assert_eq!(after_restore[0].id, doc_id);
}
