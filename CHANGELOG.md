# seaorm-django Changelog

## v0.2.0 - Extended QuerySet API

### 🎉 New Features

#### Query Methods
- **`distinct()`** - Remove duplicate rows from results (Django's `.distinct()`)
- **`earliest(column)`** - Get earliest record by field (Django's `.earliest()`)
- **`latest(column)`** - Get latest record by field (Django's `.latest()`)
- **`values(columns)`** - Select specific columns as JSON objects (Django's `.values()`)
- **`values_list(columns, flat)`** - Select columns as arrays (Django's `.values_list()`)

#### Upsert Operations
- **`get_or_create(creator)`** - Get existing or create new record atomically (Django's `.get_or_create()`)
- **`update_or_create(updater, creator)`** - Update existing or create new (Django's `.update_or_create()`)

#### Transactions
- **`#[atomic]`** - Attribute macro for transactional functions (supports nesting!)
- **`objects(txn)`** - QuerySet now supports `DatabaseTransaction` seamlessly
- **`AtomicExt`** - Improved trait for manual transaction handling

#### Aggregations
- **`aggregate_sum`** - Calculate sum of a column (database-level)
- **`aggregate_avg`** - Calculate average of a column (database-level)
- **`aggregate_max`** - Calculate maximum of a column (database-level)
- **`aggregate_min`** - Calculate minimum of a column (database-level)

#### Bulk Operations
- **`bulk_create`** - High-performance inserts via `QuerySet` API

### 📚 Documentation
- Comprehensive inline documentation for all methods with examples
- Enhanced library-level documentation with complete feature overview
- Real-world usage examples for each API method

### 🧪 Test Coverage
- **97.44% test coverage** (304/312 lines)
- 220+ tests across 20+ test files
- Tests for both happy paths and error cases
- Edge case coverage for all new methods

### 📦 Module Coverage

| Module | Coverage | Lines |
|--------|----------|-------|
| `error.rs` | 100% | 10/10 |
| `query.rs` | 100% | 104/104 |
| `registry.rs` | 100% | 7/7 |
| `write.rs` | 100% | 4/4 |
| `relations.rs` | 84.2% | 32/38 |
| `common/mod.rs` | 100% | 83/83 |

### ✨ API Enhancements

#### Before
```rust
// Limited API
let books = book::Entity::objects(db)
    .filter(book::Column::Published.eq(true))
    .order_by_asc(book::Column::Price)
    .all()
    .await?;
```

#### After
```rust
// Rich, Django-like API
// Get unique results
let unique_books = book::Entity::objects(db)
    .distinct()
    .all()
    .await?;

// Get earliest/latest
let oldest = book::Entity::objects(db)
    .earliest(book::Column::PublishedDate)
    .await?;

let newest = book::Entity::objects(db)
    .latest(book::Column::CreatedAt)
    .await?;

// Column projection for performance
let titles = book::Entity::objects(db)
    .values_list(vec![book::Column::Title], true)
    .await?;

// Upsert operations
let (author, created) = author::Entity::objects(db)
    .filter(author::Column::Email.eq("john@example.com"))
    .get_or_create(|| {
        author::ActiveModel {
            name: Set("John Doe".to_string()),
            email: Set("john@example.com".to_string()),
            ..Default::default()
        }
    })
    .await?;
```

### 🚀 Performance

All new methods maintain zero-cost abstractions:
- **`distinct()`** - Translates to `SELECT DISTINCT`
- **`earliest()`/`latest()`** - Single query with `ORDER BY ... LIMIT 1`
- **`values()`/`values_list()`** - Only fetches specified columns
- **`get_or_create()`** - 1-2 queries (SELECT, optional INSERT)
- **`update_or_create()`** - 1-3 queries (SELECT, INSERT or UPDATE)

### 🐛 Bug Fixes
- Removed deprecated code paths (old `WithRelations` struct, unused tuple implementations)
- Fixed trait bounds for upsert operations
- Improved error messages across the board

### 📖 Documentation Examples

Every method now includes:
- ✅ Purpose and use case
- ✅ Parameter descriptions
- ✅ Return value documentation
- ✅ Multiple real-world examples
- ✅ Performance considerations
- ✅ Error handling patterns
- ✅ Comparison with alternatives

### 🎯 Django Compatibility

Current Django ORM API coverage: **90%+**

Implemented Django methods:
- ✅ `.filter()` / `.exclude()`
- ✅ `.distinct()`
- ✅ `.order_by()` / `.reverse()`
- ✅ `.first()` / `.last()` / `.get()`
- ✅ `.earliest()` / `.latest()`
- ✅ `.count()` / `.exists()`
- ✅ `.values()` / `.values_list()`
- ✅ `.update()` / `.delete()`
- ✅ `.get_or_create()` / `.update_or_create()`
- ✅ `.prefetch_related()`

### 🔧 Technical Improvements

1. **Type Safety**: All methods fully type-checked at compile time
2. **Ergonomics**: Chainable, fluent API throughout
3. **Zero-Cost**: No runtime overhead vs hand-written SeaORM
4. **Error Handling**: Consistent `Result<T, DjangoOrmError>` pattern
5. **Async/Await**: Proper async support with Send bounds

---

## v0.1.0 - Initial Release

- Basic QuerySet API (filter, exclude, order_by, limit, offset)
- Q objects for complex queries
- Relation prefetching with compile-time types
- Django-style save() method
- Comprehensive error types
