//! Tests for DeleteExt trait

use crate::common::*;
use seaorm_django::query::QueryExt;
use seaorm_django::write::DeleteExt;

#[tokio::test]
async fn test_model_delete_ext() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    
    // Use DeleteExt directly
    let author = authors[0].clone();
    author.delete(&db).await.unwrap();
    
    // Verify deletion
    let remaining = author::Entity::objects(&db)
        .all()
        .await
        .unwrap();
    
    assert_eq!(remaining.len(), 2);
}
