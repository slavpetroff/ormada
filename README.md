# Ormada

**The ergonomic ORM for Rust — Django's power meets Rust's safety**

[![Crates.io](https://img.shields.io/crates/v/ormada.svg)](https://crates.io/crates/ormada)
[![Documentation](https://docs.rs/ormada/badge.svg)](https://docs.rs/ormada)
[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)

Ormada brings Django's beloved ORM ergonomics to Rust while maintaining full compile-time type safety. Built on SeaORM, it provides an expressive query API, automatic validation, and production-ready features out of the box.

## Why Ormada?

### 🎯 Compile-Time Safety with Typestate Pattern

Catch query errors at compile time, not runtime:

```rust
// ✅ Valid: filter → order → paginate → execute
Book::objects(&db)
    .filter(Book::Price.lt(5000))
    .order_by_asc(Book::Title)
    .limit(10)
    .all().await?;

// ❌ Compile error: can't filter after ordering
Book::objects(&db)
    .order_by_asc(Book::Title)
    .filter(Book::Price.lt(5000))  // Error: Ordered doesn't implement CanFilter
    .all().await?;
```

### 🐍 Django-Like Ergonomics

Familiar API for developers who love Django's ORM:

```rust
// Intuitive Model.objects() pattern
let books = Book::objects(&db)
    .filter(Book::Price.lt(5000))
    .exclude(Book::OutOfPrint.eq(true))
    .order_by_desc(Book::CreatedAt)
    .limit(10)
    .all()
    .await?;

// Complex queries with Q objects
let q = Q::any()
    .add(Book::Title.contains("Rust"))
    .add(Book::Author.eq("Alice"));
let books = Book::objects(&db).filter(q).all().await?;
```

### 📝 Simple Model Definition

Define models with intuitive attributes — no boilerplate:

```rust
#[ormada_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    
    #[max_length(200)]
    pub title: String,
    
    #[foreign_key(Author)]
    pub author_id: i32,
    
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
    
    #[auto_now_add]
    pub created_at: DateTimeWithTimeZone,
    
    #[auto_now]
    pub updated_at: DateTimeWithTimeZone,
}
```

### 🔀 Smart Database Routing

Automatic primary/replica routing with read-your-writes consistency:

```rust
let primary = Database::connect("postgresql://primary/db").await?;
let replica = Database::connect("postgresql://replica/db").await?;
let router = DatabaseRouter::new(primary, replica);

// Writes automatically go to primary
Book::objects(&router).create(book).await?;

// Reads go to replica (or primary after recent write)
Book::objects(&router).all().await?;
```

### 🗑️ Built-in Soft Delete

First-class soft delete support — no manual filtering:

```rust
#[ormada_model(table = "articles")]
pub struct Article {
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

// Soft delete (sets deleted_at)
Article::objects(&db).filter(Article::Id.eq(1)).delete().await?;

// Queries exclude deleted by default
Article::objects(&db).all().await?;

// Include deleted, only deleted, or restore
Article::objects(&db).with_deleted().all().await?;
Article::objects(&db).only_deleted().all().await?;
Article::objects(&db).filter(Article::Id.eq(1)).restore().await?;
```

### ⚡ Lifecycle Hooks

Execute logic before/after CRUD operations:

```rust
#[ormada_model(table = "books", hooks = true)]
pub struct Book { /* ... */ }

#[async_trait]
impl LifecycleHooks for book::Model {
    async fn before_create(&mut self) -> Result<(), OrmadaError> {
        self.slug = slugify(&self.title);
        Ok(())
    }
    
    async fn after_create(&self) -> Result<(), OrmadaError> {
        send_notification(&self).await;
        Ok(())
    }
}
```

### 🔒 Ergonomic Transactions

Two ways to handle atomic operations:

```rust
// Option 1: tx! macro
let (author, book) = tx!(db, |txn| async move {
    let author = Author::objects(txn).create(author).await?;
    let book = Book::objects(txn)
        .create(Book { author_id: author.id, ..Default::default() })
        .await?;
    Ok((author, book))
}).await?;

// Option 2: #[atomic] decorator
#[atomic(db)]
async fn create_with_author(db: &DatabaseRouter) -> Result<Book, OrmadaError> {
    let author = Author::objects(db).create(author).await?;
    Book::objects(db).create(Book { author_id: author.id, ..Default::default() }).await
}
```

### 📦 Declarative Migrations

Same syntax as your models — no new DSL to learn:

```rust
// migrations/m001_initial.rs
#[ormada_schema(table = "books", migration = "m001_initial")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    #[max_length(200)]
    pub title: String,
}

// migrations/m002_add_isbn.rs — delta migrations
#[ormada_schema(table = "books", migration = "m002", after = "m001", extends = Book)]
pub struct Book {
    #[index]
    pub isbn: String,  // Only new fields needed
}
```

```bash
ormada migrate make "add books table"
ormada migrate run
```

### Comparison

| Feature | Ormada | SeaORM | Diesel |
|---------|:------:|:------:|:------:|
| Django-like `Model.objects()` API | ✅ | ❌ | ❌ |
| Compile-time query validation (typestate) | ✅ | ❌ | ✅ |
| FK validation at creation | ✅ | ❌ | ❌ |
| `get_or_create` / `update_or_create` | ✅ | Manual | Manual |
| Built-in soft delete | ✅ | Manual | Manual |
| Primary/replica routing | ✅ | Manual | Manual |
| Lifecycle hooks | ✅ | Manual | Manual |
| Declarative migrations (same syntax) | ✅ | ❌ | ❌ |
| Streaming iterators | ✅ | ✅ | Manual |
| Async support | ✅ | ✅ | ❌ |

## Features at a Glance

| Category | Features |
|----------|----------|
| **Safety** | Typestate query builder, FK validation, compile-time relation checks |
| **Ergonomics** | `Model.objects()` API, Q objects, Django-style error types |
| **Performance** | Bulk ops (10-100x faster), query caching, streaming iterators |
| **Database** | Primary/replica routing, read-your-writes, multi-DB support |
| **CRUD** | `get_or_create`, `update_or_create`, `upsert_many`, bulk create |
| **Relations** | `select_related` (JOIN), `prefetch_related` (N+1 prevention) |
| **Lifecycle** | `before_create`, `after_save`, `before_delete`, and more |
| **Soft Delete** | `with_deleted()`, `only_deleted()`, `restore()`, `force_delete()` |
| **Aggregations** | COUNT, SUM, AVG, MIN, MAX, GROUP BY with projections |
| **Debugging** | `explain()`, `explain_analyze()`, `debug_sql()` |
| **Migrations** | Declarative schema, delta migrations, data migrations |

## Installation

```toml
[dependencies]
ormada = "0.1"
```

## Quick Start

```rust
use ormada::prelude::*;

#[tokio::main]
async fn main() -> Result<(), OrmadaError> {
    let db = Database::connect("sqlite::memory:").await?;
    
    // Create
    let book = Book::objects(&db)
        .create(Book {
            title: "The Rust Book".into(),
            author_id: 1,
            price: 2999,
            ..Default::default()
        })
        .await?;
    
    // Query with filters, ordering, pagination
    let books = Book::objects(&db)
        .filter(Book::Price.lt(5000))
        .order_by_desc(Book::CreatedAt)
        .limit(10)
        .all()
        .await?;
    
    // Upsert operations
    let (author, created) = Author::objects(&db)
        .filter(Author::Email.eq("alice@example.com"))
        .get_or_create(|| async {
            Ok(Author { name: "Alice".into(), email: "alice@example.com".into(), ..Default::default() })
        })
        .await?;
    
    // Eager loading (prevent N+1)
    let books = Book::objects(&db)
        .select_related(relations![Author])
        .all()
        .await?;
    
    Ok(())
}
```

## Documentation

📖 **[Full API Documentation](https://docs.rs/ormada)** — Complete reference with examples

See also: [`docs/guide.md`](docs/guide.md) for comprehensive usage guide.

## Crate Structure

| Crate | Description |
|-------|-------------|
| [`ormada`](https://crates.io/crates/ormada) | Core ORM library |
| [`ormada-derive`](https://crates.io/crates/ormada-derive) | Proc macros (`#[ormada_model]`, `#[atomic]`, etc.) |
| [`ormada-schema`](https://crates.io/crates/ormada-schema) | Schema types for migrations |
| [`ormada-cli`](https://crates.io/crates/ormada-cli) | CLI for migration management |

## Performance

Benchmarks on SQLite in-memory (M1 Mac, release build):

| Operation | 1,000 rows | 10,000 rows |
|-----------|------------|-------------|
| `all()` | ~764 µs | ~7.3 ms |
| `count()` | ~33 µs | ~33 µs |
| Cached queries | **-98.9%** | **-49.2%** overhead |
| Bulk insert | **10-100x** faster than individual |

Run benchmarks: `cargo bench`

## Database Support

| Database | Status |
|----------|--------|
| PostgreSQL | ✅ Full support (recommended) |
| SQLite | ✅ Full support |
| MySQL | 🔶 Partial support |

## Minimum Supported Rust Version

Rust 1.75 or later.

## Contributing

Contributions welcome! Please read our [Contributing Guide](CONTRIBUTING.md).

## License

MIT License — see [LICENSE](LICENSE) for details.
