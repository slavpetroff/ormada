use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use sea_orm::{Database, DatabaseConnection};
use seaorm_django::prelude::*;

// Test entity for benchmarks - using ORM's django_model macro
use seaorm_django::prelude::django_model;

#[django_model(table = "benchmark_items")]
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

async fn setup_db() -> DatabaseConnection {
    use sea_orm::{ConnectionTrait, DbBackend, Schema};

    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table using SeaORM's schema builder
    // (This is infrastructure setup, not business logic - acceptable to use SeaORM directly)
    let schema = Schema::new(DbBackend::Sqlite);
    let stmt = schema.create_table_from_entity(Entity);

    db.execute(&stmt).await.expect("Failed to create table");

    db
}

async fn seed_data(db: &DatabaseConnection, count: usize) {
    use seaorm_django::query::QueryExt;

    // Bulk insert test data using ORM's bulk_create API
    let items: Vec<Model> = (0..count)
        .map(|i| Model {
            id: i as i32,
            name: format!("Item {}", i),
            value: i as i32 % 1000,
            category: format!("Category {}", i % 10),
        })
        .collect();

    // Use ORM's bulk_create in chunks (per django-orm.md rule 4)
    for chunk in items.chunks(1000) {
        let _ = Entity::objects(db).bulk_create(chunk.to_vec()).await;
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
                use seaorm_django::query::QueryExt;
                let results = Entity::objects(&db).all().await.expect("Query failed");
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
            use seaorm_django::query::QueryExt;
            let results = Entity::objects(&db)
                .filter(Column::Value.lt(500))
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
            use seaorm_django::query::QueryExt;
            let count = Entity::objects(&db).count().await.expect("Count failed");
            black_box(count)
        });
    });

    group.bench_function("sum_with_clone", |b| {
        b.to_async(&rt).iter(|| async {
            use seaorm_django::aggregations::AggregateExt;
            use seaorm_django::query::QueryExt;
            let sum = Entity::objects(&db)
                .aggregate_sum(Column::Value)
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
            use seaorm_django::query::QueryExt;
            let results = Entity::objects(&db).all().await.expect("Query failed");
            black_box(results)
        });
    });

    group.bench_function("values_json", |b| {
        b.to_async(&rt).iter(|| async {
            use seaorm_django::query::QueryExt;
            let results = Entity::objects(&db)
                .values(vec![Column::Name, Column::Value])
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
            use seaorm_django::query::QueryExt;
            let results = Entity::objects(&db).all().await.expect("Query failed");
            // Simulate processing
            let count = results.len();
            black_box(count)
        });
    });

    group.bench_function("iterator_10k", |b| {
        b.to_async(&rt).iter(|| async {
            use futures::StreamExt;
            use seaorm_django::query::QueryExt;

            let mut stream = Entity::objects(&db)
                .values_iter(vec![Column::Id, Column::Name], Some(500))
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
