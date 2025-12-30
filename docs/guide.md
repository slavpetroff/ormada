# Ormada User Guide

This guide provides comprehensive documentation for using Ormada ORM.

## Table of Contents

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
- [Migrations](#migrations)

---

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
| `#[primary_key(auto_increment = false)]` | Non-auto-increment PK | `pub id: Uuid` |
| `#[foreign_key(Model)]` | Many-to-One relationship | `pub author_id: i32` |
| `#[foreign_key(Model, on_delete = SetNull)]` | FK with SET NULL | `pub category_id: Option<i32>` |
| `#[one_to_one(Model)]` | One-to-One relationship | `pub profile_id: i32` |
| `#[many_to_many(Model, through = JoinModel)]` | Many-to-Many | See M:N section |
| `#[max_length(n)]` | String max length validation | `pub title: String` |
| `#[min_length(n)]` | String min length validation | `pub code: String` |
| `#[range(min = a, max = b)]` | Numeric range validation | `pub age: i32` |
| `#[unique]` | Unique constraint | `pub email: String` |
| `#[index]` | Database index | `pub created_at: DateTimeWithTimeZone` |
| `#[auto_now]` | Update timestamp on every save | `pub updated_at: DateTimeWithTimeZone` |
| `#[auto_now_add]` | Set timestamp on creation only | `pub created_at: DateTimeWithTimeZone` |
| `#[soft_delete]` | Enable soft delete | `pub deleted_at: Option<DateTimeWithTimeZone>` |

### Model Organization (Required for Foreign Keys)

When using `#[foreign_key]`, models must be in separate submodules:

```rust
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
        use super::author::Author;

        #[ormada_model(table = "books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author)]
            pub author_id: i32,
            pub title: String,
        }
    }
}

pub use models::author::Author;
pub use models::book::Book;
```

### Supported Primary Key Types

```rust
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

---

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
Book::objects(&db).order_by_asc(Book::Title).all().await?;
Book::objects(&db).order_by_desc(Book::CreatedAt).all().await?;

// Multiple orderings
Book::objects(&db)
    .order_by_desc(Book::Featured)
    .order_by_asc(Book::Title)
    .all().await?;
```

### Pagination

```rust
Book::objects(&db).limit(10).offset(20).all().await?;
```

### Retrieval Methods

```rust
let books: Vec<Book> = Book::objects(&db).all().await?;
let book: Book = Book::objects(&db).first().await?;
let book: Book = Book::objects(&db).get(42).await?;
let exists: bool = Book::objects(&db).filter(Book::Isbn.eq("978-0134685991")).exists().await?;
let count: u64 = Book::objects(&db).filter(Book::Published.eq(true)).count().await?;
let oldest = Book::objects(&db).earliest(Book::CreatedAt).await?;
let newest = Book::objects(&db).latest(Book::CreatedAt).await?;
```

---

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
Book::objects(&db)
    .filter(Book::Published.eq(false))
    .delete()
    .await?;
```

---

## Upsert Operations

Race-condition safe operations with automatic retry.

### Get or Create

```rust
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
}
```

### Update or Create

```rust
let (book, created) = Book::objects(&db)
    .filter(Book::Isbn.eq("978-1234567890"))
    .update_or_create(
        |mut book| async move {
            book.price = 2999;
            Ok(book)
        },
        || async {
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

---

## Bulk Operations

**10-100x faster** than individual inserts.

> ⚠️ Bulk operations do NOT trigger lifecycle hooks.

### Bulk Create

```rust
let authors: Vec<Author> = (0..1000)
    .map(|i| Author {
        name: format!("Author {i}"),
        email: format!("author{i}@example.com"),
        ..Default::default()
    })
    .collect();

let count = Author::objects(&db).bulk_create(authors).await?;
```

### Bulk Upsert

```rust
Book::objects(&db)
    .upsert_many(books)
    .on_conflict(Book::Isbn)
    .update_fields(&[Book::Title, Book::Price])
    .execute()
    .await?;
```

---

## Streaming & Iterators

Process millions of rows without loading everything into memory.

### Stream Full Models

```rust
use futures::StreamExt;

let mut stream = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .iterator(Some(100))  // Chunk size
    .await?;

while let Some(result) = stream.next().await {
    let book = result?;
    process_book(&book).await?;
}
```

### Type-Safe Projections

```rust
#[derive(Debug, Clone, FromQueryResult)]
pub struct BookSummary {
    pub title: String,
    pub price: i32,
}

let summaries: Vec<BookSummary> = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .project::<BookSummary>()
    .await?;
```

---

## Relations

Ormada provides three types of relations:

| Relation Type | Declaration | Direction | Example |
|---------------|-------------|-----------|---------|
| **Forward (FK)** | `#[foreign_key(Model)]` | Child → Parent | Book → Author |
| **Reverse (1:N)** | Auto-inferred from FK | Parent → Children | Author → Books |
| **Many-to-Many** | `#[many_to_many(Model, through = JoinModel)]` | Both ways | Book ↔ Tags |

### Forward Relations (Foreign Key)

When a model has a foreign key, you can load the related parent model:

```rust
// Book has #[foreign_key(Author)]
let books = Book::objects(&db)
    .select_related(relations![Author])  // JOIN-based, single query
    .all()
    .await?;

for book in &books {
    println!("{} by {}", book.title, book.author.name);
}

// Or use prefetch_related for separate queries
let books = Book::objects(&db)
    .prefetch_related(relations![Author])
    .all()
    .await?;
```

### Reverse Relations (One-to-Many)

When `Book` declares `#[foreign_key(Author)]`, Ormada automatically generates:

- `Author::Model::get_books(&db)` - async method to load books
- `Author::ModelWithRelations::get_books(&db)` - returns prefetched data if available

```rust
// Single author - lazy load
let author = Author::objects(&db).first().await?;
let books = author.get_books(&db).await?;

// Multiple authors - efficient batch loading (2 queries total)
let authors = Author::objects(&db)
    .prefetch_related(reverse_relations![Book])
    .all()
    .await?;

for author in &authors {
    // Returns prefetched data, no additional DB query
    let books = author.get_books(&db).await?;
    println!("{} wrote {} books", author.name, books.len());
}
```

### Eager Loading Methods

| Method | Query Pattern | Best For |
|--------|---------------|----------|
| `select_related` | Single JOIN query | Forward FK relations (many-to-one) |
| `prefetch_related` | Separate queries | Reverse relations (one-to-many), M:N |

```rust
// select_related - Uses SQL JOIN (forward relations only)
let books = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .select_related(relations![Author])
    .all()
    .await?;

// prefetch_related - Separate queries (works for all relation types)
// Forward relation
let books = Book::objects(&db)
    .prefetch_related(relations![Author])
    .all()
    .await?;

// Reverse relation
let authors = Author::objects(&db)
    .prefetch_related(reverse_relations![Book])
    .all()
    .await?;
```

### Combining Multiple Relations

Use tuple syntax to load multiple relations in a single `prefetch_related` call:

```rust
// Multiple forward relations
let books = Book::objects(&db)
    .prefetch_related(relations![Author, Publisher])
    .all()
    .await?;

// Multiple reverse relations (Author has Books and Articles)
let authors = Author::objects(&db)
    .prefetch_related((reverse_relations![Book], reverse_relations![Article]))
    .all()
    .await?;

for author in &authors {
    let books = author.get_books(&db).await?;
    let articles = author.get_articles(&db).await?;
    println!("{} wrote {} books and {} articles", 
             author.name, books.len(), articles.len());
}

// Mixed: forward + reverse relations
let books = Book::objects(&db)
    .prefetch_related((relations![Author], reverse_relations![Review]))
    .all()
    .await?;
```

### Many-to-Many

```rust
#[ormada_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,

    #[many_to_many(Tag, through = BookTag)]
    pub tags: Vec<i32>,
}

// Use generated helper method
let book = Book::objects(&db).get(book_id).await?;
let tags = book.get_tags(&db).await?;
```

### Generated Methods Summary

When you define relations, Ormada generates these methods:

| Declaration | Generated On | Method | Returns |
|-------------|--------------|--------|---------|
| `#[foreign_key(Author)]` on Book | `Author::Model` | `get_books(&db)` | `Result<Vec<Book>>` |
| `#[foreign_key(Author)]` on Book | `Author::ModelWithRelations` | `get_books(&db)` | `Result<Vec<Book>>` (prefetched) |
| `#[many_to_many(Tag, through = BookTag)]` | `Book::Model` | `get_tags(&db)` | `Result<Vec<Tag>>` |

---

## Transactions

### Using `tx!` Macro

```rust
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

---

## Aggregations

```rust
let count = Book::objects(&db).filter(Book::Published.eq(true)).count().await?;
let total = Book::objects(&db).aggregate_sum(Book::Price).await?;
let avg_price = Book::objects(&db).aggregate_avg(Book::Price).await?;
let cheapest = Book::objects(&db).aggregate_min(Book::Price).await?;
let most_expensive = Book::objects(&db).aggregate_max(Book::Price).await?;
```

### Group By with Projections

```rust
#[derive(Debug, Clone, FromQueryResult)]
pub struct AuthorStats {
    pub author_id: i32,
    pub book_count: i64,
    pub avg_price: f64,
}

let stats: Vec<AuthorStats> = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .group_by(Book::AuthorId)
    .annotate([
        ("book_count", Aggregation::count_all()),
        ("avg_price", Aggregation::avg(Book::Price)),
    ])
    .project::<AuthorStats>()
    .await?;
```

---

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

// Soft delete
Article::objects(&db).filter(Article::Id.eq(1)).delete().await?;

// Query excludes soft-deleted by default
Article::objects(&db).all().await?;

// Include soft-deleted
Article::objects(&db).with_deleted().all().await?;

// Only soft-deleted
Article::objects(&db).only_deleted().all().await?;

// Restore
Article::objects(&db).filter(Article::Id.eq(1)).restore().await?;

// Permanently delete
Article::objects(&db).filter(Article::Id.eq(1)).force_delete().await?;
```

---

## Query Debugging

```rust
// Debug SQL
let sql = Book::objects(&db)
    .filter(Book::Published.eq(true))
    .debug_sql(true);  // true = pretty-print

// Explain query plan
let plan = Book::objects(&db)
    .filter(Book::Price.lt(5000))
    .explain(true)
    .await?;

// Explain analyze (actually executes!)
let analysis = Book::objects(&db)
    .filter(Book::AuthorId.eq(author_id))
    .explain_analyze(true)
    .await?;
```

---

## Database Router

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

// Writes → Primary, Reads → Replica
// Read-your-writes consistency is automatic
```

---

## Lifecycle Hooks

```rust
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
    
    async fn before_save(&mut self) -> Result<(), OrmadaError> { Ok(()) }
    async fn after_save(&self) -> Result<(), OrmadaError> { Ok(()) }
    async fn before_delete(&mut self) -> Result<(), OrmadaError> { Ok(()) }
    async fn after_delete(&self) -> Result<(), OrmadaError> { Ok(()) }
}
```

---

## Validation

### Built-in Validators

```rust
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
// ❌ Fails: author_id would be 0
Book::objects(&db).create(Book { author_id: 0, ..Default::default() }).await;

// ✅ Correct: explicitly set author_id
Book::objects(&db).create(Book { author_id: author.id, ..Default::default() }).await;
```

---

## Error Handling

Django-style error types:

```rust
match Book::objects(&db).get(id).await {
    Ok(book) => println!("Found: {}", book.title),
    Err(OrmadaError::DoesNotExist { entity, id }) => {
        eprintln!("{entity} with id '{id}' not found");
    }
    Err(OrmadaError::IntegrityError(msg)) => {
        eprintln!("Constraint violation: {msg}");
    }
    Err(OrmadaError::ValidationError { entity, field, reason }) => {
        eprintln!("Validation failed on {entity}.{field}: {reason}");
    }
    Err(e) => eprintln!("Error: {e}"),
}
```

| Error | Django Equivalent | Use Case |
|-------|-------------------|----------|
| `DoesNotExist` | `Model.DoesNotExist` | Record not found |
| `MultipleObjectsReturned` | `Model.MultipleObjectsReturned` | Expected one, got many |
| `IntegrityError` | `IntegrityError` | Constraint violations |
| `ValidationError` | `ValidationError` | Field validation failed |
| `OperationalError` | `OperationalError` | Connection issues |
| `ProgrammingError` | `ProgrammingError` | SQL syntax errors |

---

## Migrations

Declarative migration system using the same `#[ormada_model]` syntax.

### Quick Start

```bash
ormada migrate init
ormada migrate make "add books table"
ormada migrate run
ormada migrate status
```

### Migration Files

```rust
// migrations/m001_initial.rs
use ormada::prelude::*;

#[ormada_schema(table = "authors", migration = "m001_initial")]
pub struct Author {
    #[primary_key]
    pub id: i32,
    
    #[max_length(100)]
    pub name: String,
}
```

### Delta Migrations

```rust
// migrations/m002_add_isbn.rs
#[ormada_schema(
    table = "books",
    migration = "m002_add_isbn",
    after = "m001_initial",
    extends = Book
)]
pub struct Book {
    #[index]
    pub isbn: String,
}
```

### Data Migrations

```rust
#[ormada_data_migration(migration = "m004", after = "m003")]
async fn populate_emails(db: &DatabaseConnection) -> Result<(), OrmadaError> {
    Author::objects(db)
        .filter(Author::Email.is_null())
        .update_all(|author| {
            author.email = format!("{}@example.com", author.full_name);
        })
        .await?;
    Ok(())
}
```
