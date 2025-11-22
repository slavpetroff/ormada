//! Projection compile-time validation tests and edge cases

use super::common::test_helpers::*;
use seaorm_django::prelude::*;

mod product_model {
    use super::*;

    #[django_model(table = "products")]
    pub struct Product {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub price: i32,
        pub stock: i32,
        pub category: String,
        pub description: Option<String>,
        pub active: bool,
    }
}

#[django_projection(model = product_model::Product)]
struct ProductFull {
    id: i32,
    name: String,
    price: i32,
    stock: i32,
    category: String,
    description: Option<String>,
    active: bool,
}

#[django_projection(model = product_model::Product)]
struct ProductBasic {
    id: i32,
    name: String,
    price: i32,
}

#[django_projection(model = product_model::Product)]
struct ProductReordered {
    name: String,
    price: i32,
    id: i32,
}

#[django_projection(model = product_model::Product)]
struct ProductId {
    id: i32,
}

#[django_projection(model = product_model::Product)]
struct ProductWithOptional {
    id: i32,
    name: String,
    description: Option<String>,
}

#[django_projection(model = product_model::Product)]
struct ProductActive {
    id: i32,
    name: String,
    active: bool,
}

#[test]
fn test_all_projection_types_compile() {
    let _: Option<ProductFull> = None;
    let _: Option<ProductBasic> = None;
    let _: Option<ProductReordered> = None;
    let _: Option<ProductId> = None;
    let _: Option<ProductWithOptional> = None;
    let _: Option<ProductActive> = None;
}

#[tokio::test]
async fn test_projection_all_fields() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    product_model::Product::objects(&db)
        .create(product_model::Product {
            id: 0,
            name: "Test Product".into(),
            price: 1000,
            stock: 50,
            category: "Electronics".into(),
            description: Some("A test product".into()),
            active: true,
        })
        .await
        .unwrap();

    let full: Vec<ProductFull> =
        product_model::Product::objects(&db).project::<ProductFull>().await.unwrap();

    assert_eq!(full.len(), 1);
    assert_eq!(full[0].name, "Test Product");
    assert_eq!(full[0].price, 1000);
    assert_eq!(full[0].stock, 50);
    assert_eq!(full[0].active, true);
    assert!(full[0].description.is_some());
}

#[tokio::test]
async fn test_projection_field_order_independent() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    product_model::Product::objects(&db)
        .create(product_model::Product {
            id: 0,
            name: "Product A".into(),
            price: 500,
            stock: 10,
            category: "Books".into(),
            description: None,
            active: true,
        })
        .await
        .unwrap();

    let reordered: Vec<ProductReordered> = product_model::Product::objects(&db)
        .project::<ProductReordered>()
        .await
        .unwrap();

    assert_eq!(reordered.len(), 1);
    assert_eq!(reordered[0].name, "Product A");
    assert_eq!(reordered[0].price, 500);
}

#[tokio::test]
async fn test_projection_boolean_field() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    product_model::Product::objects(&db)
        .create(product_model::Product {
            id: 0,
            name: "Active Product".into(),
            price: 1000,
            stock: 10,
            category: "Test".into(),
            description: None,
            active: true,
        })
        .await
        .unwrap();

    product_model::Product::objects(&db)
        .create(product_model::Product {
            id: 0,
            name: "Inactive Product".into(),
            price: 500,
            stock: 5,
            category: "Test".into(),
            description: None,
            active: false,
        })
        .await
        .unwrap();

    let active_only: Vec<ProductActive> = product_model::Product::objects(&db)
        .filter(product_model::Product::Active.eq(true))
        .project::<ProductActive>()
        .await
        .unwrap();

    assert_eq!(active_only.len(), 1);
    assert_eq!(active_only[0].name, "Active Product");
    assert_eq!(active_only[0].active, true);
}

#[tokio::test]
async fn test_projection_with_null_handling() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    for i in 1..=10 {
        product_model::Product::objects(&db)
            .create(product_model::Product {
                id: 0,
                name: format!("Product {}", i),
                price: i * 100,
                stock: i,
                category: "Test".into(),
                description: if i % 3 == 0 { Some(format!("Desc {}", i)) } else { None },
                active: true,
            })
            .await
            .unwrap();
    }

    let with_optional: Vec<ProductWithOptional> = product_model::Product::objects(&db)
        .project::<ProductWithOptional>()
        .await
        .unwrap();

    assert_eq!(with_optional.len(), 10);

    let with_desc = with_optional.iter().filter(|p| p.description.is_some()).count();
    let without_desc = with_optional.iter().filter(|p| p.description.is_none()).count();

    assert_eq!(with_desc, 3);
    assert_eq!(without_desc, 7);
}

#[tokio::test]
async fn test_projection_large_dataset() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    for i in 1..=500 {
        product_model::Product::objects(&db)
            .create(product_model::Product {
                id: 0,
                name: format!("Product {:03}", i),
                price: i,
                stock: i % 100,
                category: format!("Cat{}", i % 10),
                description: Some(format!("Desc {}", i)),
                active: true,
            })
            .await
            .unwrap();
    }

    let ids: Vec<ProductId> =
        product_model::Product::objects(&db).project::<ProductId>().await.unwrap();

    assert_eq!(ids.len(), 500);
}

#[tokio::test]
async fn test_projection_with_distinct() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    for i in 1..=20 {
        product_model::Product::objects(&db)
            .create(product_model::Product {
                id: 0,
                name: format!("Product {}", i),
                price: (i % 5) * 100,
                stock: i,
                category: "Test".into(),
                description: None,
                active: true,
            })
            .await
            .unwrap();
    }

    let products: Vec<ProductBasic> = product_model::Product::objects(&db)
        .distinct()
        .project::<ProductBasic>()
        .await
        .unwrap();

    assert_eq!(products.len(), 20);
}

#[tokio::test]
async fn test_projection_with_complex_filters() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL,
            category TEXT NOT NULL,
            description TEXT,
            active INTEGER NOT NULL
        )",
    )
    .await;

    for i in 1..=50 {
        product_model::Product::objects(&db)
            .create(product_model::Product {
                id: 0,
                name: format!("Product {}", i),
                price: i * 100,
                stock: i,
                category: if i <= 25 { "A" } else { "B" }.into(),
                description: None,
                active: i % 2 == 0,
            })
            .await
            .unwrap();
    }

    let q = Q::all()
        .add(product_model::Product::Price.gt(1000))
        .add(product_model::Product::Stock.lt(30))
        .add(product_model::Product::Active.eq(true));

    let filtered: Vec<ProductBasic> = product_model::Product::objects(&db)
        .filter(q)
        .order_by_asc(product_model::Product::Price)
        .project::<ProductBasic>()
        .await
        .unwrap();

    for product in &filtered {
        assert!(product.price > 1000);
    }
}
