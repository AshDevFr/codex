mod api;
mod commands;
mod config;
mod db;
mod models;
mod parsers;
mod scanner;
mod services;
mod utils;
mod web;

use clap::{Parser, Subcommand};
use commands::{scan_command, seed_command, serve_command};
use std::path::PathBuf;

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
        #[arg(short, long, default_value = "codex.yaml")]
        config: PathBuf,
    },

    /// Create initial admin user and API key
    Seed {
        /// Path to configuration file
        #[arg(short, long, default_value = "codex.yaml")]
        config: PathBuf,
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
        Commands::Seed { config } => {
            seed_command(config).await?;
        }
    }

    Ok(())
}
