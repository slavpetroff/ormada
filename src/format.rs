//! SQL formatting utilities for pretty-printing queries
//!
//! This module provides SQL formatting functionality for debugging and logging.
//! By default, SQL output is pretty-printed with proper indentation and line breaks.

use sqlformat::{FormatOptions, Indent, QueryParams};

/// Options for SQL formatting
#[derive(Debug, Clone)]
pub struct SqlFormatOptions {
    /// Whether to pretty-print the SQL (default: true)
    pub pretty: bool,
    /// Number of spaces for indentation (default: 2)
    pub indent_spaces: u8,
    /// Whether to uppercase SQL keywords (default: true)
    pub uppercase: bool,
}

impl Default for SqlFormatOptions {
    fn default() -> Self {
        Self {
            pretty: true,
            indent_spaces: 2,
            uppercase: true,
        }
    }
}

impl SqlFormatOptions {
    /// Create options with pretty-printing disabled (single-line output)
    #[must_use]
    pub const fn compact() -> Self {
        Self {
            pretty: false,
            indent_spaces: 2,
            uppercase: true,
        }
    }

    /// Create options with custom indentation
    #[must_use]
    pub const fn with_indent(mut self, spaces: u8) -> Self {
        self.indent_spaces = spaces;
        self
    }

    /// Create options with uppercase keywords disabled
    #[must_use]
    pub const fn lowercase_keywords(mut self) -> Self {
        self.uppercase = false;
        self
    }
}

/// Format a SQL string with the given options
///
/// # Arguments
/// * `sql` - The SQL string to format
/// * `options` - Formatting options (use `None` for defaults with pretty-printing)
///
/// # Returns
/// The formatted SQL string
///
/// # Examples
///
/// ```rust,ignore
/// use ormada::format::{format_sql, SqlFormatOptions};
///
/// let sql = "SELECT id, name FROM users WHERE age > 18 AND status = 'active'";
///
/// // Pretty-print (default)
/// let pretty = format_sql(sql, None);
/// println!("{}", pretty);
/// // Output:
/// // SELECT
/// //   id,
/// //   name
/// // FROM
/// //   users
/// // WHERE
/// //   age > 18
/// //   AND status = 'active'
///
/// // Compact (single-line)
/// let compact = format_sql(sql, Some(SqlFormatOptions::compact()));
/// println!("{}", compact);
/// // Output: SELECT id, name FROM users WHERE age > 18 AND status = 'active'
/// ```
#[must_use]
pub fn format_sql(sql: &str, options: Option<SqlFormatOptions>) -> String {
    let opts = options.unwrap_or_default();

    if !opts.pretty {
        return sql.to_string();
    }

    let format_options = FormatOptions {
        indent: Indent::Spaces(opts.indent_spaces),
        uppercase: Some(opts.uppercase),
        lines_between_queries: 1,
        ..Default::default()
    };

    sqlformat::format(sql, &QueryParams::None, &format_options)
}

/// Format SQL with default pretty-printing options
///
/// This is a convenience function equivalent to `format_sql(sql, None)`.
#[must_use]
pub fn format_sql_pretty(sql: &str) -> String {
    format_sql(sql, None)
}

/// Format SQL in compact single-line format
///
/// This is a convenience function equivalent to `format_sql(sql, Some(SqlFormatOptions::compact()))`.
#[must_use]
pub fn format_sql_compact(sql: &str) -> String {
    format_sql(sql, Some(SqlFormatOptions::compact()))
}

/// Build an EXPLAIN SQL statement for the given backend
///
/// Returns the appropriate EXPLAIN command for the database backend.
#[must_use]
pub fn build_explain_sql(
    backend: crate::db::DatabaseBackend,
    raw_sql: &str,
    analyze: bool,
) -> String {
    match (backend, analyze) {
        (crate::db::DatabaseBackend::Sqlite, _) => format!("EXPLAIN QUERY PLAN {raw_sql}"),
        (crate::db::DatabaseBackend::Postgres | crate::db::DatabaseBackend::MySql | _, false) => {
            format!("EXPLAIN {raw_sql}")
        }
        (crate::db::DatabaseBackend::Postgres | crate::db::DatabaseBackend::MySql | _, true) => {
            format!("EXPLAIN ANALYZE {raw_sql}")
        }
    }
}

