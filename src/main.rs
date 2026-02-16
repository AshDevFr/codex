mod api;
mod commands;
mod config;
mod db;
mod events;
mod models;
mod parsers;
mod scanner;
mod scheduler;
mod services;
mod tasks;
mod utils;
mod web;

use clap::{Parser, Subcommand};
use commands::{
    OpenApiFormat, TasksSubcommand, migrate_command, openapi_command, scan_command, seed_command,
    serve_command, tasks_command, wait_for_migrations_command, worker_command,
};
use std::path::PathBuf;

const DEFAULT_CONFIG_PATH: &str = "config/codex.yaml";

#[derive(Parser)]
#[command(name = "codex")]
#[command(about = "A digital library media server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a directory or file and analyze its contents (debugging tool)
    Scan {
        /// Path to directory or file to scan
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Output in JSON format (single object or array)
        #[arg(short, long)]
        json: bool,

        /// Include detailed page information
        #[arg(short, long)]
        pages: bool,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Start the media server
    Serve {
        /// Path to configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
    },

    /// Start task workers (without web server)
    Worker {
        /// Path to configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
    },

    /// Create initial admin user and API key
    Seed {
        /// Path to configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,

        /// Path to seed configuration file (YAML) for plugins, libraries, and user passwords
        #[arg(long)]
        seed_config: Option<PathBuf>,
    },

    /// Run database migrations
    Migrate {
        /// Path to configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,
    },

    /// Wait for database migrations to complete
    WaitForMigrations {
        /// Path to configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,

        /// Timeout in seconds (default: 300)
        #[arg(short, long, default_value = "300")]
        timeout: Option<u64>,

        /// Check interval in seconds (default: 2)
        #[arg(short, long, default_value = "2")]
        interval: Option<u64>,
    },

    /// Export OpenAPI specification to a file
    Openapi {
        /// Output file path
        #[arg(short, long, default_value = "openapi.json")]
        output: PathBuf,

        /// Output format (json or yaml)
        #[arg(short, long, default_value = "json")]
        format: OpenApiFormat,
    },

    /// Task queue management commands
    Tasks {
        /// Path to configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,

        #[command(subcommand)]
        command: TasksSubcommand,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            json,
            pages,
            verbose,
        } => {
            scan_command(path, json, pages, verbose)?;
        }
        Commands::Serve { config } => {
            serve_command(config).await?;
        }
        Commands::Worker { config } => {
            worker_command(config).await?;
        }
        Commands::Seed {
            config,
            seed_config,
        } => {
            seed_command(config, seed_config).await?;
        }
        Commands::Migrate { config } => {
            migrate_command(config).await?;
        }
        Commands::WaitForMigrations {
            config,
            timeout,
            interval,
        } => {
            wait_for_migrations_command(config, timeout, interval).await?;
        }
        Commands::Openapi { output, format } => {
            openapi_command(output, format)?;
        }
        Commands::Tasks { config, command } => {
            tasks_command(config, command).await?;
        }
    }

    Ok(())
}
