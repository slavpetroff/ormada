# Ormada

**Ergonomic ORM for Rust with compile-time safety and zero-cost abstractions**

[![Crates.io](https://img.shields.io/crates/v/ormada.svg)](https://crates.io/crates/ormada)
[![Documentation](https://docs.rs/ormada/badge.svg)](https://docs.rs/ormada)
[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![CI](https://github.com/your-org/ormada/workflows/CI/badge.svg)](https://github.com/your-org/ormada/actions)

Ormada brings Django-like ergonomics to Rust while maintaining full type safety. Built on top of SeaORM, it provides an expressive query API, automatic validation, and compile-time guarantees.

## Why Ormada?

| Feature | Ormada | SeaORM | Diesel |
|---------|--------|--------|--------|
| Django-like API (`Model.objects()`) | ✅ | ❌ | ❌ |
| Compile-time query validation (typestate) | ✅ | ❌ | ✅ |
| FK validation at creation | ✅ | ❌ | ❌ |
| `get_or_create` / `update_or_create` | ✅ Built-in | ❌ Manual | ❌ Manual |
| Streaming iterators | ✅ Built-in | ✅ `paginate()` | ❌ Manual |
| Query `explain()` / `explain_analyze()` | ✅ Built-in | ❌ Manual | 📦 Crate |
| Soft delete | ✅ Built-in | ❌ Manual | 📦 Crate |
| Primary/replica routing | ✅ Built-in | ❌ Manual | ❌ Manual |
| Lifecycle hooks | ✅ Built-in | ❌ Manual | ❌ Manual |
| Upsert (`ON CONFLICT`) | ✅ | ✅ | ✅ |
| Async support | ✅ | ✅ | ❌ |

**Legend:** ✅ = Built-in, ❌ = Not available/Manual implementation required, 📦 = Available via separate crate

## Features

- 🚀 **Zero-cost abstractions** — Compile-time typed queries with no runtime overhead
- 🎯 **Type-safe** — FK validation, query state machine, compile-time relation checks
- 🐍 **Ergonomic API** — Django-inspired `Model.objects()` pattern
- ⚡ **High performance** — Bulk operations (10-100x faster), streaming iterators, query caching
- 🔒 **Transaction support** — `tx!` macro and `#[atomic]` decorator with automatic rollback
- 🔗 **Relations** — Eager loading with `prefetch_related()` to prevent N+1 queries
- 📊 **Aggregations** — COUNT, SUM, AVG, MIN, MAX at database level
- 🗄️ **Database routing** — Primary/replica with read-your-writes consistency
- 🔍 **Query debugging** — `explain()`, `explain_analyze()`, `debug_sql()`
- 📦 **Upsert operations** — `get_or_create()`, `update_or_create()`, `upsert_many()`
- 🌊 **Streaming** — Memory-efficient `iterator()` for million-row datasets
- 🗑️ **Soft delete** — Built-in with `with_deleted()`, `only_deleted()`, `restore()`

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Model Definition](#model-definition)
- [Query API](#query-api)
- [CRUD Operations](#crud-operations)
- [Upsert Operations](#upsert-operations)
- [Bulk Operations](#bulk-operations)
- [Streaming & Iterators](#streaming--iterators)
- [Relations](#relations)
- [Transactions](#transactions)
- [Aggregations](#aggregations)
- [Soft Delete](#soft-delete)
- [Query Debugging](#query-debugging)
- [Database Router](#database-router)
- [Lifecycle Hooks](#lifecycle-hooks)
- [Validation](#validation)
- [Error Handling](#error-handling)
- [Advanced Features](#advanced-features)
- [License](#license)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ormada = "0.1"
```

The `derive` feature is enabled by default. For minimal builds:

```toml
[dependencies]
ormada = { version = "0.1", default-features = false }
```

## Quick Start

```rust
use ormada::prelude::*;
// prelude includes: ormada_model, OrmadaError, DatabaseRouter, 
// Database, DateTimeWithTimeZone, Q, tx!, and column traits

#[ormada_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    
    #[max_length(200)]
    pub title: String,
    
    pub price: i32,
    
    #[auto_now_add]
    pub created_at: DateTimeWithTimeZone,
}

#[tokio::main]
async fn main() -> Result<(), OrmadaError> {
    // Connect to database
    let db = Database::connect("sqlite::memory:").await?;
    let router = DatabaseRouter::new_single(db);
    
    // Create table (development only - use migrations in production)
    Book::create_table(&router).await?;
    
    // Create a record
    let book = Book::objects(&router)
        .create(Book {
            title: "The Rust Book".into(),
            price: 2999,
            ..Default::default()
        })
        .await?;
    
    // Query with filters, ordering, and pagination
    let books = Book::objects(&router)
        .filter(Book::Price.lt(5000))
        .order_by_desc(Book::CreatedAt)
        .limit(10)
        .all()
        .await?;
    
    Ok(())
}
```

## Model Definition

### Basic Model

```rust
use ormada::prelude::*;

#[ormada_model(table = "users")]
pub struct User {
    #[primary_key]
    pub id: i32,
    
    #[max_length(100)]
    pub name: String,
    
    #[unique]
    pub email: String,
    
    #[index]
    pub age: i32,
}
```

### Field Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `#[primary_key]` | Primary key (auto-increment by default) | `pub id: i32` |
| `#[primary_key(auto_increment = false)]` | Non-auto-increment PK (UUID, composite) | `pub id: Uuid` |
| `#[foreign_key(Model)]` | Many-to-One (cascade delete) | `pub author_id: i32` |
| `#[foreign_key(Model, on_delete = SetNull)]` | Nullable FK with SET NULL | `pub category_id: Option<i32>` |
| `#[one_to_one(Model)]` | One-to-One relationship | `pub profile_id: i32` |
| `#[many_to_many(Model, through = JoinModel)]` | Many-to-Many with join table | `// See M:N section` |
| `#[max_length(n)]` | String max length validation | `pub title: String` |
| `#[min_length(n)]` | String min length validation | `pub code: String` |
| `#[range(min = a, max = b)]` | Numeric range validation | `pub age: i32` |
| `#[unique]` | Unique constraint | `pub email: String` |
| `#[index]` | Database index | `pub created_at: DateTimeWithTimeZone` |
| `#[auto_now]` | Update timestamp on every save | `pub updated_at: DateTimeWithTimeZone` |
| `#[auto_now_add]` | Set timestamp on creation only | `pub created_at: DateTimeWithTimeZone` |
| `#[soft_delete]` | Enable soft delete | `pub deleted_at: Option<DateTimeWithTimeZone>` |

### Model Organization (Required for Foreign Keys)

When using `#[foreign_key]`, models must be organized in **separate submodules** to allow proper macro resolution:

```rust
use ormada::prelude::*;

// ✅ CORRECT: Each model in its own submodule
pub mod models {
    pub mod author {
        use ormada::prelude::*;

        #[ormada_model(table = "authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,
            pub name: String,
        }
    }

    pub mod book {
        use ormada::prelude::*;
        use super::author::Author;  // Import the parent model

        #[ormada_model(table = "books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author)]  // Reference the imported model
            pub author_id: i32,
            pub title: String,
        }
    }
}

// Re-export for convenience
pub use models::author::Author;
pub use models::book::Book;
```

> 💡 **Why submodules?** The `#[ormada_model]` macro generates internal types that would conflict if multiple models were in the same module. This pattern also provides clear separation of concerns and matches Django's `models.py` organization.

### Supported Primary Key Types

```rust
use ormada::prelude::*;
use uuid::Uuid;  // Add uuid = { version = "1.0", features = ["v4"] } to Cargo.toml

// Integer PKs (auto-increment)
#[primary_key]
pub id: i32,

#[primary_key]
pub id: i64,

// UUID PK (must set auto_increment = false)
#[primary_key(auto_increment = false)]
pub id: Uuid,

// Composite PK
#[primary_key(auto_increment = false)]
pub order_id: i32,
#[primary_key(auto_increment = false)]
pub item_id: i32,
```

### Foreign Keys

```rust
use ormada::prelude::*;

#[ormada_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    
    // Required FK - validated at creation time
    #[foreign_key(Author)]
    pub author_id: i32,
    
    // Optional FK with SET NULL on delete
    #[foreign_key(Category, on_delete = SetNull)]
    pub category_id: Option<i32>,
}
```

**FK Validation**: Non-nullable FKs are validated at creation time. Using `Default::default()` on a model with FKs will fail:

```rust
// ❌ Fails: author_id would be 0
Book::objects(&db).create(Book { ..Default::default() }).await;

// ✅ Correct: explicitly set author_id
Book::objects(&db).create(Book { 
    author_id: author.id,
    ..Default::default() 
}).await;
```

## Query API

### Filtering

```rust
// Basic filters
Book::objects(&db).filter(Book::Price.lt(3000)).all().await?;
Book::objects(&db).filter(Book::Title.eq("Rust")).all().await?;
Book::objects(&db).exclude(Book::Published.eq(false)).all().await?;

// String operations
Book::Title.contains("rust")      // LIKE '%rust%'
Book::Title.starts_with("The")    // LIKE 'The%'
Book::Title.ends_with("Guide")    // LIKE '%Guide'
Book::Title.icontains("RUST")     // Case-insensitive

// Comparisons
Book::Price.eq(2999)              // =
Book::Price.ne(0)                 // !=
Book::Price.gt(1000)              // >
Book::Price.gte(1000)             // >=
Book::Price.lt(5000)              // <
Book::Price.lte(5000)             // <=

// Null checks
Book::Description.is_null()
Book::Description.is_not_null()

// IN queries
Book::Id.is_in(vec![1, 2, 3])
```

### Complex Queries with Q Objects

```rust
// OR conditions
let q = Q::any()
    .add(Book::Title.contains("Rust"))
    .add(Book::Title.contains("Python"));

// AND conditions (default)
let q = Q::all()
    .add(Book::Price.lt(5000))
    .add(Book::Published.eq(true));

// NOT conditions
let q = Q::not(Book::Price.gt(10000));

// Combine
let q = Q::any()
    .add(Q::all()
        .add(Book::Price.lt(3000))
        .add(Book::InStock.eq(true)))
    .add(Book::Featured.eq(true));

Book::objects(&db).filter(q).all().await?;
```

### Ordering

```rust
Book::objects(&db)
    .order_by_asc(Book::Title)
    .all().await?;

Book::objects(&db)
    .order_by_desc(Book::CreatedAt)
    .all().await?;

// Multiple orderings
Book::objects(&db)
    .order_by_desc(Book::Featured)
    .order_by_asc(Book::Title)
    .all().await?;
```

### Pagination

```rust
Book::objects(&db)
    .limit(10)
    .offset(20)
    .all().await?;
```

### Retrieval Methods

```rust
// All matching records
let books: Vec<Book> = Book::objects(&db).all().await?;

// First matching record
let book: Book = Book::objects(&db).first().await?;

// Get by primary key
let book: Book = Book::objects(&db).get(42).await?;

// Check existence
let exists: bool = Book::objects(&db)
    .filter(Book::Isbn.eq("978-0134685991"))
    .exists().await?;

// Count
let count: u64 = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .count().await?;

// Earliest/Latest by field
let oldest = Book::objects(&db).earliest(Book::CreatedAt).await?;
let newest = Book::objects(&db).latest(Book::CreatedAt).await?;
```

### Distinct

```rust
Book::objects(&db)
    .distinct()
    .all().await?;
```

## CRUD Operations

### Create

```rust
let book = Book::objects(&db)
    .create(Book {
        title: "New Book".into(),
        price: 2999,
        ..Default::default()
    })
    .await?;
```

### Read

```rust
let book = Book::objects(&db).get(1).await?;
let books = Book::objects(&db).filter(Book::Price.lt(5000)).all().await?;
```

### Update

```rust
// Update single record
Book::objects(&db)
    .filter(Book::Id.eq(1))
    .update(|mut book| async move {
        book.price = 1999;
        Ok(book)
    })
    .await?;

// Bulk update
Book::objects(&db)
    .filter(Book::Price.lt(1000))
    .update(|mut book| async move {
        book.price = 999;
        Ok(book)
    })
    .await?;
```

### Delete

```rust
// Delete matching records
Book::objects(&db)
    .filter(Book::Published.eq(false))
    .delete()
    .await?;
```

## Upsert Operations

Race-condition safe operations with automatic retry on unique constraint violations.

### Get or Create

```rust
// Get existing or create new - thread-safe with automatic retry
let (author, created) = Author::objects(&db)
    .filter(Author::Email.eq("john@example.com"))
    .get_or_create(|| async {
        Ok(Author {
            name: "John Doe".into(),
            email: "john@example.com".into(),
            ..Default::default()
        })
    })
    .await?;

if created {
    println!("Created new author: {}", author.name);
} else {
    println!("Found existing author: {}", author.name);
}
```

### Update or Create

```rust
// Update if exists, create if not - with async closure support
let (book, created) = Book::objects(&db)
    .filter(Book::Isbn.eq("978-1234567890"))
    .update_or_create(
        |mut book| async move {
            // Update existing - async operations supported!
            book.price = 2999;
            book.updated_count += 1;
            Ok(book)
        },
        || async {
            // Create new
            Ok(Book {
                isbn: "978-1234567890".into(),
                title: "New Book".into(),
                price: 2999,
                ..Default::default()
            })
        }
    )
    .await?;
```

## Bulk Operations

High-performance batch operations - **10-100x faster** than individual inserts.

> ⚠️ **Note:** Bulk operations (`bulk_create`, `upsert_many`) do **NOT** trigger lifecycle hooks for performance reasons. If you need hooks, use individual `create()` calls.

### Bulk Create

```rust
use ormada::prelude::*;

// Create 1000 records in a single query
let authors: Vec<Author> = (0..1000)
    .map(|i| Author {
        name: format!("Author {i}"),
        email: format!("author{i}@example.com"),
        ..Default::default()
    })
    .collect();

let count = Author::objects(&db).bulk_create(authors).await?;
// ~0.1-0.5 seconds vs ~5-10 seconds for individual inserts
```

### Bulk Upsert

```rust
// INSERT ... ON CONFLICT DO UPDATE in a single query
let books = vec![
    Book { isbn: "123".into(), title: "Book 1".into(), price: 1000, ..Default::default() },
    Book { isbn: "456".into(), title: "Book 2".into(), price: 2000, ..Default::default() },
];

Book::objects(&db)
    .upsert_many(books)
    .on_conflict(Book::Isbn)           // Conflict column
    .update_fields(&[Book::Title, Book::Price])  // Fields to update
    .execute()
    .await?;

// Generated SQL:
// INSERT INTO books (isbn, title, price) VALUES (...)
// ON CONFLICT (isbn) DO UPDATE SET title = EXCLUDED.title, price = EXCLUDED.price
```

## Streaming & Iterators

Process millions of rows without loading everything into memory.

### Stream Full Models

```rust
use ormada::prelude::*;
use futures::StreamExt;  // Add futures = "0.3" to Cargo.toml

// Process 1 million rows with only 100 in memory at a time
let mut stream = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .iterator(Some(100))  // Chunk size: 100 rows
    .await?;

while let Some(result) = stream.next().await {
    let book = result?;
    process_book(&book).await?;
}
```

### Stream Column Values

```rust
// Stream only specific columns (more efficient)
let mut stream = Book::objects(&db)
    .values_iter(vec![Book::Title, Book::Price], Some(500))
    .await?;

while let Some(result) = stream.next().await {
    let value = result?;
    println!("Title: {}, Price: {}", value["title"], value["price"]);
}
```

### Values and Values List

```rust
// Get specific columns as JSON
let values = Book::objects(&db)
    .values(vec![Book::Title, Book::Price])
    .await?;

// Get flat list of single column
let titles: Vec<Value> = Book::objects(&db)
    .values_list(vec![Book::Title], true)  // flat=true
    .await?;

// Get tuples
let pairs = Book::objects(&db)
    .values_list(vec![Book::Title, Book::Price], false)
    .await?;
```

### Type-Safe Projections

For type-safe column selection (instead of JSON), use `project<T>()`:

```rust
use ormada::prelude::*;
use sea_orm::FromQueryResult;

// Define a DTO with only the fields you need
#[derive(Debug, Clone, FromQueryResult)]
pub struct BookSummary {
    pub title: String,
    pub price: i32,
}

// Project to your DTO - compile-time type safety!
let summaries: Vec<BookSummary> = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .project::<BookSummary>()
    .await?;

// Access typed fields directly (no JSON parsing)
for summary in summaries {
    println!("{}: ${:.2}", summary.title, summary.price as f64 / 100.0);
}
```

### Optimized Column Selection

For large tables, use `project_columns()` to select only the fields you need:

```rust
// Only SELECT title, price - not all columns from the table
let summaries: Vec<BookSummary> = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .project_columns::<BookSummary>(&[Book::Title, Book::Price])
    .await?;
```

> 💡 **Tip:** Use `project<T>()` for convenience, `project_columns<T>()` for optimization, and `values()` for dynamic field selection.

## Relations

### Defining Relations

```rust
use ormada::prelude::*;

#[ormada_model(table = "authors")]
pub struct Author {
    #[primary_key]
    pub id: i32,
    pub name: String,
}

#[ormada_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    
    #[foreign_key(Author)]
    pub author_id: i32,
    
    pub title: String,
}
```

### Eager Loading (Prevent N+1)

Ormada provides two methods for eager loading relations:

| Method | Best For | Use When |
|--------|----------|----------|
| `select_related` | FK, 1:1 relations | Loading parent from child (Book → Author) |
| `prefetch_related` | 1:N, M:N relations | Loading children from parent |

Both methods use batched queries (1+M pattern) to prevent N+1 queries.

```rust
use ormada::prelude::*;

// select_related - Following FK (Book -> Author)
// Best for: Loading the "one" side of a relationship
let books = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .select_related(relations![Author])
    .all()
    .await?;

for book in books {
    // Author is already loaded - no additional query!
    println!("{} by {}", book.title, book.author.name);
}

// prefetch_related - Same syntax, same result for FK relations
let books = Book::objects(&db)
    .prefetch_related(relations![Author])
    .all()
    .await?;

// Multiple relations
let books = Book::objects(&db)
    .prefetch_related(relations![Author, Publisher])
    .all()
    .await?;

// With filters and ordering
let books = Book::objects(&db)
    .filter(Book::Price.gte(1000))
    .order_by_desc(Book::Price)
    .limit(10)
    .prefetch_related(relations![Author])
    .all()
    .await?;
```

> 💡 **Performance**: For 100 books by 5 authors, eager loading executes 2 queries instead of 101 (N+1)!

### One-to-One Relationships

Use `#[one_to_one(Model)]` for 1:1 relationships (e.g., User-Profile):

```rust
use ormada::prelude::*;

pub mod models {
    pub mod user {
        use ormada::prelude::*;

        #[ormada_model(table = "users")]
        pub struct User {
            #[primary_key]
            pub id: i32,
            pub username: String,
        }
    }

    pub mod profile {
        use ormada::prelude::*;
        use super::user::User;

        #[ormada_model(table = "profiles")]
        pub struct Profile {
            #[primary_key]
            pub id: i32,
            #[one_to_one(User)]  // 1:1 relationship
            pub user_id: i32,
            pub bio: String,
        }
    }
}

// Query profile by user
let profile = Profile::objects(&db)
    .filter(Profile::UserId.eq(user.id))
    .first()
    .await?;
```

### Many-to-Many Relationships

Define M:N relationships using the `#[many_to_many]` decorator with a **through table** (join model):

```rust
use ormada::prelude::*;

pub mod models {
    pub mod tag {
        use ormada::prelude::*;

        #[ormada_model(table = "tags")]
        pub struct Tag {
            #[primary_key]
            pub id: i32,
            pub name: String,
        }
    }

    // Through table - must be defined BEFORE the model that uses #[many_to_many]
    pub mod book_tag {
        use ormada::prelude::*;

        #[ormada_model(table = "book_tags")]
        pub struct BookTag {
            #[primary_key]
            pub id: i32,
            #[foreign_key(super::book::Book)]
            pub book_id: i32,
            #[foreign_key(super::tag::Tag)]
            pub tag_id: i32,
        }
    }

    pub mod book {
        use ormada::prelude::*;
        use super::tag::Tag;
        use super::book_tag::BookTag;

        #[ormada_model(table = "books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            pub title: String,

            // M:N relationship - generates get_tags() method automatically!
            #[many_to_many(Tag, through = BookTag)]
            pub tags: Vec<i32>,
        }
    }
}

// Add tags to a book via the through table
BookTag::objects(&db)
    .create(BookTag { book_id: book.id, tag_id: tag.id, ..Default::default() })
    .await?;

// Use the generated get_tags() method from #[many_to_many] decorator!
let book = Book::objects(&db).get(book_id).await?;
let tags = book.get_tags(&db).await?;  // Auto-generated helper method!

for tag in &tags {
    println!("Tag: {}", tag.name);
}

// Alternative: Manual query with prefetch_related
let book_tags = BookTag::objects(&db)
    .filter(BookTag::BookId.eq(book.id))
    .prefetch_related(relations![Tag])
    .all()
    .await?;

for bt in &book_tags {
    println!("Tag: {}", bt.tag.name);
}
```

The `#[many_to_many(Model, through = JoinModel)]` decorator:
- Declares the M:N relationship on the model
- Generates a `get_{field_name}()` helper method (e.g., `get_tags()`)
- The field itself is **not** stored in the database - it's metadata only

## Transactions

### Using `tx!` Macro

```rust
use ormada::prelude::*;

let (author, book) = tx!(db, |txn| async move {
    let author = Author::objects(txn)
        .create(Author { name: "Alice".into(), ..Default::default() })
        .await?;
    
    let book = Book::objects(txn)
        .create(Book { 
            author_id: author.id,
            title: "My Book".into(),
            ..Default::default() 
        })
        .await?;
    
    Ok((author, book))
}).await?;
```

### Using `#[atomic]` Decorator

```rust
#[atomic(db)]
async fn create_author_with_book(
    db: &DatabaseRouter,
    name: String,
    title: String,
) -> Result<Book, OrmadaError> {
    let author = Author::objects(db)
        .create(Author { name, ..Default::default() })
        .await?;
    
    Book::objects(db)
        .create(Book { 
            author_id: author.id, 
            title,
            ..Default::default() 
        })
        .await
}
```

### Error Handling

```rust
let result = tx!(db, |txn| async move {
    let book = Book::objects(txn).create(book).await?;
    
    if book.price < 0 {
        return Err(OrmadaError::validation_error(
            "books", "price", "Price cannot be negative"
        ));
    }
    
    Ok(book)
}).await;

match result {
    Ok(book) => println!("Created: {}", book.title),
    Err(e) => println!("Rolled back: {}", e),
}
```

## Aggregations

```rust
// Count
let count = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .count()
    .await?;

// Sum
let total = Book::objects(&db)
    .aggregate_sum(Book::Price)
    .await?;

// Average
let avg_price = Book::objects(&db)
    .aggregate_avg(Book::Price)
    .await?;

// Min/Max
let cheapest = Book::objects(&db).aggregate_min(Book::Price).await?;
let most_expensive = Book::objects(&db).aggregate_max(Book::Price).await?;
```

### Group By with Projections

Type-safe aggregation queries with custom DTOs:

```rust
use ormada::prelude::*;
use sea_orm::FromQueryResult;

// Define a DTO for aggregation results
#[derive(Debug, Clone, FromQueryResult)]
pub struct AuthorStats {
    pub author_id: i32,
    pub book_count: i64,
    pub total_sales: i64,
    pub avg_price: f64,
}

// Group by author with multiple aggregations
let stats: Vec<AuthorStats> = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .group_by(Book::AuthorId)
    .annotate([
        ("book_count", Aggregation::count_all()),
        ("total_sales", Aggregation::sum(Book::Sales)),
        ("avg_price", Aggregation::avg(Book::Price)),
    ])
    .project::<AuthorStats>()
    .await?;

for stat in stats {
    println!("Author {}: {} books, ${:.2} avg", 
        stat.author_id, stat.book_count, stat.avg_price / 100.0);
}
```

## Soft Delete

```rust
use ormada::prelude::*;

#[ormada_model(table = "articles")]
pub struct Article {
    #[primary_key]
    pub id: i32,
    pub title: String,
    
    #[soft_delete]
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

// Soft delete (sets deleted_at timestamp)
Article::objects(&db)
    .filter(Article::Id.eq(1))
    .delete()
    .await?;

// Query excludes soft-deleted by default
Article::objects(&db).all().await?;

// Include soft-deleted records
Article::objects(&db).with_deleted().all().await?;

// Only soft-deleted records
Article::objects(&db).only_deleted().all().await?;

// Restore soft-deleted record
Article::objects(&db)
    .filter(Article::Id.eq(1))
    .restore()
    .await?;

// Permanently delete
Article::objects(&db)
    .filter(Article::Id.eq(1))
    .force_delete()
    .await?;
```

## Database Router

The `DatabaseRouter` provides intelligent routing between primary and replica databases.

### Single Database

```rust
let db = Database::connect("postgresql://localhost/mydb").await?;
let router = DatabaseRouter::new_single(db);
```

### Primary + Replica

```rust
let primary = Database::connect("postgresql://primary/mydb").await?;
let replica = Database::connect("postgresql://replica/mydb").await?;
let router = DatabaseRouter::new(primary, replica);

// Writes → Primary
Book::objects(&router).create(book).await?;

// Reads → Replica
Book::objects(&router).all().await?;
```

### Read-Your-Writes Consistency

After a write, subsequent reads automatically route to primary to ensure consistency:

```rust
let book = Book::objects(&router).create(book).await?;
let fetched = Book::objects(&router).get(book.id).await?;  // → Primary
```

## Lifecycle Hooks

### Auto-generated (Default)

By default, hooks do nothing:

```rust
#[ormada_model(table = "books")]
pub struct Book { /* fields */ }
// Hooks are auto-implemented as no-ops
```

### Custom Hooks

```rust
use ormada::prelude::*;
use async_trait::async_trait;  // Add async-trait = "0.1" to Cargo.toml

#[ormada_model(table = "books", hooks = true)]
pub struct Book { /* fields */ }

#[async_trait]
impl LifecycleHooks for book::Model {
    async fn before_create(&mut self) -> Result<(), OrmadaError> {
        // Validate, transform, etc.
        Ok(())
    }
    
    async fn after_create(&self) -> Result<(), OrmadaError> {
        // Send notifications, update cache, etc.
        Ok(())
    }
    
    async fn before_save(&mut self) -> Result<(), OrmadaError> {
        // Called before both create and update
        Ok(())
    }
    
    async fn after_save(&self) -> Result<(), OrmadaError> {
        // Called after both create and update
        Ok(())
    }
    
    async fn before_delete(&mut self) -> Result<(), OrmadaError> {
        Ok(())
    }
    
    async fn after_delete(&self) -> Result<(), OrmadaError> {
        Ok(())
    }
}
```

## Validation

### Built-in Validators

```rust
use ormada::prelude::*;

#[ormada_model(table = "users")]
pub struct User {
    #[primary_key]
    pub id: i32,
    
    #[max_length(100)]
    #[min_length(2)]
    pub name: String,
    
    #[range(min = 0, max = 150)]
    pub age: i32,
}
```

### FK Validation

Non-nullable foreign keys are validated at creation time:

```rust
// ❌ Fails with validation error
Book::objects(&db).create(Book {
    author_id: 0,  // Default value rejected
    ..Default::default()
}).await;

// Error: "foreign key cannot be the default value"
```

## Query Debugging

Debug and analyze your queries for performance optimization.

### Debug SQL

```rust
// See the generated SQL (pretty-printed by default)
let sql = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .order_by_desc(Book::CreatedAt)
    .debug_sql(true);  // true = pretty-print, false = single-line

println!("SQL:\n{}", sql);
// Output:
// SELECT
//   ...
// FROM
//   books
// WHERE
//   published = true
// ORDER BY
//   created_at DESC

// Compact single-line output
let sql_compact = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .debug_sql(false);
```

### Explain Query Plan

```rust
// Get query execution plan by running EXPLAIN (pretty-printed)
let plan = Book::objects(&db)
    .filter(Book::Price.lt(5000))
    .filter(Book::Published.eq(true))
    .explain(true)  // true = pretty-print, false = single-line
    .await?;

println!("{}", plan);
// Shows: Index Scan vs Sequential Scan, estimated rows, etc.
```

### Explain Analyze

```rust
// Run EXPLAIN ANALYZE and get actual execution statistics (pretty-printed)
// ⚠️ WARNING: Actually executes the query!
let analysis = Book::objects(&db)
    .filter(Book::AuthorId.eq(author_id))
    .explain_analyze(true)  // true = pretty-print, false = single-line
    .await?;

println!("{}", analysis);
// Shows: actual rows, execution time, buffer hits/misses
```

**Performance Tips from EXPLAIN:**

- **Sequential Scan** → Add an index
- **High actual rows** → Add pagination/limits
- **Slow sorts** → Index the ORDER BY columns
- **Many buffer reads** → Increase `shared_buffers` (PostgreSQL)

## Error Handling

Django-style error types for familiar pattern matching.

```rust
use ormada::prelude::*;

match Book::objects(&db).get(id).await {
    Ok(book) => println!("Found: {}", book.title),
    
    // Like Django's: except Book.DoesNotExist
    Err(OrmadaError::DoesNotExist { entity, id }) => {
        eprintln!("{entity} with id '{id}' not found");
    }
    
    // Like Django's: except IntegrityError
    Err(OrmadaError::IntegrityError(msg)) => {
        eprintln!("Constraint violation: {msg}");
    }
    
    // Like Django's: except ValidationError
    Err(OrmadaError::ValidationError { entity, field, reason }) => {
        eprintln!("Validation failed on {entity}.{field}: {reason}");
    }
    
    Err(e) => eprintln!("Error: {e}"),
}
```

**Available Error Types:**

| Error | Django Equivalent | Use Case |
|-------|-------------------|----------|
| `DoesNotExist` | `Model.DoesNotExist` | Record not found |
| `MultipleObjectsReturned` | `Model.MultipleObjectsReturned` | Expected one, got many |
| `IntegrityError` | `django.db.IntegrityError` | Constraint violations |
| `ValidationError` | `ValidationError` | Field validation failed |
| `OperationalError` | `OperationalError` | Connection issues |
| `ProgrammingError` | `ProgrammingError` | SQL syntax errors |

## Advanced Features

### Typestate Query Validation

Queries are validated at compile time using the typestate pattern:

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

### Query Caching

```rust
// Enable caching for a scope - subsequent identical queries return cached results
let cached_db = db.with_query_cache();

// First call - executes SQL
let books1 = Book::objects(&cached_db).all().await?;

// Second identical call - returns cached (no SQL)
let books2 = Book::objects(&cached_db).all().await?;

// Cache automatically cleared when cached_db goes out of scope
```

### Async Update Closures

```rust
// Update with async FK lookup inside the closure
let count = Book::objects(&db)
    .filter(Book::Id.eq(book_id))
    .update(|mut book| async move {
        // Async operations supported inside update!
        if let Some(author_name) = &update_dto.author_name {
            let (author, _) = Author::objects(&db)
                .filter(Author::Name.eq(author_name))
                .get_or_create(|| async {
                    Ok(Author { name: author_name.clone(), ..Default::default() })
                })
                .await?;
            book.author_id = author.id;
        }
        Ok(book)
    })
    .await?;
```

### Concurrency-Safe Updates

```rust
// Uses SELECT FOR UPDATE to prevent lost updates
let count = Book::objects(&db)
    .filter(Book::InStock.eq(true))
    .update(|mut book| async move {
        book.stock_count -= 1;  // Safe even with concurrent requests
        Ok(book)
    })
    .await?;
```

## Limitations

Be aware of these current limitations:

| Limitation | Description | Workaround |
|------------|-------------|------------|
| **String PKs** | String primary keys not yet supported for relation loading | Use `i32`, `i64`, or `Uuid` for PKs |
| **Composite FK loading** | Composite foreign key relation loading not implemented | Use single-column FKs or manual joins |
| **`bulk_create` return** | `bulk_create()` doesn't return inserted IDs | Fetch IDs separately if needed |
| **SQLite streaming** | SQLite doesn't support true streaming; uses chunked fetching | Use PostgreSQL for large datasets |
| **Query caching scope** | Cache is connection-scoped, not global | Use external cache (Redis) for distributed caching |

### Database Support

| Database | Status | Notes |
|----------|--------|-------|
| PostgreSQL | ✅ Full | Recommended for production |
| SQLite | ✅ Full | Great for development/testing |
| MySQL | 🔶 Partial | Some features may vary |

### Performance Considerations

- **`update()` with closures**: Fetches all matching rows before updating. For bulk updates of simple fields, consider raw SQL.
- **`values()` method**: Returns `Vec<serde_json::Value>` which has heap allocation overhead. For performance-critical code, use `project<T>()` or direct SeaORM queries.
- **Relation loading**: Uses standard `HashMap`. For millions of relations, consider custom loading.

### Benchmark Results

Measured on SQLite in-memory (M1 Mac, release build):

| Operation | 100 rows | 1,000 rows | 10,000 rows |
|-----------|----------|------------|-------------|
| `all()` | ~94 µs | ~764 µs | ~7.3 ms |
| `filter().all()` | ~95 µs | ~770 µs | ~3.7 ms |
| `count()` | ~33 µs | ~33 µs | ~33 µs |
| `iterator()` (chunked) | ~95 µs | ~770 µs | ~7.7 ms |
| `values()` (JSON) | ~190 µs | ~763 µs | ~683 µs |

Run benchmarks yourself: `cargo bench`

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
