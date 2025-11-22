//! Actual projection usage tests - testing the projection types themselves

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
    impl AsyncLifecycleHooks for Model {}
}

// Projection with subset of fields
#[django_projection(model = user_model::User)]
struct UserBasic {
    id: i32,
    name: String,
}

// Projection with optional field
#[django_projection(model = user_model::User)]
struct UserWithBio {
    id: i32,
    name: String,
    bio: Option<String>,
}

// Single field projection
#[django_projection(model = user_model::User)]
struct UserId {
    id: i32,
}

#[tokio::test]
async fn test_basic_projection_returns_subset() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    // Insert test data
    user_model::User::objects(&db)
        .create(user_model::User {
            id: 0,
            name: "Alice".into(),
            email: "alice@test.com".into(),
            age: 30,
            bio: Some("Developer".into()),
        })
        .await
        .unwrap();

    // Use projection to get only id and name
    let users: Vec<UserBasic> =
        user_model::User::objects(&db).project::<UserBasic>().await.unwrap();

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    // UserBasic doesn't have email, age, or bio fields - that's the point!
}

#[tokio::test]
async fn test_projection_with_optional_field() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    // Insert with bio
    user_model::User::objects(&db)
        .create(user_model::User {
            id: 0,
            name: "Bob".into(),
            email: "bob@test.com".into(),
            age: 25,
            bio: Some("Engineer".into()),
        })
        .await
        .unwrap();

    // Insert without bio
    user_model::User::objects(&db)
        .create(user_model::User {
            id: 0,
            name: "Charlie".into(),
            email: "charlie@test.com".into(),
            age: 35,
            bio: None,
        })
        .await
        .unwrap();

    // Project to type with optional field
    let users: Vec<UserWithBio> =
        user_model::User::objects(&db).project::<UserWithBio>().await.unwrap();

    assert_eq!(users.len(), 2);
    assert_eq!(users[0].bio, Some("Engineer".into()));
    assert_eq!(users[1].bio, None);
}

#[tokio::test]
async fn test_projection_with_filters() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    // Insert multiple users
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

    // Filter and project
    let users: Vec<UserBasic> = user_model::User::objects(&db)
        .filter(user_model::User::Age.gte(25))
        .project::<UserBasic>()
        .await
        .unwrap();

    assert_eq!(users.len(), 6);
    // Verify we got projection type, not full model
    for user in &users {
        assert!(user.id > 0);
        assert!(user.name.starts_with("User"));
    }
}

#[tokio::test]
async fn test_projection_with_ordering() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    let names = vec!["Zoe", "Alice", "Mike"];
    for name in names {
        user_model::User::objects(&db)
            .create(user_model::User {
                id: 0,
                name: name.into(),
                email: format!("{}@test.com", name.to_lowercase()),
                age: 25,
                bio: None,
            })
            .await
            .unwrap();
    }

    // Order and project
    let users: Vec<UserBasic> = user_model::User::objects(&db)
        .order_by_asc(user_model::User::Name)
        .project::<UserBasic>()
        .await
        .unwrap();

    assert_eq!(users.len(), 3);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[1].name, "Mike");
    assert_eq!(users[2].name, "Zoe");
}

#[tokio::test]
async fn test_projection_with_limit() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    // Insert 20 users
    for i in 1..=20 {
        user_model::User::objects(&db)
            .create(user_model::User {
                id: 0,
                name: format!("User{:02}", i),
                email: format!("user{}@test.com", i),
                age: 25,
                bio: None,
            })
            .await
            .unwrap();
    }

    // Limit and project
    let users: Vec<UserId> =
        user_model::User::objects(&db).limit(5).project::<UserId>().await.unwrap();

    assert_eq!(users.len(), 5);
    // UserId only has id field
    for user in &users {
        assert!(user.id > 0);
    }
}

#[tokio::test]
async fn test_single_field_projection() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    for i in 1..=100 {
        user_model::User::objects(&db)
            .create(user_model::User {
                id: 0,
                name: format!("User{}", i),
                email: format!("user{}@test.com", i),
                age: 25,
                bio: None,
            })
            .await
            .unwrap();
    }

    // Project to just IDs - minimal data transfer
    let ids: Vec<UserId> = user_model::User::objects(&db).project::<UserId>().await.unwrap();

    assert_eq!(ids.len(), 100);
    assert_eq!(ids[0].id, 1);
    assert_eq!(ids[99].id, 100);
}

#[tokio::test]
async fn test_projection_empty_results() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    // No data - project returns empty vec
    let users: Vec<UserBasic> =
        user_model::User::objects(&db).project::<UserBasic>().await.unwrap();

    assert_eq!(users.len(), 0);
}

#[tokio::test]
async fn test_projection_with_complex_filters() {
    let db = setup_test_db().await;

    execute_sql(
        &db,
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL,
            bio TEXT
        )",
    )
    .await;

    // Insert various ages
    for i in 1..=50 {
        user_model::User::objects(&db)
            .create(user_model::User {
                id: 0,
                name: format!("User{}", i),
                email: format!("user{}@test.com", i),
                age: 15 + i,
                bio: if i % 3 == 0 { Some(format!("Bio {}", i)) } else { None },
            })
            .await
            .unwrap();
    }

    // Complex filter: age between 30-40 AND has bio
    let q = Q::all()
        .add(user_model::User::Age.gte(30))
        .add(user_model::User::Age.lte(40))
        .add(user_model::User::Bio.is_not_null());

    let users: Vec<UserWithBio> = user_model::User::objects(&db)
        .filter(q)
        .order_by_asc(user_model::User::Name)
        .project::<UserWithBio>()
        .await
        .unwrap();

    // Verify all match criteria
    for user in &users {
        assert!(user.bio.is_some());
    }
}
