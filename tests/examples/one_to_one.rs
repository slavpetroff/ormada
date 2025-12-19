//! One-to-One Relationship Examples
//!
//! Demonstrates 1:1 relationships using the `#[one_to_one]` decorator.
//! Common use cases: User-Profile, Order-Invoice, etc.

use ormada::prelude::*;

pub mod models {
    pub mod user {
        use ormada::prelude::*;

        #[ormada_model(table = "o2o_users")]
        pub struct User {
            #[primary_key]
            pub id: i32,
            pub username: String,
            pub email: String,
        }
    }

    pub mod profile {
        
        use ormada::prelude::*;

        #[ormada_model(table = "o2o_profiles")]
        pub struct Profile {
            #[primary_key]
            pub id: i32,
            #[one_to_one(User)]
            pub user_id: i32,
            pub bio: String,
            pub avatar_url: String,
        }
    }
}

pub use models::profile::Profile;
pub use models::user::User;

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    User::create_table(&router).await?;
    Profile::create_table(&router).await?;
    Ok(router)
}

/// Create a User and their Profile (1:1 relationship)
pub async fn example_create_one_to_one(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let user = User::objects(db)
        .create(User {
            username: "alice".into(),
            email: "alice@example.com".into(),
            ..Default::default()
        })
        .await?;

    let profile = Profile::objects(db)
        .create(Profile {
            user_id: user.id,
            bio: "Software developer".into(),
            avatar_url: "https://example.com/alice.jpg".into(),
            ..Default::default()
        })
        .await?;

    assert_eq!(profile.user_id, user.id);

    Ok(())
}

/// Query Profile by User (1:1 lookup)
pub async fn example_query_profile_by_user(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let user = User::objects(db)
        .create(User {
            username: "bob".into(),
            email: "bob@example.com".into(),
            ..Default::default()
        })
        .await?;

    Profile::objects(db)
        .create(Profile {
            user_id: user.id,
            bio: "Designer".into(),
            avatar_url: "https://example.com/bob.jpg".into(),
            ..Default::default()
        })
        .await?;

    // Get profile for a specific user (1:1 means exactly one profile per user)
    let profile = Profile::objects(db).filter(Profile::UserId.eq(user.id)).first().await?;

    assert_eq!(profile.user_id, user.id);

    Ok(())
}

/// Demonstrate uniqueness of 1:1 relationship
pub async fn example_one_to_one_uniqueness(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let user = User::objects(db)
        .create(User {
            username: "charlie".into(),
            email: "charlie@example.com".into(),
            ..Default::default()
        })
        .await?;

    Profile::objects(db)
        .create(Profile {
            user_id: user.id,
            bio: "Engineer".into(),
            avatar_url: "https://example.com/charlie.jpg".into(),
            ..Default::default()
        })
        .await?;

    // In a real app, you'd have a UNIQUE constraint on user_id
    // to enforce 1:1 at the database level
    let count = Profile::objects(db).filter(Profile::UserId.eq(user.id)).count().await?;

    assert_eq!(count, 1);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_one_to_one() {
        let db = setup_db().await.unwrap();
        example_create_one_to_one(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_query_profile_by_user() {
        let db = setup_db().await.unwrap();
        example_query_profile_by_user(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_one_to_one_uniqueness() {
        let db = setup_db().await.unwrap();
        example_one_to_one_uniqueness(&db).await.unwrap();
    }
}
