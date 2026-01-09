use super::config::Config;
use anyhow::Result;
use std::fs;
use std::path::Path;

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ApiConfig, ApplicationConfig, AuthConfig, DatabaseConfig, DatabaseType, EmailConfig,
        LoggingConfig, SQLiteConfig, ScannerConfig, TaskConfig,
    };
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_from_file() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
application:
  host: 127.0.0.1
  port: 3000
"#;

        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), yaml_content).unwrap();

        let config = Config::from_file(temp_file.path()).unwrap();

        // Application name moved to database settings
        assert_eq!(config.application.host, "127.0.0.1");
        assert_eq!(config.application.port, 3000);
        assert!(matches!(config.database.db_type, DatabaseType::SQLite));
    }

    #[test]
    fn test_config_to_file() {
        let config = Config {
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: "./codex.db".to_string(),
                    pragmas: None,
                }),
            },
            application: ApplicationConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig::default(),
            scanner: ScannerConfig::default(),
        };

        let temp_file = NamedTempFile::new().unwrap();
        config.to_file(temp_file.path()).unwrap();

        let loaded_config = Config::from_file(temp_file.path()).unwrap();

        // Application name moved to database settings
        assert_eq!(loaded_config.application.port, 8080);
        assert!(matches!(
            loaded_config.database.db_type,
            DatabaseType::SQLite
        ));
    }

    #[test]
    fn test_config_from_invalid_file() {
        let result = Config::from_file("/nonexistent/path/to/file.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_from_malformed_yaml() {
        let yaml_content = "this is not valid yaml: {{{}";

        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), yaml_content).unwrap();

        let result = Config::from_file(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_task_and_scanner_sections() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
task:
  worker_count: 6
scanner:
  max_concurrent_scans: 3
"#;

        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), yaml_content).unwrap();

        let config = Config::from_file(temp_file.path()).unwrap();

        assert_eq!(config.task.worker_count, 6);
        assert_eq!(config.scanner.max_concurrent_scans, 3);
    }

    #[test]
    fn test_config_serialization_includes_task_and_scanner() {
        let config = Config {
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: "./codex.db".to_string(),
                    pragmas: None,
                }),
            },
            application: ApplicationConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig { worker_count: 8 },
            scanner: ScannerConfig {
                max_concurrent_scans: 4,
            },
        };

        let temp_file = NamedTempFile::new().unwrap();
        config.to_file(temp_file.path()).unwrap();

        let loaded_config = Config::from_file(temp_file.path()).unwrap();

        assert_eq!(loaded_config.task.worker_count, 8);
        assert_eq!(loaded_config.scanner.max_concurrent_scans, 4);
    }
}
