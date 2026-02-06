//! Settings service for managing runtime configuration
//!
//! TODO: Remove allow(dead_code) once all settings features are fully integrated

#![allow(dead_code)]

use crate::db::repositories::SettingsRepository;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
struct CachedSetting {
    value: String,
    value_type: String,
}

#[derive(Clone)]
pub struct SettingsService {
    cache: Arc<RwLock<HashMap<String, CachedSetting>>>,
    db: DatabaseConnection,
    last_reload: Arc<RwLock<DateTime<Utc>>>,
}

impl SettingsService {
    /// Initialize with database values
    pub async fn new(db: DatabaseConnection) -> Result<Self> {
        let service = Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            db,
            last_reload: Arc::new(RwLock::new(Utc::now())),
        };

        // Load all settings into cache
        service.reload().await?;

        Ok(service)
    }

    /// Start the automatic reload background task
    ///
    /// Accepts a `CancellationToken` for graceful shutdown support.
    /// Returns a `JoinHandle` that can be used to await task completion.
    pub fn start_auto_reload(
        self: Arc<Self>,
        reload_interval_seconds: u64,
        cancel_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let mut reload_interval = interval(Duration::from_secs(reload_interval_seconds));

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        tracing::info!("Settings auto-reload task shutting down");
                        break;
                    }
                    _ = reload_interval.tick() => {
                        if let Err(e) = self.reload().await {
                            tracing::error!("Failed to reload settings: {}", e);
                        }
                    }
                }
            }
        })
    }

    /// Get setting with cache
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        // Try cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(key) {
                let value: T = SettingsRepository::parse_value(&cached.value, &cached.value_type)?;
                return Ok(Some(value));
            }
        }

        // If not in cache, try database
        if let Some(value) = SettingsRepository::get_value::<T>(&self.db, key).await? {
            return Ok(Some(value));
        }

        Ok(None)
    }

    /// Get setting value with fallback to default
    pub async fn get_or_default<T: DeserializeOwned>(&self, key: &str, default: T) -> Result<T> {
        match self.get(key).await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// Get string setting with fallback
    pub async fn get_string(&self, key: &str, default: &str) -> Result<String> {
        self.get_or_default(key, default.to_string()).await
    }

    /// Get integer setting with fallback
    pub async fn get_int(&self, key: &str, default: i64) -> Result<i64> {
        self.get_or_default(key, default).await
    }

    /// Get unsigned integer setting with fallback
    pub async fn get_uint(&self, key: &str, default: u64) -> Result<u64> {
        let value: i64 = self.get_or_default(key, default as i64).await?;
        Ok(value as u64)
    }

    /// Get boolean setting with fallback
    pub async fn get_bool(&self, key: &str, default: bool) -> Result<bool> {
        self.get_or_default(key, default).await
    }

    /// Get float setting with fallback
    pub async fn get_float(&self, key: &str, default: f64) -> Result<f64> {
        self.get_or_default(key, default).await
    }

    /// Update setting and invalidate cache
    pub async fn set(
        &self,
        key: &str,
        value: String,
        user_id: Uuid,
        change_reason: Option<String>,
        ip_address: Option<String>,
    ) -> Result<()> {
        // Update in database
        let updated =
            SettingsRepository::set(&self.db, key, value, user_id, change_reason, ip_address)
                .await?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(
            key.to_string(),
            CachedSetting {
                value: updated.value,
                value_type: updated.value_type,
            },
        );

        Ok(())
    }

    /// Reload all settings from database (hot reload)
    pub async fn reload(&self) -> Result<()> {
        let settings = SettingsRepository::get_all(&self.db).await?;

        let mut cache = self.cache.write().await;
        cache.clear();

        for setting in settings {
            cache.insert(
                setting.key.clone(),
                CachedSetting {
                    value: setting.value,
                    value_type: setting.value_type,
                },
            );
        }

        let mut last_reload = self.last_reload.write().await;
        *last_reload = Utc::now();

        tracing::debug!(
            "Settings reloaded successfully, {} settings in cache",
            cache.len()
        );

        Ok(())
    }

    /// Get the last reload timestamp
    pub async fn last_reload_time(&self) -> DateTime<Utc> {
        *self.last_reload.read().await
    }

    /// Get the number of cached settings
    pub async fn cache_size(&self) -> usize {
        self.cache.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::setup_test_db;

    #[tokio::test]
    async fn test_settings_service_get() {
        let db = setup_test_db().await;
        let service = SettingsService::new(db)
            .await
            .expect("Failed to create service");

        // Use a setting that exists in the database (runtime-configurable)
        let value: Option<i64> = service
            .get("scanner.scan_timeout_minutes")
            .await
            .expect("Failed to get setting");

        assert_eq!(value, Some(120));
    }

    #[tokio::test]
    async fn test_settings_service_get_or_default() {
        let db = setup_test_db().await;
        let service = SettingsService::new(db)
            .await
            .expect("Failed to create service");

        // Existing setting - use a setting that exists in the database (runtime-configurable)
        let value = service
            .get_int("scanner.scan_timeout_minutes", 10)
            .await
            .expect("Failed to get setting");
        assert_eq!(value, 120);

        // Non-existing setting should return default
        let value = service
            .get_int("nonexistent.setting", 99)
            .await
            .expect("Failed to get setting");
        assert_eq!(value, 99);
    }

    #[tokio::test]
    async fn test_settings_service_reload() {
        let db = setup_test_db().await;
        let service = SettingsService::new(db.clone())
            .await
            .expect("Failed to create service");

        let initial_size = service.cache_size().await;
        assert!(initial_size > 0);

        // Reload should refresh cache
        service.reload().await.expect("Failed to reload");

        let new_size = service.cache_size().await;
        assert_eq!(new_size, initial_size);
    }

    #[tokio::test]
    async fn test_settings_service_update() {
        let db = setup_test_db().await;
        let service = SettingsService::new(db)
            .await
            .expect("Failed to create service");
        let user_id = Uuid::new_v4();

        // Update a setting - use a setting that exists in the database (runtime-configurable)
        service
            .set(
                "scanner.scan_timeout_minutes",
                "240".to_string(),
                user_id,
                Some("Test update".to_string()),
                None,
            )
            .await
            .expect("Failed to update setting");

        // Value should be updated in cache
        let value = service
            .get_int("scanner.scan_timeout_minutes", 120)
            .await
            .expect("Failed to get setting");
        assert_eq!(value, 240);
    }

    #[tokio::test]
    async fn test_settings_service_auto_reload_graceful_shutdown() {
        let db = setup_test_db().await;
        let service = Arc::new(
            SettingsService::new(db)
                .await
                .expect("Failed to create service"),
        );

        // Create a cancellation token
        let cancel_token = CancellationToken::new();

        // Start auto-reload with a short interval (1 second for test)
        let handle = service.clone().start_auto_reload(1, cancel_token.clone());

        // Let it run for a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify the task is still running
        assert!(!handle.is_finished());

        // Cancel and wait for graceful shutdown
        cancel_token.cancel();

        // The task should complete within a reasonable time
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "Auto-reload task did not shutdown in time");
        assert!(
            result.unwrap().is_ok(),
            "Auto-reload task panicked during shutdown"
        );
    }
}
