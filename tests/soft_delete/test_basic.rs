use chrono::Utc;
use sea_orm::{Database, DatabaseConnection};
use seaorm_django::prelude::*;

#[django_model(table = "products")]
pub struct Product {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub price: i32,
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

impl AsyncLifecycleHooks for Model {}

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    // Create table
    let schema = r#"
        CREATE TABLE products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            deleted_at TEXT
        );
    "#;

    use sea_orm::ConnectionTrait;
    db.execute_unprepared(schema).await.expect("Failed to create table");

    db
}

async fn create_test_products(db: &DatabaseConnection) -> Vec<Model> {
    let mut products = vec![];

    for i in 1..=5 {
        let product = Entity::objects(db)
            .create(Model {
                id: 0,
                name: format!("Product {}", i),
                price: i * 100,
                deleted_at: None,
            })
            .await
            .unwrap();
        products.push(product);
    }

    products
}

#[tokio::test]
async fn test_soft_delete_basic() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let products = create_test_products(db).await;

    // All products should be visible
    let all = Entity::objects(db).all().await.unwrap();
    assert_eq!(all.len(), 5);

    // Soft delete one product
    let deleted = products[0].clone().delete(db).await.unwrap();

    // Deleted product should have deleted_at set
    assert!(deleted.deleted_at.is_some(), "deleted_at should be set");

    // Now only 4 should be visible
    let visible = Entity::objects(db).all().await.unwrap();
    assert!(visible.len() == 4, "4 products should be visible");
    assert!(deleted.deleted_at.is_some(), "deleted_at should be set");
}

#[tokio::test]
async fn test_soft_delete_excludes_from_queries() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let products = create_test_products(db).await;

    // Delete multiple products
    products[0].clone().delete(db).await.unwrap();
    products[2].clone().delete(db).await.unwrap();

    // Queries should exclude deleted
    let visible = Entity::objects(db).all().await.unwrap();
    assert_eq!(visible.len(), 3);

    // Count should exclude deleted
    let count = Entity::objects(db).count().await.unwrap();
    assert_eq!(count, 3);

    // Filter should exclude deleted
    let filtered = Entity::objects(db).filter(Column::Price.gte(200)).all().await.unwrap();
    assert_eq!(filtered.len(), 3); // Products 2, 4, 5 (1 and 3 are deleted, 2 has price 200)
}

#[tokio::test]
async fn test_force_delete_permanently_removes() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let products = create_test_products(db).await;

    // Soft delete first
    let product = products[0].clone();
    let soft_deleted = product.delete(db).await.unwrap();

    // Verify it's soft deleted
    let visible = Entity::objects(db).all().await.unwrap();
    assert_eq!(visible.len(), 4);

    // Now force delete (permanent)
    soft_deleted.force_delete(db).await.unwrap();

    // Even with_deleted shouldn't find it
    let all_including_deleted = Entity::objects(db).with_deleted().all().await.unwrap();
    assert_eq!(all_including_deleted.len(), 4); // Permanently gone
}

#[tokio::test]
async fn test_soft_delete_preserves_data() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let products = create_test_products(db).await;
    let original = products[0].clone();
    let original_id = original.id;
    let original_name = original.name.clone();
    let original_price = original.price;

    // Soft delete
    let deleted = original.delete(db).await.unwrap();

    // Data should be preserved except deleted_at
    assert_eq!(deleted.id, original_id);
    assert_eq!(deleted.name, original_name);
    assert_eq!(deleted.price, original_price);
    assert!(deleted.deleted_at.is_some());
}

#[tokio::test]
async fn test_get_excludes_soft_deleted() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let products = create_test_products(db).await;
    let product_id = products[0].id;

    // Should be able to get it
    let found = Entity::objects(db).get(product_id).await.unwrap();
    assert_eq!(found.id, product_id);

    // Soft delete it
    products[0].clone().delete(db).await.unwrap();

    // Now get should fail (not found)
    let result = Entity::objects(db).get(product_id).await;
    assert!(result.is_err());
}
