//! Ormada CLI - Migration management tool
//!
//! Commands:
//! - `ormada migrate make <name>` - Generate migration from model changes
//! - `ormada migrate run` - Apply pending migrations
//! - `ormada migrate status` - Show migration status
//! - `ormada migrate rollback` - Rollback last migration
//! - `ormada migrate sql` - Show SQL without applying

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod config;
mod generator;
mod introspect;
mod sql_generator;
mod tracker;

#[derive(Parser)]
#[command(name = "ormada")]
#[command(author = "Stanislav Petrov <slavpetroff@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "Ormada ORM CLI - Database migration management", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Migration management commands
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },
}

#[derive(Subcommand)]
enum MigrateAction {
    /// Generate a new migration from model changes
    Make {
        /// Migration name/description
        name: String,

        /// Don't prompt for confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Apply pending migrations
    Run {
        /// Apply only this specific migration
        #[arg(long)]
        migration: Option<String>,

        /// Show SQL without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Show migration status
    Status,

    /// Rollback the last applied migration
    Rollback {
        /// Number of migrations to rollback
        #[arg(short, long, default_value = "1")]
        steps: u32,

        /// Don't prompt for confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Generate SQL for pending migrations without applying
    Sql {
        /// Output to file instead of stdout
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Initialize migrations directory
    Init {
        /// Custom path for migrations directory (default: migrations)
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Migrate { action } => match action {
            MigrateAction::Make { name, yes } => {
                commands::migrate_make(&name, yes).await?;
            }
            MigrateAction::Run { migration, dry_run } => {
                commands::migrate_run(migration.as_deref(), dry_run).await?;
            }
            MigrateAction::Status => {
                commands::migrate_status().await?;
            }
            MigrateAction::Rollback { steps, yes } => {
                commands::migrate_rollback(steps, yes).await?;
            }
            MigrateAction::Sql { output } => {
                commands::migrate_sql(output.as_deref()).await?;
            }
            MigrateAction::Init { path } => {
                commands::migrate_init(path.as_deref()).await?;
            }
        },
    }

    Ok(())
}
