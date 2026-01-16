use codex::commands::{migrate_command, wait_for_migrations_command};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_wait_for_migrations_when_complete() {
    // Create a temporary config file
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");

    // Create a minimal config file
    let config_content = format!(
        r#"
application:
  host: "127.0.0.1"
  port: 8080

database:
  db_type: sqlite
  sqlite:
    path: "{}"
"#,
        temp_dir.path().join("test.db").to_str().unwrap()
    );

    std::fs::write(&config_path, config_content).unwrap();

    // Run migrations first
    migrate_command(PathBuf::from(config_path.clone()))
        .await
        .expect("Migration should succeed");

    // Wait for migrations (should complete immediately)
    let result = wait_for_migrations_command(PathBuf::from(config_path), Some(10), Some(1)).await;

    // Should succeed quickly since migrations are already complete
    assert!(
        result.is_ok(),
        "Wait for migrations should succeed when migrations are complete: {:?}",
        result
    );
}

#[tokio::test]
async fn test_wait_for_migrations_timeout() {
    // Create a temporary config file pointing to a non-existent database
    // This will cause connection failures, which should eventually timeout
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");

    // Create a config file pointing to a non-existent database path
    let config_content = format!(
        r#"
application:
  host: "127.0.0.1"
  port: 8080

database:
  db_type: sqlite
  sqlite:
    path: "/nonexistent/path/to/database.db"
"#,
    );

    std::fs::write(&config_path, config_content).unwrap();

    // Wait for migrations with a short timeout
    let result = wait_for_migrations_command(PathBuf::from(config_path), Some(2), Some(1)).await;

    // Should timeout or fail
    assert!(
        result.is_err(),
        "Wait for migrations should fail or timeout when database is unreachable"
    );
}

#[tokio::test]
async fn test_wait_for_migrations_with_pending_migrations() {
    // This test simulates waiting for migrations that are in progress
    // We create a fresh database and wait for migrations to complete
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");
    let db_path = temp_dir.path().join("test.db");

    // Create a minimal config file
    let config_content = format!(
        r#"
application:
  host: "127.0.0.1"
  port: 8080

database:
  db_type: sqlite
  sqlite:
    path: "{}"
"#,
        db_path.to_str().unwrap()
    );

    std::fs::write(&config_path, config_content).unwrap();

    // Start waiting for migrations in the background
    let wait_handle = tokio::spawn({
        let config_path = config_path.clone();
        async move { wait_for_migrations_command(PathBuf::from(config_path), Some(10), Some(1)).await }
    });

    // Give it a moment to start waiting
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now run migrations (simulating another process running migrations)
    migrate_command(PathBuf::from(config_path.clone()))
        .await
        .expect("Migration should succeed");

    // Wait for the wait command to complete
    let result = wait_handle.await.unwrap();

    // Should succeed now that migrations are complete
    assert!(
        result.is_ok(),
        "Wait for migrations should succeed after migrations are run: {:?}",
        result
    );
}

#[tokio::test]
async fn test_wait_for_migrations_default_timeout() {
    // Test that default timeout works
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");

    // Create a config file pointing to a non-existent database path
    let config_content = format!(
        r#"
application:
  host: "127.0.0.1"
  port: 8080

database:
  db_type: sqlite
  sqlite:
    path: "/nonexistent/path/to/database.db"
"#,
    );

    std::fs::write(&config_path, config_content).unwrap();

    // Wait for migrations with default timeout (should be 300 seconds)
    // But we'll use a shorter timeout for testing
    let start = std::time::Instant::now();
    let result = wait_for_migrations_command(PathBuf::from(config_path), Some(2), None).await;
    let elapsed = start.elapsed();

    // Should timeout or fail
    assert!(
        result.is_err(),
        "Wait for migrations should fail or timeout when database is unreachable"
    );

    // Should have waited at least 2 seconds (timeout)
    assert!(
        elapsed >= std::time::Duration::from_secs(2),
        "Should have waited at least the timeout duration"
    );
}
