use super::common::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_aggregate_min_on_empty_table() {
    let db = setup_test_db().await;

    // Query empty table
    let result = author::Entity::objects(&db)
        .filter(author::Column::Age.gt(999)) // No matches
        .aggregate_min(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, None, "Min on empty result should be None");
}

#[tokio::test]
async fn test_aggregate_max_on_empty_table() {
    let db = setup_test_db().await;

    // Query empty table
    let result = author::Entity::objects(&db)
        .filter(author::Column::Age.gt(999)) // No matches
        .aggregate_max(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, None, "Max on empty result should be None");
}

#[tokio::test]
async fn test_aggregate_sum_on_empty_table() {
    let db = setup_test_db().await;

    // Query empty table
    let result = author::Entity::objects(&db)
        .filter(author::Column::Age.gt(999)) // No matches
        .aggregate_sum(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, None, "Sum on empty result should be None");
}

#[tokio::test]
async fn test_aggregate_avg_on_empty_table() {
    let db = setup_test_db().await;

    // Query empty table
    let result = author::Entity::objects(&db)
        .filter(author::Column::Age.gt(999)) // No matches
        .aggregate_avg(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, None, "Avg on empty result should be None");
}

#[tokio::test]
async fn test_aggregate_min_with_data() {
    let db = setup_test_db().await;

    // Create some authors
    for age in [25, 30, 35, 40] {
        author::Entity::objects(&db)
            .create(author::Model {
                name: format!("Author {}", age),
                email: format!("author{}@example.com", age),
                age,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let result = author::Entity::objects(&db)
        .aggregate_min(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, Some(25.0));
}

#[tokio::test]
async fn test_aggregate_max_with_data() {
    let db = setup_test_db().await;

    // Create some authors
    for age in [25, 30, 35, 40] {
        author::Entity::objects(&db)
            .create(author::Model {
                name: format!("Author {}", age),
                email: format!("author{}@example.com", age),
                age,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let result = author::Entity::objects(&db)
        .aggregate_max(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, Some(40.0));
}

#[tokio::test]
async fn test_aggregate_sum_with_data() {
    let db = setup_test_db().await;

    // Create some authors
    for age in [10, 20, 30] {
        author::Entity::objects(&db)
            .create(author::Model {
                name: format!("Author {}", age),
                email: format!("author{}@example.com", age),
                age,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let result = author::Entity::objects(&db)
        .aggregate_sum(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, Some(60.0));
}

#[tokio::test]
async fn test_aggregate_avg_with_data() {
    let db = setup_test_db().await;

    // Create some authors
    for age in [20, 30, 40] {
        author::Entity::objects(&db)
            .create(author::Model {
                name: format!("Author {}", age),
                email: format!("author{}@example.com", age),
                age,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let result = author::Entity::objects(&db)
        .aggregate_avg(author::Column::Age)
        .await
        .unwrap();

    assert_eq!(result, Some(30.0));
}
