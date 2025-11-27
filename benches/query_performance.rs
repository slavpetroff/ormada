// Benchmarks are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::uninlined_format_args)]

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ormada::prelude::*;
use ormada::router::DatabaseRouter;

// Test entity for benchmarks - using ORM's ormada_model macro

#[ormada_model(table = "benchmark_items")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BenchmarkItem {
    #[primary_key]
    #[serde(skip_deserializing)]
    id: i32,

    pub name: String,
    pub value: i32,
    pub category: String,
}

// Use the generated module directly

async fn setup_db() -> DatabaseRouter {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");
    let router = DatabaseRouter::new_single(db);

    // Create table using ormada's generated method
    BenchmarkItem::create_table(&router).await.expect("Failed to create table");

    router
}

async fn seed_data(db: &DatabaseRouter, count: usize) {
    // Bulk insert test data using ORM's bulk_create API
    let items: Vec<Model> = (0..count)
        .map(|i| Model {
            name: format!("Item {i}"),
            value: i as i32 % 1000,
            category: format!("Category {}", i % 10),
            ..Default::default()
        })
        .collect();

    // Use ORM's bulk_create in chunks
    for chunk in items.chunks(1000) {
        let _ = BenchmarkItem::objects(db).bulk_create(chunk.to_vec()).await;
    }
}

fn bench_query_all_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_all");

    for size in [100, 1000, 10_000].iter() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let db = rt.block_on(setup_db());
        rt.block_on(seed_data(&db, *size));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.to_async(&rt).iter(|| async {
                let results = BenchmarkItem::objects(&db).all().await.expect("Query failed");
                black_box(results)
            });
        });
    }

    group.finish();
}

fn bench_query_filtered(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(setup_db());
    rt.block_on(seed_data(&db, 10_000));

    c.bench_function("query_filtered_10k", |b| {
        b.to_async(&rt).iter(|| async {
            let results = BenchmarkItem::objects(&db)
                .filter(BenchmarkItem::Value.lt(500))
                .all()
                .await
                .expect("Query failed");
            black_box(results)
        });
    });
}

fn bench_aggregations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(setup_db());
    rt.block_on(seed_data(&db, 10_000));

    let mut group = c.benchmark_group("aggregations");

    group.bench_function("count", |b| {
        b.to_async(&rt).iter(|| async {
            let count = BenchmarkItem::objects(&db).count().await.expect("Count failed");
            black_box(count)
        });
    });

    group.bench_function("sum_with_clone", |b| {
        b.to_async(&rt).iter(|| async {
            use ormada::aggregations::AggregateExt;
            let sum = BenchmarkItem::objects(&db)
                .aggregate_sum(BenchmarkItem::Value)
                .await
                .expect("Sum failed");
            black_box(sum)
        });
    });

    group.finish();
}

fn bench_values_vs_models(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(setup_db());
    rt.block_on(seed_data(&db, 1000));

    let mut group = c.benchmark_group("values_vs_models");

    group.bench_function("full_models", |b| {
        b.to_async(&rt).iter(|| async {
            let results = BenchmarkItem::objects(&db).all().await.expect("Query failed");
            black_box(results)
        });
    });

    group.bench_function("values_json", |b| {
        b.to_async(&rt).iter(|| async {
            let results = BenchmarkItem::objects(&db)
                .values(vec![BenchmarkItem::Name, BenchmarkItem::Value])
                .await
                .expect("Values failed");
            black_box(results)
        });
    });

    group.finish();
}

fn bench_iterator_vs_all(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(setup_db());
    rt.block_on(seed_data(&db, 10_000));

    let mut group = c.benchmark_group("iterator_vs_all");

    group.bench_function("all_10k", |b| {
        b.to_async(&rt).iter(|| async {
            let results = BenchmarkItem::objects(&db).all().await.expect("Query failed");
            // Simulate processing
            let count = results.len();
            black_box(count)
        });
    });

    group.bench_function("values_iter_10k", |b| {
        b.to_async(&rt).iter(|| async {
            use futures::StreamExt;

            let mut stream = BenchmarkItem::objects(&db)
                .values_iter(vec![BenchmarkItem::Id, BenchmarkItem::Name], Some(500))
                .await
                .expect("Iterator failed");

            let mut count = 0;
            while let Some(result) = stream.next().await {
                result.expect("Stream error");
                count += 1;
            }
            black_box(count)
        });
    });

    group.bench_function("model_iter_10k", |b| {
        b.to_async(&rt).iter(|| async {
            use futures::StreamExt;

            let mut stream =
                BenchmarkItem::objects(&db).iterator(Some(500)).await.expect("Iterator failed");

            let mut count = 0;
            while let Some(result) = stream.next().await {
                result.expect("Stream error");
                count += 1;
            }
            black_box(count)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_query_all_sizes,
    bench_query_filtered,
    bench_aggregations,
    bench_values_vs_models,
    bench_iterator_vs_all,
);
criterion_main!(benches);
