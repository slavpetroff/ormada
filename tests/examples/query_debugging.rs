//! Query Debugging Example - debug_sql, explain, explain_analyze
//!
//! These tools help you understand and optimize your queries:
//! - `debug_sql()` - See the exact SQL that will be executed
//! - `explain()` - Get the query execution plan (without running)
//! - `explain_analyze()` - Get actual execution statistics (runs the query)

use ormada::prelude::*;

#[ormada_model(table = "debug_books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub price: i32,
    pub category: String,
    pub published: bool,
}

pub async fn setup_db() -> Result<DatabaseRouter, OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    Book::create_table(&router).await?;
    Ok(router)
}

async fn seed_books(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    let categories = ["Fiction", "Non-Fiction", "Technical", "Biography"];
    for i in 0..100 {
        Book::objects(db)
            .create(Book {
                title: format!("Book {}", i),
                price: 1000 + (i % 50) * 100,
                category: categories[i as usize % 4].into(),
                published: i % 3 != 0,
                ..Default::default()
            })
            .await?;
    }
    Ok(())
}

/// debug_sql() - Inspect generated SQL before execution
///
/// Use this to verify your query logic and debug issues.
pub async fn example_debug_sql(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    // Complex query with multiple conditions (pretty-printed)
    let sql = Book::objects(db)
        .filter(Book::Published.eq(true))
        .filter(Book::Price.lt(5000))
        .filter(Book::Category.eq("Technical"))
        .order_by_desc(Book::Price)
        .limit(10)
        .debug_sql(true);

    // Verify SQL structure
    assert!(sql.contains("SELECT"), "Should be a SELECT query");
    assert!(sql.to_lowercase().contains("where"), "Should have WHERE clause");
    assert!(sql.to_lowercase().contains("order by"), "Should have ORDER BY");
    assert!(sql.to_lowercase().contains("limit"), "Should have LIMIT");

    // Print for debugging (in real usage)
    // println!("Generated SQL:\n{}", sql);

    Ok(())
}

/// explain() - Get query plan without executing
///
/// Use this to identify potential performance issues like:
/// - Sequential scans (missing indexes)
/// - Expensive sorts
/// - Suboptimal join strategies
pub async fn example_explain(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    // Get execution plan for a filtered query (pretty-printed)
    let plan = Book::objects(db)
        .filter(Book::Category.eq("Technical"))
        .filter(Book::Price.gt(3000))
        .order_by_asc(Book::Title)
        .explain(true)
        .await?;

    // Plan should contain query strategy info
    assert!(!plan.is_empty(), "Plan should not be empty");

    // In SQLite, plan contains "SCAN" or "SEARCH" keywords
    // In PostgreSQL, you'd see "Seq Scan", "Index Scan", etc.
    // println!("Query Plan:\n{}", plan);

    Ok(())
}

/// explain_analyze() - Get actual execution statistics
///
/// WARNING: This actually runs the query!
/// Use for performance tuning to see real execution times.
pub async fn example_explain_analyze(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    seed_books(db).await?;

    // Analyze a query that might be slow (pretty-printed)
    let analysis = Book::objects(db)
        .filter(Book::Published.eq(true))
        .filter(Book::Price.between(2000, 4000))
        .order_by_desc(Book::Price)
        .explain_analyze(true)
        .await?;

    assert!(!analysis.is_empty(), "Analysis should not be empty");

    // In real usage, look for:
    // - High "actual rows" vs "estimated rows" (statistics out of date)
    // - Long execution times on specific operations
    // - Sequential scans on large tables
    // println!("Execution Analysis:\n{}", analysis);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_debug_sql() {
        let db = setup_db().await.unwrap();
        example_debug_sql(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_explain() {
        let db = setup_db().await.unwrap();
        example_explain(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_explain_analyze() {
        let db = setup_db().await.unwrap();
        example_explain_analyze(&db).await.unwrap();
    }
}
