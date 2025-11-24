// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

use seaorm_django::prelude::*;

mod common;
use common::test_helpers::setup_test_db;

mod test_create_table_mod {
    use super::*;

    #[django_model(table = "created_tables")]
    pub struct CreatedTable {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
    impl AsyncLifecycleHooks for Model {}
}

#[tokio::test]
async fn test_create_table_works() {
    let db = setup_test_db().await;

    // Create table using the new method
    test_create_table_mod::CreatedTable::create_table(&db).await.unwrap();

    // Verify it works by inserting data
    test_create_table_mod::CreatedTable::objects(&db)
        .create(test_create_table_mod::Model { id: 0, name: "Test".to_string() })
        .await
        .unwrap();

    // Verify it's there
    let count = test_create_table_mod::CreatedTable::objects(&db).count().await.unwrap();
    assert_eq!(count, 1);

    // Test drop
    test_create_table_mod::CreatedTable::drop_table(&db).await.unwrap();

    // Verify drop
    let result = test_create_table_mod::CreatedTable::objects(&db).count().await;
    assert!(result.is_err());
}
