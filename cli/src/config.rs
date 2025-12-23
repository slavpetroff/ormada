//! Configuration loading for Ormada CLI

use anyhow::{Context, Result};
use std::path::PathBuf;

/// CLI configuration
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// Project root directory
    pub project_root: PathBuf,
    /// Migrations directory
    pub migrations_dir: PathBuf,
    /// Source directories to scan for models
    pub model_paths: Vec<PathBuf>,
    /// Database URL
    pub database_url: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            migrations_dir: PathBuf::from("migrations"),
            model_paths: vec![PathBuf::from("src")],
            database_url: None,
        }
    }
}

impl CliConfig {
    /// Load configuration from current directory
    ///
    /// Configuration is loaded in order of precedence:
    /// 1. ormada.toml (if exists)
    /// 2. [package.metadata.ormada] in Cargo.toml
    /// 3. Defaults
    pub fn load() -> Result<Self> {
        let project_root = find_project_root()?;
        let mut config = Self {
            project_root: project_root.clone(),
            migrations_dir: PathBuf::from("migrations"),
            ..Self::default()
        };

        // Try to load database URL from environment
        config.database_url = std::env::var("DATABASE_URL").ok();

        // First, try ormada.toml
        let ormada_toml = project_root.join("ormada.toml");
        if ormada_toml.exists() {
            let content = std::fs::read_to_string(&ormada_toml)
                .with_context(|| format!("Failed to read {}", ormada_toml.display()))?;
            parse_ormada_config(&content, &mut config);
            return Ok(config);
        }

        // Fall back to Cargo.toml [package.metadata.ormada]
        let cargo_toml = project_root.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)
                .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
            parse_cargo_metadata(&content, &mut config);
        }

        Ok(config)
    }

    /// Get absolute path to migrations directory
    pub fn migrations_path(&self) -> PathBuf {
        if self.migrations_dir.is_absolute() {
            self.migrations_dir.clone()
        } else {
            self.project_root.join(&self.migrations_dir)
        }
    }

    /// Ensure migrations directory exists
    pub fn ensure_migrations_dir(&self) -> Result<()> {
        let path = self.migrations_path();
        if !path.exists() {
            std::fs::create_dir_all(&path).with_context(|| {
                format!("Failed to create migrations directory: {}", path.display())
            })?;
        }
        Ok(())
    }
}

/// Find the project root by looking for Cargo.toml
fn find_project_root() -> Result<PathBuf> {
    let current = std::env::current_dir().context("Failed to get current directory")?;

    let mut path = current.as_path();
    loop {
        if path.join("Cargo.toml").exists() {
            return Ok(path.to_path_buf());
        }

        match path.parent() {
            Some(parent) => path = parent,
            None => break,
        }
    }

    // Fall back to current directory
    Ok(current)
}

/// Generate a migration ID from timestamp and name
pub fn generate_migration_id(name: &str) -> String {
    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d_%H%M%S");
    let slug = slugify(name);
    format!("m{}_{}", timestamp, slug)
}

/// Convert a name to a slug (lowercase, underscores)
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Parse ormada.toml config content
fn parse_ormada_config(content: &str, config: &mut CliConfig) {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("migrations_dir") {
            if let Some((_key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                config.migrations_dir = PathBuf::from(value);
            }
        }
        if line.starts_with("url") && !line.starts_with('#') {
            if let Some((_key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                config.database_url = Some(value.to_string());
            }
        }
    }
}

/// Parse [package.metadata.ormada] from Cargo.toml
fn parse_cargo_metadata(content: &str, config: &mut CliConfig) {
    let mut in_ormada_section = false;

    for line in content.lines() {
        let line = line.trim();

        // Check for section headers
        if line.starts_with('[') {
            in_ormada_section = line == "[package.metadata.ormada]"
                || line == "[package.metadata.ormada.migrations]";
            continue;
        }

        if !in_ormada_section {
            continue;
        }

        // Parse key-value pairs
        if line.starts_with("migrations_dir") || line.starts_with("dir") {
            if let Some((_key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                config.migrations_dir = PathBuf::from(value);
            }
        }
        if line.starts_with("database_url") {
            if let Some((_key, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                config.database_url = Some(value.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Add Books Table"), "add_books_table");
        assert_eq!(slugify("add-isbn-column"), "add_isbn_column");
        assert_eq!(slugify("Initial Migration!"), "initial_migration");
    }

    #[test]
    fn test_parse_ormada_config() {
        let content = r#"
[migrations]
migrations_dir = "src/db/migrations"

[database]
url = "postgres://localhost/test"
"#;
        let mut config = CliConfig::default();
        parse_ormada_config(content, &mut config);

        assert_eq!(config.migrations_dir, PathBuf::from("src/db/migrations"));
        assert_eq!(config.database_url, Some("postgres://localhost/test".to_string()));
    }

    #[test]
    fn test_parse_cargo_metadata() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
ormada = "0.1"

[package.metadata.ormada]
migrations_dir = "db/migrations"
database_url = "sqlite://./dev.db"
"#;
        let mut config = CliConfig::default();
        parse_cargo_metadata(content, &mut config);

        assert_eq!(config.migrations_dir, PathBuf::from("db/migrations"));
        assert_eq!(config.database_url, Some("sqlite://./dev.db".to_string()));
    }

    #[test]
    fn test_parse_cargo_metadata_with_dir_shorthand() {
        let content = r#"
[package.metadata.ormada]
dir = "migrations"
"#;
        let mut config = CliConfig::default();
        parse_cargo_metadata(content, &mut config);

        assert_eq!(config.migrations_dir, PathBuf::from("migrations"));
    }

    #[test]
    fn test_generate_migration_id() {
        let id = generate_migration_id("add books");
        assert!(id.starts_with("m20"));
        assert!(id.contains("add_books"));
    }
}
