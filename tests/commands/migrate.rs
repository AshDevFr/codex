use codex::commands::migrate_command;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_migrate_command() {
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

    // Run migrate command
    let result = migrate_command(PathBuf::from(config_path)).await;

    // Should succeed
    assert!(result.is_ok(), "Migration command should succeed: {:?}", result);
}

#[tokio::test]
async fn test_migrate_command_verifies_completion() {
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

    // Run migrate command
    let result = migrate_command(PathBuf::from(config_path.clone())).await;
    assert!(result.is_ok(), "First migration should succeed");

    // Run again - should still succeed (idempotent)
    let result2 = migrate_command(PathBuf::from(config_path)).await;
    assert!(result2.is_ok(), "Second migration should also succeed (idempotent)");
}

