use crate::config::{Config, DatabaseType};
use std::path::PathBuf;

/// Serve command handler - starts the media server
pub fn serve_command(config_path: PathBuf) -> anyhow::Result<()> {
    let config = Config::from_file(config_path.to_str().unwrap())?;

    println!("Starting {}", config.application.name);

    match config.database.db_type {
        DatabaseType::Postgres => {
            println!(
                "Database: {:?} at {}:{}",
                config.database.db_type,
                config.database.postgres.as_ref().unwrap().host,
                config.database.postgres.as_ref().unwrap().port
            );
        }
        DatabaseType::SQLite => {
            println!(
                "Database: {:?} at {}",
                config.database.db_type,
                config.database.sqlite.as_ref().unwrap().path
            );
        }
    }
    println!(
        "Application server: {}:{}",
        config.application.host, config.application.port
    );
    println!("Debug mode: {}", config.application.debug);

    Ok(())
}

