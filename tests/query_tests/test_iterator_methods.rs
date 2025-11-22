//! Tests for iterator methods (values_iter, values_list_iter)

use crate::common::*;
use futures::StreamExt;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_values_iter_basic() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let mut stream = Author::objects(&db).values_iter(vec![Author::Name], None).await.unwrap();

    let mut count = 0;
    while let Some(value) = stream.next().await {
        let value = value.unwrap();
        assert!(value.is_object());
        count += 1;
    }

    assert!(count > 0);
}

#[tokio::test]
async fn test_values_iter_with_chunk_size() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let mut stream = Author::objects(&db).values_iter(vec![Author::Name], Some(1)).await.unwrap();

    let mut count = 0;
    while let Some(value) = stream.next().await {
        let _value = value.unwrap();
        count += 1;
    }

    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_values_iter_empty_columns() {
    let db = setup_test_db().await;

    let mut stream = Author::objects(&db).values_iter(vec![], None).await.unwrap();

    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn test_values_list_iter_basic() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let mut stream = Author::objects(&db)
        .values_list_iter(vec![Author::Name, Author::Age], false, None)
        .await
        .unwrap();

    let mut count = 0;
    while let Some(value) = stream.next().await {
        let value = value.unwrap();
        assert!(value.is_array());
        count += 1;
    }

    assert!(count > 0);
}

#[tokio::test]
async fn test_values_list_iter_flat() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let mut stream = Author::objects(&db)
        .values_list_iter(vec![Author::Name], true, None)
        .await
        .unwrap();

    let mut count = 0;
    while let Some(value) = stream.next().await {
        let value = value.unwrap();
        assert!(value.is_string());
        count += 1;
    }

    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_values_list_iter_with_chunk_size() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let mut stream = Author::objects(&db)
        .values_list_iter(vec![Author::Name], false, Some(1))
        .await
        .unwrap();

    let mut count = 0;
    while let Some(value) = stream.next().await {
        let _value = value.unwrap();
        count += 1;
    }

    assert_eq!(count, 3);
}
