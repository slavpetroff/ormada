//! Comprehensive tests for different PK/FK types
//!
//! This module tests:
//! - Different PK types: i32, i64
//! - Different FK types matching their referenced PK types  
//! - FK validation for all supported integer types
//!
//! NOTE: String/UUID PKs with FK relations require additional macro work
//! to support non-integer types in `get_foreign_key` and `load_related`.
//! Currently, only integer PK/FK types are fully supported for relations.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::too_many_lines)]

use ormada::prelude::*;
use rstest::*;
use sea_orm::Database;

// ============================================================================
// Test Models with Different PK/FK Types
// ============================================================================

pub mod pk_types {
    use ormada::prelude::*;

    // Model with i32 PK (most common)
    pub mod category_i32 {
        use super::*;

        #[ormada_model(table = "categories_i32")]
        pub struct CategoryI32 {
            #[primary_key]
            pub id: i32,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with i64 PK (for large datasets)
    pub mod category_i64 {
        use super::*;

        #[ormada_model(table = "categories_i64")]
        pub struct CategoryI64 {
            #[primary_key]
            pub id: i64,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with FK to i32 PK
    pub mod item_i32 {
        use super::*;

        #[ormada_model(table = "items_i32")]
        pub struct ItemI32 {
            #[primary_key]
            pub id: i32,

            #[foreign_key(CategoryI32)]
            pub category_id: i32,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with FK to i64 PK
    pub mod item_i64 {
        use super::*;

        #[ormada_model(table = "items_i64")]
        pub struct ItemI64 {
            #[primary_key]
            pub id: i32,

            #[foreign_key(CategoryI64)]
            pub category_id: i64,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with nullable FK to i32 PK
    pub mod item_nullable_i32 {
        use super::*;

        #[ormada_model(table = "items_nullable_i32")]
        pub struct ItemNullableI32 {
            #[primary_key]
            pub id: i32,

            #[foreign_key(CategoryI32, on_delete = SetNull)]
            pub category_id: Option<i32>,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with nullable FK to i64 PK
    pub mod item_nullable_i64 {
        use super::*;

        #[ormada_model(table = "items_nullable_i64")]
        pub struct ItemNullableI64 {
            #[primary_key]
            pub id: i32,

            #[foreign_key(CategoryI64, on_delete = SetNull)]
            pub category_id: Option<i64>,

            #[max_length(100)]
            pub name: String,
        }
    }

    // ========================================================================
    // UUID PK/FK Models
    // ========================================================================

    // Model with UUID PK
    pub mod category_uuid {
        use super::*;

        #[ormada_model(table = "categories_uuid")]
        pub struct CategoryUuid {
            #[primary_key(auto_increment = false)]
            pub id: Uuid,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with FK to UUID PK
    pub mod item_uuid {
        use super::*;

        #[ormada_model(table = "items_uuid")]
        pub struct ItemUuid {
            #[primary_key]
            pub id: i32,

            #[foreign_key(CategoryUuid)]
            pub category_id: Uuid,

            #[max_length(100)]
            pub name: String,
        }
    }

    // Model with nullable FK to UUID PK
    pub mod item_nullable_uuid {
        use super::*;

        #[ormada_model(table = "items_nullable_uuid")]
        pub struct ItemNullableUuid {
            #[primary_key]
            pub id: i32,

            #[foreign_key(CategoryUuid, on_delete = SetNull)]
            pub category_id: Option<Uuid>,

            #[max_length(100)]
            pub name: String,
        }
    }

    // ========================================================================
    // Composite PK Models
    // ========================================================================

    // Junction table with composite PK (many-to-many relationship)
    pub mod order_item {
        use super::*;

        #[ormada_model(table = "order_items")]
        pub struct OrderItem {
            #[primary_key(auto_increment = false)]
            pub order_id: i32,

            #[primary_key(auto_increment = false)]
            pub item_id: i32,

            pub quantity: i32,
            pub price: i32,
        }
    }

    // Model referencing composite PK (for testing FK to composite)
    pub mod order {
        use super::*;

        #[ormada_model(table = "orders")]
        pub struct Order {
            #[primary_key]
            pub id: i32,

            #[max_length(100)]
            pub customer_name: String,

