// Benchmarks are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_raw_string_hashes)]

//! Benchmark for caching performance
//!
//! Measures the benefit of QuerySet caching vs repeated queries
//!
//! ## Configuration
//! Adjust these constants to tune benchmark behavior:
//! - TOTAL_RECORDS: Number of records to seed (default: 100,000)
//! - WARM_UP_TIME_SECS: Warm-up duration (default: 5s)
//! - MEASUREMENT_TIME_SECS: Measurement duration (default: 30s)
//! - SAMPLE_SIZE: Number of samples to collect (default: 50)

use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use ormada::prelude::*;
use sea_orm::{Database, DatabaseConnection};

// ============================================================================
// BENCHMARK CONFIGURATION - Adjust these values as needed
// ============================================================================

/// Total number of records to seed in the database
const TOTAL_RECORDS: i32 = 100_000;

/// Batch size for bulk inserts (SQLite has ~999 variable limit)
const BATCH_SIZE: i32 = 1000;

/// Warm-up time before measurement starts
const WARM_UP_TIME_SECS: u64 = 5;

/// Duration to measure each benchmark
const MEASUREMENT_TIME_SECS: u64 = 30;

/// Number of samples to collect per benchmark
const SAMPLE_SIZE: usize = 50;

// Model for benchmarking
mod benchmark_item {
    use ormada::prelude::*;

    #[ormada_model(table = "benchmark_items")]
    pub struct BenchmarkItem {
        #[primary_key]
        pub id: i32,

        #[index]
        pub value: i32,

        #[max_length(100)]
        pub data: String,
    }
}

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.expect("Failed to connect");

    // Create table using raw SQL
    use sea_orm::ConnectionTrait;

    db.execute_unprepared(
        r#"
        CREATE TABLE IF NOT EXISTS benchmark_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            value INTEGER NOT NULL,
            data TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("Failed to create table");

    // Seed records for high-load testing (batched to avoid SQLite limit)
    for batch_start in (0..TOTAL_RECORDS).step_by(BATCH_SIZE as usize) {
        let batch_end = (batch_start + BATCH_SIZE).min(TOTAL_RECORDS);
        let models: Vec<_> = (batch_start..batch_end)
            .map(|i| benchmark_item::Model {
                id: 0,
                value: i % 1000,
                data: format!("Item {}", i),
            })
            .collect();

        benchmark_item::BenchmarkItem::objects(&db)
            .bulk_create(models)
            .await
            .expect("Failed to seed batch");
    }

    db
}

fn bench_cache_hit_vs_miss(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_performance");

    // Apply benchmark configuration from module constants
    group.warm_up_time(Duration::from_secs(WARM_UP_TIME_SECS));
    group.measurement_time(Duration::from_secs(MEASUREMENT_TIME_SECS));
    group.sample_size(SAMPLE_SIZE);

    // Benchmark 1: No caching - 100 queries (high load)
    group.bench_function("no_cache_100_queries", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                for _ in 0..100 {
                    let _ = black_box(
                        benchmark_item::BenchmarkItem::objects(&db)
                            .filter(benchmark_item::BenchmarkItem::Value.eq(42))
                            .all()
                            .await
                            .unwrap(),
                    );
                }
            }
        });
    });

    // Benchmark 2: With caching - 100 queries (high load)
    group.bench_function("with_cache_100_queries", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                let queryset = benchmark_item::BenchmarkItem::objects(&db)
                    .filter(benchmark_item::BenchmarkItem::Value.eq(42));

                for _ in 0..100 {
                    let _ = black_box(queryset.all().await.unwrap());
                }
            }
        });
    });

    // Benchmark 3: Cache hit overhead (single cached query)
    group.bench_function("cache_hit_overhead", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                let queryset = benchmark_item::BenchmarkItem::objects(&db)
                    .filter(benchmark_item::BenchmarkItem::Value.eq(42));

                // First call - populates cache
                let _ = queryset.all().await.unwrap();

                // Second call - cache hit (this is what we measure)
                black_box(queryset.all().await.unwrap())
            }
        });
    });

    // Benchmark 4: First query (cache miss)
    group.bench_function("first_query_cache_miss", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                let queryset = benchmark_item::BenchmarkItem::objects(&db)
                    .filter(benchmark_item::BenchmarkItem::Value.eq(42));

                black_box(queryset.all().await.unwrap())
            }
        });
    });

    // Benchmark 5: Heavy repeated access (1000 cached calls)
    group.bench_function("heavy_cached_access_1000", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                let queryset = benchmark_item::BenchmarkItem::objects(&db)
                    .filter(benchmark_item::BenchmarkItem::Value.eq(42));

                // First call to populate cache
                let _ = queryset.all().await.unwrap();

                // Heavy load: 1000 accesses to cached result
                for _ in 0..1000 {
                    black_box(queryset.all().await.unwrap());
                }
            }
        });
    });

    // Benchmark 6: Large result set query
    group.bench_function("large_result_set_10k_rows", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                // Query that returns ~10,000 rows
                let queryset = benchmark_item::BenchmarkItem::objects(&db)
                    .filter(benchmark_item::BenchmarkItem::Value.lt(10));

                let _ = black_box(queryset.all().await.unwrap());
            }
        });
    });

    // Benchmark 7: Large result set with caching
    group.bench_function("large_result_cached_10k_rows", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                let queryset = benchmark_item::BenchmarkItem::objects(&db)
                    .filter(benchmark_item::BenchmarkItem::Value.lt(10));

                // First call
                let _ = queryset.all().await.unwrap();
                // Second call - cached
                black_box(queryset.all().await.unwrap())
            }
        });
    });

    // Benchmark 8: Complex query chain under load
    group.bench_function("complex_query_chain_50_iterations", |b| {
        let db = rt.block_on(setup_db());
        b.to_async(&rt).iter(move || {
            let db = db.clone();
            async move {
                for _ in 0..50 {
                    let _ = black_box(
                        benchmark_item::BenchmarkItem::objects(&db)
                            .filter(benchmark_item::BenchmarkItem::Value.gte(100))
                            .filter(benchmark_item::BenchmarkItem::Value.lt(200))
                            .order_by_desc(benchmark_item::BenchmarkItem::Value)
                            .limit(50)
                            .all()
                            .await
                            .unwrap(),
                    );
                }
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_cache_hit_vs_miss);
criterion_main!(benches);
