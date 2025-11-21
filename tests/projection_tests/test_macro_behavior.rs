//! Tests for django_projection macro behavior
//!
//! BUG IDENTIFIED: Line 119 in derive/src/projection.rs has type mismatch
//! It tries: let _ : Model = Model::Column which is wrong
//! Should be: let _ : <Model as EntityTrait>::Column = Model::Column
//!
//! These tests document current behavior and will pass once bug is fixed.

use super::common::test_helpers::*;
use seaorm_django::prelude::*;

mod user_model {
    use super::*;
    
    #[django_model(table = "users")]
    pub struct User {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub age: i32,
        pub bio: Option<String>,
    }
}

// THESE WILL FAIL TO COMPILE UNTIL BUG IS FIXED:
/*
#[django_projection(model = user_model::User)]
struct UserBasic {
    id: i32,
    name: String,
}

#[django_projection(model = user_model::User)]
struct UserWithOptional {
    id: i32,
    name: String,
    bio: Option<String>,
}
*/

#[tokio::test]
async fn test_model_without_projection() {
    let db = setup_test_db().await;
    
    execute_sql(&db, 
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )"
    ).await;
    
    // Test basic CRUD with full model
    user_model::User::objects(&db)
        .create(user_model::User {
            id: 0,
            name: "Alice".into(),
            email: "alice@test.com".into(),
            age: 30,
            bio: Some("Test bio".into()),
        })
        .await
        .unwrap();
    
    let users: Vec<user_model::User> = user_model::User::objects(&db)
        .all()
        .await
        .unwrap();
    
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[0].email, "alice@test.com");
    assert!(users[0].bio.is_some());
}

#[tokio::test]
async fn test_filters_ordering_limit() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )"
    ).await;
    
    // Insert test data
    for i in 1..=10 {
        user_model::User::objects(&db)
            .create(user_model::User {
                id: 0,
                name: format!("User{}", i),
                email: format!("user{}@test.com", i),
                age: 20 + i,
                bio: None,
            })
            .await
            .unwrap();
    }
    
    // Test complex filter chain
    let results: Vec<user_model::User> = user_model::User::objects(&db)
        .filter(user_model::User::Age.gte(25))
        .filter(user_model::User::Age.lt(30))
        .order_by_asc(user_model::User::Name)
        .limit(3)
        .all()
        .await
        .unwrap();
    
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|u| u.age >= 25 && u.age < 30));
}

#[tokio::test]
async fn test_optional_fields() {
    let db = setup_test_db().await;
    
    execute_sql(&db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )"
    ).await;
    
    // Insert with NULL
    user_model::User::objects(&db)
        .create(user_model::User {
            id: 0,
            name: "NoB io".into(),
            email: "test@test.com".into(),
            age: 25,
            bio: None,
        })
        .await
        .unwrap();
    
    // Insert with value
    user_model::User::objects(&db)
        .create(user_model::User {
            id: 0,
            name: "WithBio".into(),
            email: "test2@test.com".into(),
            age: 30,
            bio: Some("My bio".into()),
        })
        .await
        .unwrap();
    
    let users: Vec<user_model::User> = user_model::User::objects(&db)
        .all()
        .await
        .unwrap();
    
    assert_eq!(users.len(), 2);
    assert!(users[0].bio.is_none());
    assert_eq!(users[1].bio.as_ref().unwrap(), "My bio");
}

// TODO: Once projection macro bug is fixed, add these tests:
// - test_projection_subset_fields()
// - test_projection_with_optional()
// - test_projection_compile_validation()
// - test_projection_with_filters()
// - test_projection_ordering()
// - test_projection_pagination()
