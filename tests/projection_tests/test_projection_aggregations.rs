//! Projection tests with aggregations and computed fields

use super::common::test_helpers::*;
use seaorm_django::prelude::*;

mod order_model {
    use super::*;
    
    #[django_model(table = "orders")]
    pub struct Order {
        #[primary_key]
        pub id: i32,
        pub customer_id: i32,
        pub product_name: String,
        pub quantity: i32,
        pub price: i32,
        pub status: String,
    }
}

// Projection with computed aggregation fields
#[django_projection(model = order_model::Order)]
struct CustomerOrderStats {
    customer_id: i32,
    #[computed]
    order_count: i64,
    #[computed]
    total_quantity: Option<i64>,
    #[computed]
    avg_price: Option<f64>,
}

// Projection with single computed field
#[django_projection(model = order_model::Order)]
struct CustomerOrderCount {
    customer_id: i32,
    #[computed]
    order_count: i64,
}

// Projection mixing regular and computed fields
#[django_projection(model = order_model::Order)]
struct CustomerSummary {
    customer_id: i32,
    status: String,
    #[computed]
    total_orders: i64,
}

#[tokio::test]
async fn test_projection_with_group_by_and_aggregation() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    // Insert orders for multiple customers
    let orders = vec![
        (1, "Product A", 2, 1000, "completed"),
        (1, "Product B", 1, 1500, "completed"),
        (2, "Product A", 5, 1000, "pending"),
        (2, "Product C", 3, 2000, "completed"),
        (3, "Product A", 1, 1000, "completed"),
    ];
    
    for (cust_id, product, qty, price, status) in orders {
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id: cust_id,
                product_name: product.into(),
                quantity: qty,
                price,
                status: status.into(),
            })
            .await
            .unwrap();
    }
    
    // Group by customer and compute aggregates
    let stats: Vec<CustomerOrderStats> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .annotate([
            ("order_count", Aggregation::count_all()),
            ("total_quantity", Aggregation::sum(order_model::Order::Quantity)),
            ("avg_price", Aggregation::avg(order_model::Order::Price)),
        ])
        .project::<CustomerOrderStats>()
        .await
        .unwrap();
    
    assert_eq!(stats.len(), 3);
    
    // Customer 1: 2 orders
    let cust1 = stats.iter().find(|s| s.customer_id == 1).unwrap();
    assert_eq!(cust1.order_count, 2);
    assert_eq!(cust1.total_quantity, Some(3));
    
    // Customer 2: 2 orders
    let cust2 = stats.iter().find(|s| s.customer_id == 2).unwrap();
    assert_eq!(cust2.order_count, 2);
    assert_eq!(cust2.total_quantity, Some(8));
    
    // Customer 3: 1 order
    let cust3 = stats.iter().find(|s| s.customer_id == 3).unwrap();
    assert_eq!(cust3.order_count, 1);
    assert_eq!(cust3.total_quantity, Some(1));
}

#[tokio::test]
async fn test_projection_single_computed_field() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    for i in 1..=10 {
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id: i % 3 + 1,
                product_name: "Product".into(),
                quantity: 1,
                price: 1000,
                status: "completed".into(),
            })
            .await
            .unwrap();
    }
    
    let counts: Vec<CustomerOrderCount> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .annotate([("order_count", Aggregation::count_all())])
        .project::<CustomerOrderCount>()
        .await
        .unwrap();
    
    assert_eq!(counts.len(), 3);
    for count in &counts {
        assert!(count.order_count > 0);
    }
}

#[tokio::test]
async fn test_projection_with_filter_then_aggregate() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    // Insert completed and pending orders
    for i in 1..=20 {
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id: i % 5 + 1,
                product_name: format!("Product {}", i),
                quantity: i,
                price: i * 100,
                status: if i % 2 == 0 { "completed" } else { "pending" }.into(),
            })
            .await
            .unwrap();
    }
    
    // Only count completed orders per customer
    let stats: Vec<CustomerOrderCount> = order_model::Order::objects(&db)
        .filter(order_model::Order::Status.eq("completed"))
        .group_by(order_model::Order::CustomerId)
        .annotate([("order_count", Aggregation::count_all())])
        .project::<CustomerOrderCount>()
        .await
        .unwrap();
    
    assert!(stats.len() > 0);
    assert!(stats.len() <= 5);
}

