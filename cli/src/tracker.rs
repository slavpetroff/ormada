//! Migration tracking - manages applied migrations in the database
//!
//! Uses an internal `_ormada_migrations` table to track:
//! - Migration version/ID
//! - Migration name
//! - Checksum (to detect tampering)
//! - Applied timestamp

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

/// Represents an applied migration record
#[derive(Debug, Clone)]
pub struct AppliedMigration {
    /// Migration version/ID (e.g., "m20231222_100000_initial")
    pub version: String,
    /// Human-readable name
    pub name: String,
    /// SHA256 checksum of migration content
    pub checksum: String,
    /// When the migration was applied
    pub applied_at: DateTime<Utc>,
}

/// Represents a pending migration to be applied
#[derive(Debug, Clone)]
pub struct PendingMigration {
    /// Migration version/ID
    pub version: String,
    /// Human-readable name
    pub name: String,
    /// SQL content to execute
    pub sql: String,
    /// SHA256 checksum
    pub checksum: String,
}

impl PendingMigration {
    /// Create a new pending migration
    pub fn new(version: &str, name: &str, sql: &str) -> Self {
        let checksum = compute_checksum(sql);
        Self {
            version: version.to_string(),
            name: name.to_string(),
            sql: sql.to_string(),
            checksum,
        }
    }
}

/// Migration report - similar to refinery's Report
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    /// Migrations that were applied
    pub applied: Vec<AppliedMigration>,
    /// Migrations that were skipped (already applied)
    pub skipped: Vec<String>,
    /// Migrations that failed
    pub failed: Option<(String, String)>,
    /// Whether this was a dry run
    pub dry_run: bool,
}

impl MigrationReport {
    /// Create a new empty report
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run, ..Default::default() }
    }

    /// Check if any migrations were applied
    pub fn has_changes(&self) -> bool {
        !self.applied.is_empty()
    }

    /// Get total number of applied migrations
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    /// Format report for display
    pub fn format(&self) -> String {
        use colored::Colorize;

        let mut output = String::new();

        if self.dry_run {
            output.push_str(&format!("{}\n\n", "DRY RUN - No changes applied".yellow().bold()));
        }

        if self.applied.is_empty() && self.skipped.is_empty() {
            output.push_str(&format!("{} No migrations to apply\n", "✓".green()));
            return output;
        }

        if !self.skipped.is_empty() {
            output.push_str(&format!("{} Skipped (already applied):\n", "→".blue()));
            for version in &self.skipped {
                output.push_str(&format!("  {} {}\n", "○".dimmed(), version));
            }
            output.push('\n');
        }

        if !self.applied.is_empty() {
            let verb = if self.dry_run { "Would apply" } else { "Applied" };
            output.push_str(&format!("{} {}:\n", "✓".green(), verb));
            for migration in &self.applied {
                output.push_str(&format!(
                    "  {} {} ({})\n",
                    "●".green(),
                    migration.version,
                    migration.name
                ));
            }
        }

        if let Some((version, error)) = &self.failed {
            output.push_str(&format!("\n{} Failed: {}\n", "✗".red(), version));
            output.push_str(&format!("  Error: {}\n", error.red()));
        }

        output
    }

    /// Format SQL preview for dry run
    pub fn format_sql_preview(&self, pending: &[PendingMigration]) -> String {
        use colored::Colorize;

        let mut output = String::new();
        output.push_str(&format!("{}\n\n", "SQL Preview (Dry Run)".cyan().bold()));

        for migration in pending {
            output.push_str(&format!(
                "-- Migration: {} ({})\n",
                migration.version.green(),
                migration.name
            ));
            output.push_str(&format!("-- Checksum: {}\n", migration.checksum.dimmed()));
            output.push_str("-- ----------------------------------------\n");
            output.push_str(&migration.sql);
            output.push_str("\n\n");
        }

        output
    }
}

