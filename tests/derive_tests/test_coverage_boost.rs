//! Strategic tests to achieve 95% coverage
//!
//! Tests specifically targeting uncovered code paths

use sea_orm::Related;
use seaorm_django::prelude::*;

use crate::common::{Author, Book};

// Test empty tuple LoadRelations (relations.rs lines 130, 134)
#[tokio::test]
async fn test_empty_tuple_relations() {
    use crate::common::{create_sample_authors, setup_test_db};

    let db = setup_test_db().await;
    let _authors_created = create_sample_authors(&db).await;

    // Query without prefetch_related - uses () empty tuple
    let authors = Author::objects(&db).all().await.unwrap();

    assert_eq!(authors.len(), 3);
}
