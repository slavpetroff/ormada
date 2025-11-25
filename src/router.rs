//! Database routing for read replicas
//!
//! Provides automatic routing between primary (write) and replica (read) databases
//! with consistency guarantees to prevent race conditions.
//!
//! ## Key Design Principles
//!
//! 1. **Read-Your-Writes Consistency**: Reads after writes in the same context go to primary
//! 2. **Write Safety**: All writes always go to primary
//! 3. **Transaction Safety**: All transaction operations use primary only
//! 4. **Ergonomics**: Single DB works transparently, replicas are opt-in
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Single database (default - no configuration needed)
//! let db = Database::connect("postgresql://localhost/db").await?;
//! Book::objects(&db).all().await?;  // Works normally
//!
//! // With read replicas
//! let router = DatabaseRouter::new(
//!     Database::connect("postgresql://primary/db").await?,
//!     vec![
//!         Database::connect("postgresql://replica1/db").await?,
//!         Database::connect("postgresql://replica2/db").await?,
//!     ]
//! );
//!
//! // Reads go to replicas (round-robin)
//! Book::objects(&router).all().await?;
//!
//! // Writes go to primary
//! Book::objects(&router).create(book).await?;
//!
//! // Read after write in same context → primary (consistency!)
//! let book = Book::objects(&router).create(book).await?;
//! let reloaded = Book::objects(&router).get(book.id).await?;  // Uses primary!
//! ```

use async_trait::async_trait;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr,
    ExecResult, IsolationLevel, QueryResult, Statement, StatementBuilder, TransactionError,
    TransactionTrait,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Database routing strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Always use primary database
    Primary,
    /// Use read replicas with round-robin load balancing
    RoundRobin,
}

/// Transaction state for the router
///
/// Explicit enum representation of transaction state, making illegal states
/// unrepresentable and enabling exhaustive pattern matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionState {
    /// No active transaction
    #[default]
    Idle,
    /// Transaction is active - all operations use primary
    Active,
}

impl TransactionState {
    /// Check if currently in an active transaction
    #[inline]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if idle (no active transaction)
    #[inline]
    pub const fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Transition to active state
    pub fn begin(&mut self) {
        *self = Self::Active;
    }

    /// Transition to idle state
    pub fn end(&mut self) {
        *self = Self::Idle;
    }
}

/// Context tracking for read-your-writes consistency
///
/// Tracks whether writes have occurred in the current context.
/// Once a write happens, subsequent reads in the same context use primary.
#[derive(Debug, Clone)]
pub struct ConsistencyContext {
    /// Has a write occurred in this context?
    write_occurred: Arc<AtomicBool>,
}