/// Compute SHA256 checksum of content
pub fn compute_checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// SQL for creating the migrations tracking table
pub const CREATE_MIGRATIONS_TABLE_POSTGRES: &str = r#"
CREATE TABLE IF NOT EXISTS _ormada_migrations (
    version TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
"#;

pub const CREATE_MIGRATIONS_TABLE_SQLITE: &str = r#"
CREATE TABLE IF NOT EXISTS _ormada_migrations (
    version TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// Migration runner - connects to DB and applies migrations
pub struct MigrationRunner {
    pool: PgPool,
}

impl MigrationRunner {
    /// Create a new migration runner with database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("Failed to connect to database")?;

        Ok(Self { pool })
    }

    /// Ensure the migrations tracking table exists
    pub async fn ensure_tracking_table(&self) -> Result<()> {
        sqlx::query(CREATE_MIGRATIONS_TABLE_POSTGRES)
            .execute(&self.pool)
            .await
            .context("Failed to create migrations tracking table")?;
        Ok(())
    }

    /// Get all applied migrations from the database
    pub async fn get_applied_migrations(&self) -> Result<Vec<AppliedMigration>> {
        let rows = sqlx::query(
            "SELECT version, name, checksum, applied_at FROM _ormada_migrations ORDER BY version",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch applied migrations")?;

        let migrations = rows
            .into_iter()
            .map(|row| AppliedMigration {
                version: row.get("version"),
                name: row.get("name"),
                checksum: row.get("checksum"),
                applied_at: row.get("applied_at"),
            })
            .collect();

        Ok(migrations)
    }

    /// Check if a specific migration has been applied
    pub async fn is_migration_applied(&self, version: &str) -> Result<bool> {
        let result: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM _ormada_migrations WHERE version = $1")
                .bind(version)
                .fetch_optional(&self.pool)
                .await
                .context("Failed to check migration status")?;

        Ok(result.is_some())
    }

    /// Get pending migrations (not yet applied)
    pub async fn get_pending_migrations(
        &self,
        all_migrations: &[PendingMigration],
    ) -> Result<Vec<PendingMigration>> {
        let applied = self.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<_> =
            applied.iter().map(|m| m.version.as_str()).collect();

        let pending: Vec<_> = all_migrations
            .iter()
            .filter(|m| !applied_versions.contains(m.version.as_str()))
            .cloned()
            .collect();

        Ok(pending)
    }

    /// Apply a single migration
    pub async fn apply_migration(&self, migration: &PendingMigration) -> Result<AppliedMigration> {
        // Start a transaction
        let mut tx = self.pool.begin().await?;

        // Split SQL into individual statements and execute each
        // PostgreSQL prepared statements don't support multiple commands
        for statement in split_sql_statements(&migration.sql) {
            let trimmed = statement.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            if let Err(e) = sqlx::query(trimmed).execute(&mut *tx).await {
                tx.rollback().await?;
                return Err(anyhow::anyhow!(
                    "Failed to execute migration {}: {}",
                    migration.version,
                    e
                ));
            }
        }

        // Record the migration
        let applied_at: DateTime<Utc> = Utc::now();
        sqlx::query(
            "INSERT INTO _ormada_migrations (version, name, checksum, applied_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(&migration.version)
        .bind(&migration.name)
        .bind(&migration.checksum)
        .bind(applied_at)
        .execute(&mut *tx)
        .await
        .context("Failed to record migration")?;

        // Commit
        tx.commit().await?;

        Ok(AppliedMigration {
            version: migration.version.clone(),
            name: migration.name.clone(),
            checksum: migration.checksum.clone(),
            applied_at,
        })
    }

    /// Apply all pending migrations
    pub async fn run_migrations(
        &self,
        migrations: &[PendingMigration],
        dry_run: bool,
    ) -> Result<MigrationReport> {
        self.ensure_tracking_table().await?;

        let applied = self.get_applied_migrations().await?;
        let pending = self.get_pending_migrations(migrations).await?;
        let mut report = MigrationReport::new(dry_run);

        // Record skipped (already applied)
        report.skipped = applied.iter().map(|m| m.version.clone()).collect();

        if dry_run {
            // In dry run, just mark what would be applied
            for migration in &pending {
                report.applied.push(AppliedMigration {
                    version: migration.version.clone(),
                    name: migration.name.clone(),
                    checksum: migration.checksum.clone(),
                    applied_at: Utc::now(),
                });
            }
            return Ok(report);
        }

        // Apply each pending migration
        for migration in &pending {
            match self.apply_migration(migration).await {
                Ok(applied) => {
                    report.applied.push(applied);
                }
                Err(e) => {
                    report.failed = Some((migration.version.clone(), e.to_string()));
                    break;
                }
            }
        }

        Ok(report)
    }

    /// Verify migration checksums match
    pub async fn verify_checksums(&self, migrations: &[PendingMigration]) -> Result<Vec<String>> {
        let applied = self.get_applied_migrations().await?;
        let mut mismatches = Vec::new();

        for applied_migration in &applied {
            if let Some(local) = migrations.iter().find(|m| m.version == applied_migration.version)
            {
                if local.checksum != applied_migration.checksum {
                    mismatches.push(format!(
                        "{}: expected {}, got {}",
                        applied_migration.version, applied_migration.checksum, local.checksum
                    ));
                }
            }
        }

        Ok(mismatches)
    }

    /// Remove a migration record (for rollback)
    pub async fn remove_migration_record(&self, version: &str) -> Result<()> {
        sqlx::query("DELETE FROM _ormada_migrations WHERE version = $1")
            .bind(version)
            .execute(&self.pool)
            .await
            .context("Failed to remove migration record")?;
        Ok(())
    }
}

/// Validate migration ordering and dependencies
#[derive(Debug, Clone)]
pub struct MigrationValidator {
    migrations: Vec<MigrationMeta>,
}

#[derive(Debug, Clone)]
pub struct MigrationMeta {
    pub id: String,
    pub after: Option<String>,
}

impl MigrationValidator {
    pub fn new() -> Self {
        Self { migrations: Vec::new() }
    }

    pub fn add(&mut self, id: &str, after: Option<&str>) {
        self.migrations.push(MigrationMeta {
            id: id.to_string(),
            after: after.map(String::from),
        });
    }

    /// Validate migrations and return ordered list or error
    pub fn validate(&self) -> Result<Vec<String>> {
        // Check for duplicate IDs
        let mut seen = std::collections::HashSet::new();
        for m in &self.migrations {
            if !seen.insert(&m.id) {
                anyhow::bail!("Duplicate migration ID: {}", m.id);
            }
        }

        // Check for missing dependencies
        let ids: std::collections::HashSet<_> = self.migrations.iter().map(|m| &m.id).collect();
        for m in &self.migrations {
            if let Some(ref after) = m.after {
                if !ids.contains(after) {
                    anyhow::bail!(
                        "Migration '{}' depends on '{}' which does not exist",
                        m.id,
                        after
                    );
                }
            }
        }

        // Topological sort to detect cycles and get order
        self.topological_sort()
    }

    fn topological_sort(&self) -> Result<Vec<String>> {
        use std::collections::{HashMap, VecDeque};

        // Build adjacency list (after -> [dependents])
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();

        for m in &self.migrations {
            in_degree.entry(&m.id).or_insert(0);
            graph.entry(&m.id).or_default();

            if let Some(ref after) = m.after {
                *in_degree.entry(&m.id).or_insert(0) += 1;
                graph.entry(after.as_str()).or_default().push(&m.id);
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<&str> =
            in_degree.iter().filter(|(_, &deg)| deg == 0).map(|(&id, _)| id).collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.to_string());

            if let Some(dependents) = graph.get(node) {
                for &dep in dependents {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if result.len() != self.migrations.len() {
            anyhow::bail!("Circular dependency detected in migrations");
        }

        Ok(result)
    }
}

/// Split SQL into individual statements
/// Handles semicolons inside strings and parentheses
fn split_sql_statements(sql: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    let mut paren_depth: i32 = 0;

    let chars: Vec<char> = sql.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
        } else {
            match c {
                '\'' | '"' => {
                    in_string = true;
                    string_char = c;
                }
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                ';' if paren_depth == 0 => {
                    let stmt = &sql[start..i];
                    if !stmt.trim().is_empty() {
                        statements.push(stmt);
                    }
                    start = i + 1;
                }
                _ => {}
            }
        }
    }

    // Don't forget the last statement (might not end with semicolon)
    let last = &sql[start..];
    if !last.trim().is_empty() {
        statements.push(last);
    }

    statements
}

/// Helper to filter pending migrations without database
pub fn filter_pending_migrations(
    all_migrations: &[PendingMigration],
    applied_versions: &[&str],
) -> Vec<PendingMigration> {
    let applied_set: std::collections::HashSet<_> = applied_versions.iter().copied().collect();
    all_migrations
        .iter()
        .filter(|m| !applied_set.contains(m.version.as_str()))
        .cloned()
        .collect()
}

/// Verify checksums match between local and applied migrations
pub fn verify_checksums_local(
    local_migrations: &[PendingMigration],
    applied_migrations: &[AppliedMigration],
) -> Vec<String> {
    let mut mismatches = Vec::new();

    for applied in applied_migrations {
        if let Some(local) = local_migrations.iter().find(|m| m.version == applied.version) {
            if local.checksum != applied.checksum {
                mismatches.push(format!(
                    "{}: local={}, applied={}",
                    applied.version,
                    &local.checksum[..8],
                    &applied.checksum[..8]
                ));
            }
        }
    }

    mismatches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_sql_statements_simple() {
        let sql = "CREATE TABLE a; CREATE TABLE b;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("CREATE TABLE a"));
        assert!(stmts[1].contains("CREATE TABLE b"));
    }

    #[test]
    fn test_split_sql_statements_with_parentheses() {
        let sql = "CREATE TABLE a (id INT; name TEXT); CREATE TABLE b;";
        let stmts = split_sql_statements(sql);
        // The semicolon inside parentheses should NOT split
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_split_sql_statements_with_strings() {
        let sql = "INSERT INTO a VALUES ('hello; world'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        // The semicolon inside the string should NOT split
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_split_sql_statements_multiline() {
        let sql = r#"
CREATE TABLE users (
    id INTEGER,
    name TEXT
);

CREATE TABLE posts (
    id INTEGER
);
"#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_compute_checksum() {
        let checksum1 = compute_checksum("CREATE TABLE foo (id INT);");
        let checksum2 = compute_checksum("CREATE TABLE foo (id INT);");
        let checksum3 = compute_checksum("CREATE TABLE bar (id INT);");

        assert_eq!(checksum1, checksum2);
        assert_ne!(checksum1, checksum3);
        assert_eq!(checksum1.len(), 64); // SHA256 hex = 64 chars
    }

    #[test]
    fn test_validator_detects_duplicates() {
        let mut v = MigrationValidator::new();
        v.add("m001", None);
        v.add("m001", None);

        let result = v.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate"));
    }

    #[test]
    fn test_validator_detects_missing_dependency() {
        let mut v = MigrationValidator::new();
        v.add("m002", Some("m001"));

        let result = v.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_validator_detects_circular_dependency() {
        let mut v = MigrationValidator::new();
        v.add("m001", Some("m002"));
        v.add("m002", Some("m001"));

        let result = v.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular"));
    }

    #[test]
    fn test_validator_orders_correctly() {
        let mut v = MigrationValidator::new();
        v.add("m003", Some("m002"));
        v.add("m001", None);
        v.add("m002", Some("m001"));

        let result = v.validate().unwrap();
        assert_eq!(result, vec!["m001", "m002", "m003"]);
    }

    #[test]
    fn test_validator_handles_no_dependencies() {
        let mut v = MigrationValidator::new();
        v.add("m001", None);
        v.add("m002", None);
        v.add("m003", None);

        let result = v.validate().unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_migration_report_format() {
        let mut report = MigrationReport::new(false);
        report.applied.push(AppliedMigration {
            version: "m001".to_string(),
            name: "initial".to_string(),
            checksum: "abc123".to_string(),
            applied_at: Utc::now(),
        });

        let output = report.format();
        assert!(output.contains("Applied"));
        assert!(output.contains("m001"));
    }

    #[test]
    fn test_dry_run_report() {
        let report = MigrationReport::new(true);
        let output = report.format();
        assert!(output.contains("DRY RUN"));
    }

    #[test]
    fn test_filter_pending_migrations_all_pending() {
        let all = vec![
            PendingMigration::new("m001", "initial", "CREATE TABLE a;"),
            PendingMigration::new("m002", "add_users", "CREATE TABLE b;"),
            PendingMigration::new("m003", "add_posts", "CREATE TABLE c;"),
        ];

        let pending = filter_pending_migrations(&all, &[]);
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn test_filter_pending_migrations_some_applied() {
        let all = vec![
            PendingMigration::new("m001", "initial", "CREATE TABLE a;"),
            PendingMigration::new("m002", "add_users", "CREATE TABLE b;"),
            PendingMigration::new("m003", "add_posts", "CREATE TABLE c;"),
        ];

        let pending = filter_pending_migrations(&all, &["m001", "m002"]);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].version, "m003");
    }

    #[test]
    fn test_filter_pending_migrations_all_applied() {
        let all = vec![
            PendingMigration::new("m001", "initial", "CREATE TABLE a;"),
            PendingMigration::new("m002", "add_users", "CREATE TABLE b;"),
        ];

        let pending = filter_pending_migrations(&all, &["m001", "m002"]);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_verify_checksums_local_match() {
        let local = vec![PendingMigration::new("m001", "initial", "CREATE TABLE a;")];
        let applied = vec![AppliedMigration {
            version: "m001".to_string(),
            name: "initial".to_string(),
            checksum: compute_checksum("CREATE TABLE a;"),
            applied_at: Utc::now(),
        }];

        let mismatches = verify_checksums_local(&local, &applied);
        assert!(mismatches.is_empty());
    }

    #[test]
    fn test_verify_checksums_local_mismatch() {
        let local = vec![PendingMigration::new("m001", "initial", "CREATE TABLE a;")];
        let applied = vec![AppliedMigration {
            version: "m001".to_string(),
            name: "initial".to_string(),
            checksum: "different_checksum_value_here_1234567890abcdef".to_string(),
            applied_at: Utc::now(),
        }];

        let mismatches = verify_checksums_local(&local, &applied);
        assert_eq!(mismatches.len(), 1);
        assert!(mismatches[0].contains("m001"));
    }

    #[test]
    fn test_verify_checksums_local_new_migration_not_mismatch() {
        let local = vec![
            PendingMigration::new("m001", "initial", "CREATE TABLE a;"),
            PendingMigration::new("m002", "add_users", "CREATE TABLE b;"),
        ];
        let applied = vec![AppliedMigration {
            version: "m001".to_string(),
            name: "initial".to_string(),
            checksum: compute_checksum("CREATE TABLE a;"),
            applied_at: Utc::now(),
        }];

        // m002 is new, not applied yet - should not be a mismatch
        let mismatches = verify_checksums_local(&local, &applied);
        assert!(mismatches.is_empty());
    }

    #[test]
    fn test_pending_migration_checksum_computed() {
        let m1 = PendingMigration::new("m001", "test", "SELECT 1;");
        let m2 = PendingMigration::new("m002", "test", "SELECT 1;");
        let m3 = PendingMigration::new("m003", "test", "SELECT 2;");

        // Same SQL = same checksum
        assert_eq!(m1.checksum, m2.checksum);
        // Different SQL = different checksum
        assert_ne!(m1.checksum, m3.checksum);
    }
}