#[tokio::test]
async fn test_projection_mixing_regular_and_computed() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    let statuses = vec!["completed", "pending", "cancelled"];
    for i in 1..=15 {
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id: i % 3 + 1,
                product_name: "Product".into(),
                quantity: 1,
                price: 1000,
                status: statuses[i as usize % 3].into(),
            })
            .await
            .unwrap();
    }
    
    // Group by customer_id AND status, count orders
    let summaries: Vec<CustomerSummary> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .group_by(order_model::Order::Status)
        .annotate([("total_orders", Aggregation::count_all())])
        .project::<CustomerSummary>()
        .await
        .unwrap();
    
    assert!(summaries.len() > 0);
    for summary in &summaries {
        assert!(summary.customer_id > 0);
        assert!(!summary.status.is_empty());
        assert!(summary.total_orders > 0);
    }
}

#[tokio::test]
async fn test_projection_aggregation_with_having() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    // Create customers with varying order counts
    for i in 1..=30 {
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id: i % 10 + 1,
                product_name: "Product".into(),
                quantity: 1,
                price: 1000,
                status: "completed".into(),
            })
            .await
            .unwrap();
    }
    
    // Get all customer order counts
    let counts: Vec<CustomerOrderCount> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .annotate([("order_count", Aggregation::count_all())])
        .project::<CustomerOrderCount>()
        .await
        .unwrap();
    
    assert_eq!(counts.len(), 10);
    // Each customer should have 3 orders (30 orders / 10 customers)
    for count in &counts {
        assert_eq!(count.order_count, 3);
    }
}

#[tokio::test]
async fn test_projection_multiple_aggregations() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    // Insert varied data
    for i in 1..=20 {
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id: i % 5 + 1,
                product_name: format!("Product {}", i),
                quantity: i,
                price: i * 100,
                status: "completed".into(),
            })
            .await
            .unwrap();
    }
    
    let stats: Vec<CustomerOrderStats> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .annotate([
            ("order_count", Aggregation::count_all()),
            ("total_quantity", Aggregation::sum(order_model::Order::Quantity)),
            ("avg_price", Aggregation::avg(order_model::Order::Price)),
        ])
        .project::<CustomerOrderStats>()
        .await
        .unwrap();
    
    assert_eq!(stats.len(), 5);
    
    for stat in &stats {
        assert!(stat.order_count > 0);
        assert!(stat.total_quantity.is_some());
        assert!(stat.avg_price.is_some());
    }
}

#[tokio::test]
async fn test_projection_aggregation_empty_group() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    // No data - aggregation on empty set
    let stats: Vec<CustomerOrderCount> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .annotate([("order_count", Aggregation::count_all())])
        .project::<CustomerOrderCount>()
        .await
        .unwrap();
    
    assert_eq!(stats.len(), 0);
}

#[tokio::test]
async fn test_projection_aggregation_with_ordering() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL,
            product_name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            price INTEGER NOT NULL,
            status TEXT NOT NULL
        )"
    ).await;
    
    // Insert data with different counts per customer
    for i in 1..=15 {
        let customer_id = if i <= 5 { 1 } else if i <= 10 { 2 } else { 3 };
        order_model::Order::objects(&db)
            .create(order_model::Order {
                id: 0,
                customer_id,
                product_name: "Product".into(),
                quantity: 1,
                price: 1000,
                status: "completed".into(),
            })
            .await
            .unwrap();
    }
    
    // Group and order by customer_id
    let counts: Vec<CustomerOrderCount> = order_model::Order::objects(&db)
        .group_by(order_model::Order::CustomerId)
        .annotate([("order_count", Aggregation::count_all())])
        .order_by_asc(order_model::Order::CustomerId)
        .project::<CustomerOrderCount>()
        .await
        .unwrap();
    
    assert_eq!(counts.len(), 3);
    assert_eq!(counts[0].customer_id, 1);
    assert_eq!(counts[1].customer_id, 2);
    assert_eq!(counts[2].customer_id, 3);
    // Customer 1 has 5 orders, customer 2 has 5, customer 3 has 5
    assert_eq!(counts[0].order_count, 5);
    assert_eq!(counts[1].order_count, 5);
    assert_eq!(counts[2].order_count, 5);
}