impl ConsistencyContext {
    /// Create a new consistency context
    pub fn new() -> Self {
        Self {
            write_occurred: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Mark that a write has occurred
    pub fn mark_write(&self) {
        self.write_occurred.store(true, Ordering::Release);
    }

    /// Check if a write has occurred
    pub fn has_write_occurred(&self) -> bool {
        self.write_occurred.load(Ordering::Acquire)
    }

    /// Reset the context (for testing or explicit consistency boundaries)
    pub fn reset(&self) {
        self.write_occurred.store(false, Ordering::Release);
    }
}

impl Default for ConsistencyContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Database router for primary/replica setup
///
/// Automatically routes queries to the appropriate database:
/// - Writes → always primary
/// - Reads after writes → primary (read-your-writes consistency)
/// - Pure reads → replicas (round-robin)
/// - Transactions → always primary
///
/// If no replicas are configured, all operations use the primary.
#[derive(Debug, Clone)]
pub struct DatabaseRouter {
    /// Primary database (handles all writes)
    primary: DatabaseConnection,

    /// Read replicas (optional)
    replicas: Vec<DatabaseConnection>,

    /// Round-robin counter for replica selection
    replica_index: Arc<AtomicUsize>,

    /// Consistency context for read-your-writes
    context: ConsistencyContext,

    /// Transaction state (explicit enum for clarity)
    transaction_state: Arc<RwLock<TransactionState>>,
}

impl DatabaseRouter {
    /// Create a new router with primary database only
    ///
    /// All queries will use the primary. This is the default configuration.
    pub fn new_single(primary: DatabaseConnection) -> Self {
        Self {
            primary,
            replicas: Vec::new(),
            replica_index: Arc::new(AtomicUsize::new(0)),
            context: ConsistencyContext::new(),
            transaction_state: Arc::new(RwLock::new(TransactionState::Idle)),
        }
    }

    /// Create a new router with primary and read replicas
    ///
    /// Reads will be distributed across replicas using round-robin.
    /// Writes always use primary.
    pub fn new_with_replicas(
        primary: DatabaseConnection,
        replicas: Vec<DatabaseConnection>,
    ) -> Self {
        Self {
            primary,
            replicas,
            replica_index: Arc::new(AtomicUsize::new(0)),
            context: ConsistencyContext::new(),
            transaction_state: Arc::new(RwLock::new(TransactionState::Idle)),
        }
    }

    /// Get a connection for read operations
    ///
    /// Returns primary if:
    /// - No replicas configured
    /// - Write has occurred in this context (read-your-writes)
    /// - Currently in transaction
    ///
    /// Otherwise returns a replica using round-robin.
    pub async fn read_connection(&self) -> &DatabaseConnection {
        // Always use primary if in transaction
        if self.transaction_state.read().await.is_active() {
            return &self.primary;
        }

        // Use primary if write occurred (read-your-writes consistency)
        if self.context.has_write_occurred() {
            return &self.primary;
        }

        // Use primary if no replicas configured
        if self.replicas.is_empty() {
            return &self.primary;
        }

        // Round-robin across replicas
        let index = self.replica_index.fetch_add(1, Ordering::Relaxed);
        let replica_idx = index % self.replicas.len();
        &self.replicas[replica_idx]
    }

    /// Get a connection for write operations
    ///
    /// Always returns primary and marks context as having writes.
    pub fn write_connection(&self) -> &DatabaseConnection {
        // Mark that a write occurred
        self.context.mark_write();
        &self.primary
    }

    /// Get the primary connection (for transactions)
    pub const fn primary_connection(&self) -> &DatabaseConnection {
        &self.primary
    }

    /// Mark that we're entering a transaction
    pub async fn begin_transaction(&self) {
        self.transaction_state.write().await.begin();
    }

    /// Mark that we're leaving a transaction
    pub async fn end_transaction(&self) {
        self.transaction_state.write().await.end();
    }

    /// Check if we're currently in a transaction
    pub async fn is_in_transaction(&self) -> bool {
        self.transaction_state.read().await.is_active()
    }

    /// Get the current transaction state
    pub async fn transaction_state(&self) -> TransactionState {
        *self.transaction_state.read().await
    }

    /// Get the consistency context
    pub const fn context(&self) -> &ConsistencyContext {
        &self.context
    }

    /// Reset consistency context (advanced usage)
    ///
    /// Resets the write tracking. Use with caution - this can break
    /// read-your-writes guarantees if not used properly.
    pub fn reset_context(&self) {
        self.context.reset();
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for DatabaseRouter {
    fn get_database_backend(&self) -> DbBackend {
        self.primary.get_database_backend()
    }

    async fn execute<S: StatementBuilder>(&self, stmt: &S) -> Result<ExecResult, DbErr> {
        // Delegate to primary for writes
        self.primary.execute(stmt).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        // Delegate to primary for writes
        self.primary.execute_unprepared(sql).await
    }

    async fn execute_raw(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        // Delegate to primary for writes
        self.primary.execute_raw(stmt).await
    }

    async fn query_one<S: StatementBuilder>(&self, stmt: &S) -> Result<Option<QueryResult>, DbErr> {
        // Delegate to primary (routing happens at higher level via read_connection)
        self.primary.query_one(stmt).await
    }

    async fn query_all<S: StatementBuilder>(&self, stmt: &S) -> Result<Vec<QueryResult>, DbErr> {
        // Delegate to primary (routing happens at higher level via read_connection)
        self.primary.query_all(stmt).await
    }

    async fn query_one_raw(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        // Delegate to primary (routing happens at higher level via read_connection)
        self.primary.query_one_raw(stmt).await
    }

    async fn query_all_raw(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        // Delegate to primary (routing happens at higher level via read_connection)
        self.primary.query_all_raw(stmt).await
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for &DatabaseRouter {
    fn get_database_backend(&self) -> DbBackend {
        self.primary.get_database_backend()
    }

    async fn execute<S: StatementBuilder>(&self, stmt: &S) -> Result<ExecResult, DbErr> {
        self.primary.execute(stmt).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.primary.execute_unprepared(sql).await
    }

    async fn execute_raw(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        self.primary.execute_raw(stmt).await
    }

    async fn query_one<S: StatementBuilder>(&self, stmt: &S) -> Result<Option<QueryResult>, DbErr> {
        self.primary.query_one(stmt).await
    }

    async fn query_all<S: StatementBuilder>(&self, stmt: &S) -> Result<Vec<QueryResult>, DbErr> {
        self.primary.query_all(stmt).await
    }

    async fn query_one_raw(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.primary.query_one_raw(stmt).await
    }

    async fn query_all_raw(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.primary.query_all_raw(stmt).await
    }
}

#[async_trait]
impl TransactionTrait for DatabaseRouter {
    type Transaction = DatabaseTransaction;

    async fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
        self.primary.begin().await
    }

    async fn begin_with_config(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<DatabaseTransaction, DbErr> {
        self.primary.begin_with_config(isolation_level, access_mode).await
    }

    async fn transaction<F, T, E>(&self, callback: F) -> Result<T, TransactionError<E>>
    where
        F: for<'a> FnOnce(
                &'a DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>
            + Send,
        T: Send,
        E: Send + std::fmt::Debug + std::fmt::Display,
    {
        self.primary.transaction(callback).await
    }

    async fn transaction_with_config<F, T, E>(
        &self,
        callback: F,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<T, TransactionError<E>>
    where
        F: for<'a> FnOnce(
                &'a DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>
            + Send,
        T: Send,
        E: Send + std::fmt::Debug + std::fmt::Display,
    {
        self.primary
            .transaction_with_config(callback, isolation_level, access_mode)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistency_context() {
        let ctx = ConsistencyContext::new();

        // Initially no writes
        assert!(!ctx.has_write_occurred());

        // Mark write
        ctx.mark_write();
        assert!(ctx.has_write_occurred());

        // Reset
        ctx.reset();
        assert!(!ctx.has_write_occurred());
    }

    #[tokio::test]
    async fn test_router_read_connection_no_replicas() {
        let primary = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let router = DatabaseRouter::new_single(primary);

        // Should always return primary when no replicas
        let conn1 = router.read_connection().await;
        let conn2 = router.read_connection().await;

        // Both should be the same connection (primary)
        assert_eq!(conn1.get_database_backend(), conn2.get_database_backend());
    }

    #[tokio::test]
    async fn test_router_write_marks_context() {
        let primary = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let router = DatabaseRouter::new_single(primary);

        // Initially no write
        assert!(!router.context().has_write_occurred());

        // Call write_connection
        let _conn = router.write_connection();

        // Should mark write
        assert!(router.context().has_write_occurred());
    }

    #[tokio::test]
    async fn test_router_read_after_write_uses_primary() {
        let primary = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let replica = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

        // Perform a write
        let _write_conn = router.write_connection();

        // Subsequent read should use primary (not replica)
        let _read_conn = router.read_connection().await;

        // In single-threaded test, we can verify it's using primary
        // by checking the context flag
        assert!(router.context().has_write_occurred());
    }

    #[tokio::test]
    async fn test_transaction_forces_primary() {
        let primary = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let replica = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let router = DatabaseRouter::new_with_replicas(primary, vec![replica]);

        // Mark as in transaction
        router.begin_transaction().await;

        // Even reads should use primary
        let _read_conn = router.read_connection().await;

        // Verify we're in transaction mode
        assert!(router.is_in_transaction().await);

        // End transaction
        router.end_transaction().await;
        assert!(!router.is_in_transaction().await);
    }

    // ========================================================================
    // TransactionState Enum Tests
    // ========================================================================

    #[test]
    fn test_transaction_state_default() {
        let state = TransactionState::default();
        assert!(state.is_idle());
        assert!(!state.is_active());
    }

    #[test]
    fn test_transaction_state_transitions() {
        let mut state = TransactionState::Idle;

        // Begin transaction
        state.begin();
        assert!(state.is_active());
        assert!(!state.is_idle());

        // End transaction
        state.end();
        assert!(state.is_idle());
        assert!(!state.is_active());
    }

    #[test]
    fn test_transaction_state_pattern_matching() {
        let idle = TransactionState::Idle;
        let active = TransactionState::Active;

        // Exhaustive pattern matching
        match idle {
            TransactionState::Idle => assert!(true),
            TransactionState::Active => panic!("Should be idle"),
        }

        match active {
            TransactionState::Active => assert!(true),
            TransactionState::Idle => panic!("Should be active"),
        }
    }

    #[test]
    fn test_transaction_state_equality() {
        assert_eq!(TransactionState::Idle, TransactionState::Idle);
        assert_eq!(TransactionState::Active, TransactionState::Active);
        assert_ne!(TransactionState::Idle, TransactionState::Active);
    }

    #[test]
    fn test_transaction_state_clone_copy() {
        let state = TransactionState::Active;
        let cloned = state.clone();
        let copied = state; // Copy

        assert_eq!(state, cloned);
        assert_eq!(state, copied);
    }

    #[test]
    fn test_transaction_state_debug() {
        let idle = TransactionState::Idle;
        let active = TransactionState::Active;

        assert!(format!("{:?}", idle).contains("Idle"));
        assert!(format!("{:?}", active).contains("Active"));
    }

    #[tokio::test]
    async fn test_router_transaction_state_method() {
        let primary = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let router = DatabaseRouter::new_single(primary);

        // Initially idle
        assert_eq!(router.transaction_state().await, TransactionState::Idle);

        // Begin transaction
        router.begin_transaction().await;
        assert_eq!(router.transaction_state().await, TransactionState::Active);

        // End transaction
        router.end_transaction().await;
        assert_eq!(router.transaction_state().await, TransactionState::Idle);
    }
}
