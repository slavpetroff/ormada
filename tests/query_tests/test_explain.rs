use crate::common::*;

#[tokio::test]
async fn test_explain_basic_query() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let plan = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .explain()
        .await
        .unwrap();

    assert!(!plan.is_empty(), "Explain should return a plan");
    assert!(plan.contains("SELECT"), "Plan should contain SQL");
    assert!(plan.contains("books"), "Plan should reference table name");
}

#[tokio::test]
async fn test_explain_with_filters() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let plan = Book::objects(&db)
        .filter(Book::Author.eq("Tolkien"))
        .filter(Book::Price.gte(2000))
        .explain()
        .await
        .unwrap();

    assert!(!plan.is_empty());
    assert!(plan.contains("WHERE") || plan.contains("where"), "Should show WHERE clause");
}

#[tokio::test]
async fn test_explain_with_ordering() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let plan = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::Price)
        .explain()
        .await
        .unwrap();

    assert!(!plan.is_empty());
    assert!(plan.contains("ORDER BY") || plan.contains("order by"), "Should show ORDER BY");
}

#[tokio::test]
async fn test_explain_analyze() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let analysis = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .explain_analyze()
        .await
        .unwrap();

    assert!(!analysis.is_empty(), "Explain analyze should return results");
    assert!(analysis.contains("EXPLAIN"), "Should contain EXPLAIN");
}

#[tokio::test]
async fn test_explain_with_limit() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let plan = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .limit(10)
        .explain()
        .await
        .unwrap();

    assert!(!plan.is_empty());
    assert!(plan.contains("LIMIT") || plan.contains("limit"), 
           "Plan should reference the LIMIT");
}

#[tokio::test]
async fn test_debug_sql_still_works() {
    let db = setup_test_db().await;

    let sql = Book::objects(&db)
        .filter(Book::Author.eq("Test"))
        .debug_sql();

    assert!(!sql.is_empty());
    assert!(sql.to_lowercase().contains("select"));
    assert!(sql.to_lowercase().contains("where"));
}

// Edge case tests

#[tokio::test]
async fn test_explain_empty_queryset() {
    let db = setup_test_db().await;

    let plan = Book::objects(&db)
        .explain()
        .await
        .unwrap();

    assert!(!plan.is_empty(), "Should return plan even for empty table");
}

#[tokio::test]
async fn test_explain_complex_query() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let plan = Book::objects(&db)
        .filter(Book::Price.gte(1000))
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::Price)
        .limit(5)
        .explain()
        .await
        .unwrap();

    assert!(!plan.is_empty());
    let plan_lower = plan.to_lowercase();
    assert!(plan_lower.contains("select"), "Should contain SELECT");
}

#[tokio::test]
async fn test_explain_after_filter_chain() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let queryset = Book::objects(&db)
        .filter(Book::Published.eq(true))
        .filter(Book::Price.lte(3000));
    
    let plan = queryset.explain().await.unwrap();
    
    assert!(!plan.is_empty());
}

#[tokio::test]
async fn test_explain_preserves_query() {
    let db = setup_test_db().await;
    let _books = create_sample_books(&db).await;

    let queryset = Book::objects(&db)
        .filter(Book::Published.eq(true));
    
    // Get explain
    let _plan = queryset.explain().await.unwrap();
    
    // Query should still work after explain
    let results = queryset.all().await.unwrap();
    assert!(results.len() > 0, "Query should still execute after explain");
}