/// Format explain output with SQL and results
#[must_use]
pub fn format_explain_output(
    raw_sql: &str,
    pretty: bool,
    rows_affected: u64,
    analyze: bool,
    explain_sql: Option<&str>,
) -> String {
    let formatted_sql = if pretty { format_sql_pretty(raw_sql) } else { raw_sql.to_string() };
    let prefix = if analyze { "EXPLAIN ANALYZE" } else { "EXPLAIN" };

    explain_sql.map_or_else(
        || format!("{prefix} output for query:\n{formatted_sql}\n\nRows affected: {rows_affected}"),
        |explain| {
            format!(
                "{prefix} output for query:\n{formatted_sql}\n\nRows affected: {rows_affected}\n\nTo run manually: {explain}"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sql_pretty_default() {
        let sql = "SELECT id, name FROM users WHERE age > 18";
        let formatted = format_sql(sql, None);

        assert!(formatted.contains('\n'), "Pretty format should have newlines");
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
        assert!(formatted.contains("WHERE"));
    }

    #[test]
    fn test_format_sql_compact() {
        let sql = "SELECT id, name FROM users WHERE age > 18";
        let formatted = format_sql(sql, Some(SqlFormatOptions::compact()));

        assert!(!formatted.contains('\n'), "Compact format should not have newlines");
        assert_eq!(formatted, sql);
    }

    #[test]
    fn test_format_sql_uppercase() {
        let sql = "select id from users";
        let formatted = format_sql(sql, None);

        assert!(formatted.contains("SELECT"), "Keywords should be uppercased");
        assert!(formatted.contains("FROM"), "Keywords should be uppercased");
    }

    #[test]
    fn test_format_sql_lowercase() {
        let sql = "SELECT id FROM users";
        let formatted = format_sql(sql, Some(SqlFormatOptions::default().lowercase_keywords()));

        assert!(formatted.contains("select") || formatted.contains("SELECT"));
    }

    #[test]
    fn test_format_sql_complex_query() {
        let sql = "SELECT u.id, u.name, COUNT(o.id) as order_count FROM users u LEFT JOIN orders o ON u.id = o.user_id WHERE u.status = 'active' GROUP BY u.id, u.name ORDER BY order_count DESC LIMIT 10";
        let formatted = format_sql(sql, None);

        assert!(formatted.contains('\n'));
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("LEFT JOIN"));
        assert!(formatted.contains("GROUP BY"));
        assert!(formatted.contains("ORDER BY"));
    }

    #[test]
    fn test_build_explain_sql_postgres() {
        let sql = "SELECT * FROM users";

        let explain = build_explain_sql(crate::db::DatabaseBackend::Postgres, sql, false);
        assert_eq!(explain, "EXPLAIN SELECT * FROM users");

        let analyze = build_explain_sql(crate::db::DatabaseBackend::Postgres, sql, true);
        assert_eq!(analyze, "EXPLAIN ANALYZE SELECT * FROM users");
    }

    #[test]
    fn test_build_explain_sql_sqlite() {
        let sql = "SELECT * FROM users";

        let explain = build_explain_sql(crate::db::DatabaseBackend::Sqlite, sql, false);
        assert_eq!(explain, "EXPLAIN QUERY PLAN SELECT * FROM users");

        let analyze = build_explain_sql(crate::db::DatabaseBackend::Sqlite, sql, true);
        assert_eq!(analyze, "EXPLAIN QUERY PLAN SELECT * FROM users");
    }

    #[test]
    fn test_build_explain_sql_mysql() {
        let sql = "SELECT * FROM users";

        let explain = build_explain_sql(crate::db::DatabaseBackend::MySql, sql, false);
        assert_eq!(explain, "EXPLAIN SELECT * FROM users");

        let analyze = build_explain_sql(crate::db::DatabaseBackend::MySql, sql, true);
        assert_eq!(analyze, "EXPLAIN ANALYZE SELECT * FROM users");
    }

    #[test]
    fn test_format_explain_output_basic() {
        let output = format_explain_output("SELECT * FROM users", true, 10, false, None);

        assert!(output.contains("EXPLAIN output for query"));
        assert!(output.contains("Rows affected: 10"));
    }

    #[test]
    fn test_format_explain_output_analyze() {
        let explain_sql = "EXPLAIN ANALYZE SELECT * FROM users";
        let output =
            format_explain_output("SELECT * FROM users", false, 5, true, Some(explain_sql));

        assert!(output.contains("EXPLAIN ANALYZE output for query"));
        assert!(output.contains("Rows affected: 5"));
        assert!(output.contains("To run manually:"));
        assert!(output.contains(explain_sql));
    }
}