            pub total: i32,
        }
    }

    // Simple item for composite PK tests
    pub mod simple_item {
        use super::*;

        #[ormada_model(table = "simple_items")]
        pub struct SimpleItem {
            #[primary_key]
            pub id: i32,

            #[max_length(100)]
            pub name: String,

            pub price: i32,
        }
    }
}

// Re-exports
pub use pk_types::category_i32::CategoryI32;
pub use pk_types::category_i64::CategoryI64;
pub use pk_types::category_uuid::CategoryUuid;
pub use pk_types::item_i32::ItemI32;
pub use pk_types::item_i64::ItemI64;
pub use pk_types::item_nullable_i32::ItemNullableI32;
pub use pk_types::item_nullable_i64::ItemNullableI64;
pub use pk_types::item_nullable_uuid::ItemNullableUuid;
pub use pk_types::item_uuid::ItemUuid;
pub use pk_types::order::Order;
pub use pk_types::order_item::OrderItem;
pub use pk_types::simple_item::SimpleItem;

// ============================================================================
// Database Fixtures
// ============================================================================

#[fixture]
pub async fn db_empty() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");
    DatabaseRouter::new_single(db)
}

#[fixture]
#[awt]
pub async fn db_pk_types(#[future] db_empty: DatabaseRouter) -> DatabaseRouter {
    let db = db_empty;

    // Create tables for i32 PK/FK
    CategoryI32::create_table(&db)
        .await
        .expect("Failed to create categories_i32 table");
    ItemI32::create_table(&db).await.expect("Failed to create items_i32 table");
    ItemNullableI32::create_table(&db)
        .await
        .expect("Failed to create items_nullable_i32 table");

    // Create tables for i64 PK/FK
    CategoryI64::create_table(&db)
        .await
        .expect("Failed to create categories_i64 table");
    ItemI64::create_table(&db).await.expect("Failed to create items_i64 table");
    ItemNullableI64::create_table(&db)
        .await
        .expect("Failed to create items_nullable_i64 table");

    // Create tables for UUID PK/FK
    CategoryUuid::create_table(&db)
        .await
        .expect("Failed to create categories_uuid table");
    ItemUuid::create_table(&db).await.expect("Failed to create items_uuid table");
    ItemNullableUuid::create_table(&db)
        .await
        .expect("Failed to create items_nullable_uuid table");

    // Create tables for composite PK
    Order::create_table(&db).await.expect("Failed to create orders table");
    SimpleItem::create_table(&db)
        .await
        .expect("Failed to create simple_items table");
    OrderItem::create_table(&db).await.expect("Failed to create order_items table");

    db
}

// ============================================================================
// i32 PK/FK Tests (Happy Path)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_i32_pk_create_and_read(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category with i32 PK
    let category = CategoryI32::objects(&db)
        .create(CategoryI32 {
            name: "Electronics".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(category.id > 0);
    assert_eq!(category.name, "Electronics");

    // Read it back
    let fetched = CategoryI32::objects(&db).get(category.id).await.unwrap();

    assert_eq!(fetched.id, category.id);
    assert_eq!(fetched.name, "Electronics");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i32_fk_create_with_valid_reference(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category first
    let category = CategoryI32::objects(&db)
        .create(CategoryI32 {
            name: "Electronics".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create item with valid FK
    let item = ItemI32::objects(&db)
        .create(ItemI32 {
            category_id: category.id,
            name: "Laptop".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert_eq!(item.category_id, category.id);
    assert_eq!(item.name, "Laptop");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i32_fk_validation_rejects_default(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Try to create item with default FK (0)
    let result = ItemI32::objects(&db)
        .create(ItemI32 {
            name: "Orphan Item".to_string(),
            ..Default::default() // category_id will be 0
        })
        .await;

    assert!(result.is_err());
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(err_str.contains("foreign key cannot be the default value"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i32_nullable_fk_accepts_none(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create item with None FK (valid for nullable FK)
    let item = ItemNullableI32::objects(&db)
        .create(ItemNullableI32 {
            category_id: None,
            name: "Uncategorized Item".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert!(item.category_id.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i32_nullable_fk_accepts_some(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category first
    let category = CategoryI32::objects(&db)
        .create(CategoryI32 {
            name: "Electronics".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create item with Some FK
    let item = ItemNullableI32::objects(&db)
        .create(ItemNullableI32 {
            category_id: Some(category.id),
            name: "Laptop".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert_eq!(item.category_id, Some(category.id));
}

// ============================================================================
// i64 PK/FK Tests (Happy Path)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_i64_pk_create_and_read(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category with i64 PK
    let category = CategoryI64::objects(&db)
        .create(CategoryI64 {
            name: "Big Data Category".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(category.id > 0);
    assert_eq!(category.name, "Big Data Category");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i64_fk_create_with_valid_reference(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category first
    let category = CategoryI64::objects(&db)
        .create(CategoryI64 {
            name: "Big Data Category".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create item with valid FK
    let item = ItemI64::objects(&db)
        .create(ItemI64 {
            category_id: category.id,
            name: "Big Data Item".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert_eq!(item.category_id, category.id);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i64_fk_validation_rejects_default(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Try to create item with default FK (0)
    let result = ItemI64::objects(&db)
        .create(ItemI64 {
            name: "Orphan Item".to_string(),
            ..Default::default() // category_id will be 0
        })
        .await;

    assert!(result.is_err());
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(err_str.contains("foreign key cannot be the default value"));
}

// ============================================================================
// i64 Nullable FK Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_i64_nullable_fk_accepts_none(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create item with None FK (valid for nullable FK)
    let item = ItemNullableI64::objects(&db)
        .create(ItemNullableI64 {
            category_id: None,
            name: "Uncategorized Item".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert!(item.category_id.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_i64_nullable_fk_accepts_some(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category first
    let category = CategoryI64::objects(&db)
        .create(CategoryI64 {
            name: "Big Data Category".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create item with Some FK
    let item = ItemNullableI64::objects(&db)
        .create(ItemNullableI64 {
            category_id: Some(category.id),
            name: "Big Data Item".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert_eq!(item.category_id, Some(category.id));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_multiple_items_same_category_i32(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create category
    let category = CategoryI32::objects(&db)
        .create(CategoryI32 {
            name: "Electronics".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create multiple items in same category
    for name in ["Laptop", "Phone", "Tablet"] {
        ItemI32::objects(&db)
            .create(ItemI32 {
                category_id: category.id,
                name: name.to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Verify all items exist
    let items = ItemI32::objects(&db)
        .filter(ItemI32::CategoryId.eq(category.id))
        .all()
        .await
        .unwrap();

    assert_eq!(items.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_fk_to_different_category(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create two categories
    let cat1 = CategoryI32::objects(&db)
        .create(CategoryI32 {
            name: "Electronics".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    let cat2 = CategoryI32::objects(&db)
        .create(CategoryI32 {
            name: "Books".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create item in first category
    let item = ItemI32::objects(&db)
        .create(ItemI32 {
            category_id: cat1.id,
            name: "Item".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Update to second category
    ItemI32::objects(&db)
        .filter(ItemI32::Id.eq(item.id))
        .update(|mut item| async move {
            item.category_id = cat2.id;
            Ok(item)
        })
        .await
        .unwrap();

    // Verify update
    let updated = ItemI32::objects(&db).get(item.id).await.unwrap();

    assert_eq!(updated.category_id, cat2.id);
}

// ============================================================================
// Default Value Tests (Compile-time type safety)
// ============================================================================

#[test]
fn test_i32_default_is_zero() {
    let item: ItemI32 = Default::default();
    assert_eq!(item.category_id, 0);
}

#[test]
fn test_i64_default_is_zero() {
    let item: ItemI64 = Default::default();
    assert_eq!(item.category_id, 0i64);
}

#[test]
fn test_nullable_i32_default_is_none() {
    let item: ItemNullableI32 = Default::default();
    assert!(item.category_id.is_none());
}

#[test]
fn test_nullable_i64_default_is_none() {
    let item: ItemNullableI64 = Default::default();
    assert!(item.category_id.is_none());
}

// ============================================================================
// UUID PK/FK Tests (Happy Path)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_uuid_pk_create_and_read(#[future] db_pk_types: DatabaseRouter) {
    use uuid::Uuid;
    let db = db_pk_types;

    let uuid_id = Uuid::new_v4();

    // Create category with UUID PK
    let category = CategoryUuid::objects(&db)
        .create(CategoryUuid {
            id: uuid_id,
            name: "Electronics".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(category.id, uuid_id);
    assert_eq!(category.name, "Electronics");

    // Read it back by filtering on UUID
    let fetched = CategoryUuid::objects(&db)
        .filter(CategoryUuid::Id.eq(uuid_id))
        .first()
        .await
        .unwrap();

    assert_eq!(fetched.id, uuid_id);
    assert_eq!(fetched.name, "Electronics");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_uuid_fk_create_with_valid_reference(#[future] db_pk_types: DatabaseRouter) {
    use uuid::Uuid;
    let db = db_pk_types;

    let uuid_id = Uuid::new_v4();

    // Create category first
    let category = CategoryUuid::objects(&db)
        .create(CategoryUuid {
            id: uuid_id,
            name: "Electronics".to_string(),
        })
        .await
        .unwrap();

    // Create item with valid FK
    let item = ItemUuid::objects(&db)
        .create(ItemUuid {
            category_id: category.id,
            name: "Laptop".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert_eq!(item.category_id, uuid_id);
    assert_eq!(item.name, "Laptop");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_uuid_fk_validation_rejects_nil_default(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Try to create item with default FK (nil UUID)
    let result = ItemUuid::objects(&db)
        .create(ItemUuid {
            name: "Orphan Item".to_string(),
            ..Default::default() // category_id will be nil UUID
        })
        .await;

    assert!(result.is_err());
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(err_str.contains("foreign key cannot be the default value"));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_uuid_nullable_fk_accepts_none(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create item with None FK (valid for nullable FK)
    let item = ItemNullableUuid::objects(&db)
        .create(ItemNullableUuid {
            category_id: None,
            name: "Uncategorized Item".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert!(item.category_id.is_none());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_uuid_nullable_fk_accepts_some(#[future] db_pk_types: DatabaseRouter) {
    use uuid::Uuid;
    let db = db_pk_types;

    let uuid_id = Uuid::new_v4();

    // Create category first
    let category = CategoryUuid::objects(&db)
        .create(CategoryUuid {
            id: uuid_id,
            name: "Electronics".to_string(),
        })
        .await
        .unwrap();

    // Create item with Some FK
    let item = ItemNullableUuid::objects(&db)
        .create(ItemNullableUuid {
            category_id: Some(category.id),
            name: "Laptop".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(item.id > 0);
    assert_eq!(item.category_id, Some(uuid_id));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_uuid_multiple_items_same_category(#[future] db_pk_types: DatabaseRouter) {
    use uuid::Uuid;
    let db = db_pk_types;

    let uuid_id = Uuid::new_v4();

    // Create category
    let category = CategoryUuid::objects(&db)
        .create(CategoryUuid {
            id: uuid_id,
            name: "Electronics".to_string(),
        })
        .await
        .unwrap();

    // Create multiple items in same category
    for name in ["Laptop", "Phone", "Tablet"] {
        ItemUuid::objects(&db)
            .create(ItemUuid {
                category_id: category.id,
                name: name.to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    // Verify all items exist
    let items = ItemUuid::objects(&db)
        .filter(ItemUuid::CategoryId.eq(uuid_id))
        .all()
        .await
        .unwrap();

    assert_eq!(items.len(), 3);
}

#[test]
fn test_uuid_default_is_nil() {
    use uuid::Uuid;
    let item: ItemUuid = Default::default();
    assert_eq!(item.category_id, Uuid::nil());
}

#[test]
fn test_nullable_uuid_default_is_none() {
    let item: ItemNullableUuid = Default::default();
    assert!(item.category_id.is_none());
}

// ============================================================================
// Composite PK Tests (Happy Path)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_composite_pk_create_and_read(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create order and item first
    let order = Order::objects(&db)
        .create(Order {
            customer_name: "John Doe".to_string(),
            total: 10000,
            ..Default::default()
        })
        .await
        .unwrap();

    let item = SimpleItem::objects(&db)
        .create(SimpleItem {
            name: "Laptop".to_string(),
            price: 5000,
            ..Default::default()
        })
        .await
        .unwrap();

    // Create order_item with composite PK
    let order_item = OrderItem::objects(&db)
        .create(OrderItem {
            order_id: order.id,
            item_id: item.id,
            quantity: 2,
            price: 5000,
        })
        .await
        .unwrap();

    assert_eq!(order_item.order_id, order.id);
    assert_eq!(order_item.item_id, item.id);
    assert_eq!(order_item.quantity, 2);
    assert_eq!(order_item.price, 5000);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_composite_pk_multiple_items_per_order(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create order
    let order = Order::objects(&db)
        .create(Order {
            customer_name: "Jane Doe".to_string(),
            total: 15000,
            ..Default::default()
        })
        .await
        .unwrap();

    // Create multiple items
    let mut items = Vec::new();
    for (name, price) in [("Laptop", 5000), ("Mouse", 500), ("Keyboard", 1000)] {
        let item = SimpleItem::objects(&db)
            .create(SimpleItem {
                name: name.to_string(),
                price,
                ..Default::default()
            })
            .await
            .unwrap();
        items.push(item);
    }

    // Create order_items for each
    for (i, item) in items.iter().enumerate() {
        OrderItem::objects(&db)
            .create(OrderItem {
                order_id: order.id,
                item_id: item.id,
                quantity: (i + 1) as i32,
                price: item.price,
            })
            .await
            .unwrap();
    }

    // Verify all order_items exist
    let order_items = OrderItem::objects(&db)
        .filter(OrderItem::OrderId.eq(order.id))
        .all()
        .await
        .unwrap();

    assert_eq!(order_items.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_composite_pk_same_item_multiple_orders(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create item
    let item = SimpleItem::objects(&db)
        .create(SimpleItem {
            name: "Popular Item".to_string(),
            price: 1000,
            ..Default::default()
        })
        .await
        .unwrap();

    // Create multiple orders
    let mut orders = Vec::new();
    for name in ["Alice", "Bob", "Charlie"] {
        let order = Order::objects(&db)
            .create(Order {
                customer_name: name.to_string(),
                total: 1000,
                ..Default::default()
            })
            .await
            .unwrap();
        orders.push(order);
    }

    // Add same item to each order
    for order in &orders {
        OrderItem::objects(&db)
            .create(OrderItem {
                order_id: order.id,
                item_id: item.id,
                quantity: 1,
                price: item.price,
            })
            .await
            .unwrap();
    }

    // Verify all order_items exist for this item
    let order_items = OrderItem::objects(&db)
        .filter(OrderItem::ItemId.eq(item.id))
        .all()
        .await
        .unwrap();

    assert_eq!(order_items.len(), 3);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_composite_pk_update(#[future] db_pk_types: DatabaseRouter) {
    let db = db_pk_types;

    // Create order and item
    let order = Order::objects(&db)
        .create(Order {
            customer_name: "Test".to_string(),
            total: 1000,
            ..Default::default()
        })
        .await
        .unwrap();

    let item = SimpleItem::objects(&db)
        .create(SimpleItem {
            name: "Item".to_string(),
            price: 500,
            ..Default::default()
        })
        .await
        .unwrap();

    // Create order_item
    OrderItem::objects(&db)
        .create(OrderItem {
            order_id: order.id,
            item_id: item.id,
            quantity: 1,
            price: 500,
        })
        .await
        .unwrap();

    // Update quantity
    OrderItem::objects(&db)
        .filter(OrderItem::OrderId.eq(order.id))
        .filter(OrderItem::ItemId.eq(item.id))
        .update(|mut oi| async move {
            oi.quantity = 5;
            Ok(oi)
        })
        .await
        .unwrap();

    // Verify update
    let updated = OrderItem::objects(&db)
        .filter(OrderItem::OrderId.eq(order.id))
        .filter(OrderItem::ItemId.eq(item.id))
        .first()
        .await
        .unwrap();

    assert_eq!(updated.quantity, 5);
}

#[test]
fn test_composite_pk_default() {
    let order_item: OrderItem = Default::default();
    assert_eq!(order_item.order_id, 0);
    assert_eq!(order_item.item_id, 0);
    assert_eq!(order_item.quantity, 0);
    assert_eq!(order_item.price, 0);
}
