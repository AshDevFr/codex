// Tests for init_database with CODEX_SKIP_MIGRATIONS

use codex::commands::common::init_database;
use codex::config::{Config, DatabaseConfig, DatabaseType, SQLiteConfig};
use std::env;
use tempfile::TempDir;

#[tokio::test]
async fn test_init_database_without_skip_migrations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Ensure CODEX_SKIP_MIGRATIONS is not set
    env::remove_var("CODEX_SKIP_MIGRATIONS");

    let config = Config {
        application: codex::config::ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: None,
            }),
        },
        api: codex::config::ApiConfig {
            base_path: "/api/v1".to_string(),
            cors_enabled: false,
            cors_origins: vec![],
            max_page_size: 100,
            enable_swagger: false,
            swagger_path: "/swagger".to_string(),
        },
        auth: codex::config::AuthConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 24,
        },
        email: codex::config::EmailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 25,
            smtp_from_email: "test@example.com".to_string(),
            smtp_username: None,
            smtp_password: None,
        },
        logging: codex::config::LoggingConfig {
            level: "info".to_string(),
            console: true,
            file: None,
        },
        scanner: codex::config::ScannerConfig {
            max_concurrent_scans: 2,
        },
        task: codex::config::TaskConfig {
            worker_count: 4,
        },
    };

    // Initialize database - should run migrations
    let result = init_database(&config).await;

    assert!(
        result.is_ok(),
        "init_database should succeed when CODEX_SKIP_MIGRATIONS is not set: {:?}",
        result
    );

    let db = result.unwrap();

    // Verify migrations are complete
    let complete = db.migrations_complete().await.unwrap();
    assert!(complete, "Migrations should be complete after init_database");
}

#[tokio::test]
async fn test_init_database_with_skip_migrations_complete() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Set CODEX_SKIP_MIGRATIONS
    env::set_var("CODEX_SKIP_MIGRATIONS", "true");

    let config = Config {
        application: codex::config::ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: None,
            }),
        },
        api: codex::config::ApiConfig {
            base_path: "/api/v1".to_string(),
            cors_enabled: false,
            cors_origins: vec![],
            max_page_size: 100,
            enable_swagger: false,
            swagger_path: "/swagger".to_string(),
        },
        auth: codex::config::AuthConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 24,
        },
        email: codex::config::EmailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 25,
            smtp_from_email: "test@example.com".to_string(),
            smtp_username: None,
            smtp_password: None,
        },
        logging: codex::config::LoggingConfig {
            level: "info".to_string(),
            console: true,
            file: None,
        },
        scanner: codex::config::ScannerConfig {
            max_concurrent_scans: 2,
        },
        task: codex::config::TaskConfig {
            worker_count: 4,
        },
    };

    // First, run migrations to set up the database
    let db = codex::db::Database::new(&config.database).await.unwrap();
    db.run_migrations().await.unwrap();

    // Now initialize with skip migrations - should succeed since migrations are complete
    let result = init_database(&config).await;

    assert!(
        result.is_ok(),
        "init_database should succeed when CODEX_SKIP_MIGRATIONS is set and migrations are complete: {:?}",
        result
    );

    // Clean up
    env::remove_var("CODEX_SKIP_MIGRATIONS");
}

#[tokio::test]
async fn test_init_database_with_skip_migrations_incomplete() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Set CODEX_SKIP_MIGRATIONS
    env::set_var("CODEX_SKIP_MIGRATIONS", "true");

    let config = Config {
        application: codex::config::ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: None,
            }),
        },
        api: codex::config::ApiConfig {
            base_path: "/api/v1".to_string(),
            cors_enabled: false,
            cors_origins: vec![],
            max_page_size: 100,
            enable_swagger: false,
            swagger_path: "/swagger".to_string(),
        },
        auth: codex::config::AuthConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 24,
        },
        email: codex::config::EmailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 25,
            smtp_from_email: "test@example.com".to_string(),
            smtp_username: None,
            smtp_password: None,
        },
        logging: codex::config::LoggingConfig {
            level: "info".to_string(),
            console: true,
            file: None,
        },
        scanner: codex::config::ScannerConfig {
            max_concurrent_scans: 2,
        },
        task: codex::config::TaskConfig {
            worker_count: 4,
        },
    };

    // Initialize with skip migrations on a fresh database - should fail
    let result = init_database(&config).await;

    assert!(
        result.is_err(),
        "init_database should fail when CODEX_SKIP_MIGRATIONS is set but migrations are incomplete: {:?}",
        result
    );

    // Verify the error message
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("not complete"),
        "Error message should mention migrations are not complete: {}",
        error_msg
    );

    // Clean up
    env::remove_var("CODEX_SKIP_MIGRATIONS");
}

#[tokio::test]
async fn test_init_database_with_skip_migrations_variant_1() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Set CODEX_SKIP_MIGRATIONS to "1" (alternative form)
    env::set_var("CODEX_SKIP_MIGRATIONS", "1");

    let config = Config {
        application: codex::config::ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: None,
            }),
        },
        api: codex::config::ApiConfig {
            base_path: "/api/v1".to_string(),
            cors_enabled: false,
            cors_origins: vec![],
            max_page_size: 100,
            enable_swagger: false,
            swagger_path: "/swagger".to_string(),
        },
        auth: codex::config::AuthConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 24,
        },
        email: codex::config::EmailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 25,
            smtp_from_email: "test@example.com".to_string(),
            smtp_username: None,
            smtp_password: None,
        },
        logging: codex::config::LoggingConfig {
            level: "info".to_string(),
            console: true,
            file: None,
        },
        scanner: codex::config::ScannerConfig {
            max_concurrent_scans: 2,
        },
        task: codex::config::TaskConfig {
            worker_count: 4,
        },
    };

    // First, run migrations to set up the database
    let db = codex::db::Database::new(&config.database).await.unwrap();
    db.run_migrations().await.unwrap();

    // Now initialize with skip migrations - should succeed
    let result = init_database(&config).await;

    assert!(
        result.is_ok(),
        "init_database should succeed when CODEX_SKIP_MIGRATIONS is set to '1' and migrations are complete: {:?}",
        result
    );

    // Clean up
    env::remove_var("CODEX_SKIP_MIGRATIONS");
}

