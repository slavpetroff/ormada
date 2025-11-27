# Ormada

**Ergonomic ORM for SeaORM with zero-cost abstractions**

[![Coverage](https://img.shields.io/badge/coverage-84%25-green)](./tarpaulin-report.html)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](./LICENSE)

## Features

- 🚀 **Zero-cost abstractions** - Compile-time typed queries, no runtime overhead
- 🎯 **Type-safe** - Full compile-time checking with typestate pattern
- 🐍 **Ergonomic API** - Familiar, expressive queries with minimal boilerplate
- ⚡ **Performance** - Direct integration with SeaORM, no duplication

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ormada = { version = "0.3", features = ["derive"] }
sea-orm = "0.12"
```

### Define a Model

```rust
use ormada::prelude::*;

#[ormada_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    
    #[max_length(200)]
    pub title: String,
    
    pub price: i32,
    pub published: bool,
    
    #[auto_now_add]
    pub created_at: DateTimeWithTimeZone,
    
    #[auto_now]
    pub updated_at: DateTimeWithTimeZone,
}
```

That's it! No manual `impl LifecycleHooks` needed.

### Connect and Query

```rust
use ormada::prelude::*;

#[tokio::main]
async fn main() -> Result<(), OrmadaError> {
    // Connect using DatabaseRouter (supports primary/replica routing)
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    
    // Create table
    Book::create_table(&router).await?;
    
    // Create a book
    let book = Book::objects(&router)
        .create(Book {
            title: "The Rust Book".to_string(),
            price: 2999,
            published: true,
            ..Default::default()    
        })
        .await?;
    
    // Query with filters
    let cheap_books = Book::objects(&router)
        .filter(Book::Price.lt(3000))
        .filter(Book::Published.eq(true))
        .order_by_desc(Book::CreatedAt)
        .limit(10)
        .all()
        .await?;
    
    // Aggregations
    let count = Book::objects(&router)
        .filter(Book::Published.eq(true))
        .count()
        .await?;
    
    Ok(())
}
```

## DatabaseRouter

The `DatabaseRouter` provides intelligent routing for read/write operations:

```rust
use ormada::prelude::*;

// Single database (development)
let router = DatabaseRouter::new_single(primary_db);

// Primary + Replica (production)
let router = DatabaseRouter::new(primary_db, replica_db);

// Writes go to primary, reads go to replica (unless in transaction)
Book::objects(&router).create(...).await?;  // → Primary
Book::objects(&router).all().await?;        // → Replica
```

### Read-Your-Writes Consistency

```rust
// After a write, reads temporarily go to primary
Book::objects(&router).create(book).await?;
let books = Book::objects(&router).all().await?;  // → Primary (for consistency)
```

## Transactions

### Using `tx!` Macro (Recommended)

```rust
use ormada::prelude::*;

let result = tx!(router, |txn| async move {
    let author = Author::objects(txn)
        .create(Author { name: "Alice".into(), ..Default::default() })
        .await?;
    
    let book = Book::objects(txn)
        .create(Book { author_id: author.id, ..Default::default() })
        .await?;
    
    Ok((author, book))
}).await?;
```

### Using `#[atomic]` Decorator

```rust
use ormada::prelude::*;

#[atomic(db)]
async fn create_author_with_book(db: &DatabaseRouter) -> Result<(), OrmadaError> {
    // Everything here runs in a transaction
    let author = Author::objects(db).create(...).await?;
    let book = Book::objects(db).create(...).await?;
    Ok(())
}
```

## Lifecycle Hooks

By default, hooks are auto-generated and do nothing. For custom hooks:

```rust
#[ormada_model(table = "books", hooks = true)]
pub struct Book { /* fields */ }

#[async_trait]
impl LifecycleHooks for book::Model {
    async fn before_save(&mut self) -> Result<(), OrmadaError> {
        // Validate, transform, log, etc.
        Ok(())
    }
    
    async fn after_create(&self) -> Result<(), OrmadaError> {
        // Send notification, update cache, etc.
        Ok(())
    }
}
```

## Field Attributes

| Attribute | Description |
|-----------|-------------|
| `#[primary_key]` | Mark as primary key |
| `#[foreign_key(Model)]` | Define foreign key relationship |
| `#[index]` | Create database index |
| `#[unique]` | Unique constraint |
| `#[max_length(n)]` | String max length validation |
| `#[auto_now]` | Update timestamp on every save |
| `#[auto_now_add]` | Set timestamp on creation only |
| `#[soft_delete]` | Enable soft delete (field must be `Option<DateTimeWithTimeZone>`) |

## Query Methods

```rust
// Filtering
.filter(Book::Price.lt(3000))
.exclude(Book::Published.eq(false))

// Ordering
.order_by_asc(Book::Title)
.order_by_desc(Book::CreatedAt)

// Pagination
.limit(10)
.offset(20)

// Retrieval
.all().await?           // Vec<Book>
.first().await?         // Option<Book>
.get(id).await?         // Book (or error)
.count().await?         // u64
.exists().await?        // bool

// Aggregations
.aggregate_sum(Book::Price).await?
.aggregate_avg(Book::Price).await?
.aggregate_max(Book::Price).await?

// Complex queries with Q objects
let q = Q::any()
    .add(Book::Title.contains("Rust"))
    .add(Book::Title.contains("Python"));
Book::objects(&db).filter(q).all().await?
```

## Soft Delete

```rust
#[ormada_model(table = "articles")]
pub struct Article {
    #[primary_key]
    pub id: i32,
    pub title: String,
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

// Soft delete (sets deleted_at)
article.delete(&db).await?;

// Query excludes soft-deleted by default
Article::objects(&db).all().await?;  // Only non-deleted

// Include soft-deleted
Article::objects(&db).with_deleted().all().await?;

// Only soft-deleted
Article::objects(&db).only_deleted().all().await?;

// Restore
article.restore(&db).await?;

// Force delete (permanent)
article.force_delete(&db).await?;
```

## Typestate Query Validation

Queries are validated at compile time:

```rust
// ✅ Valid chain
Book::objects(db)
    .filter(...)      // Fresh → Filtered
    .order_by_asc(...)// Filtered → Ordered
    .limit(10)        // Ordered → Paginated
    .all().await?;

// ❌ Won't compile: can't filter after ordering
Book::objects(db)
    .order_by_asc(...)
    .filter(...)      // Error: Ordered doesn't implement CanFilter
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
