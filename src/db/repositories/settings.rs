use crate::db::entities::{
    settings, settings::Entity as Setting, settings_history,
    settings_history::Entity as SettingHistory,
};
use anyhow::{Result, anyhow};
use chrono::Utc;
use sea_orm::*;
use serde::de::DeserializeOwned;
use uuid::Uuid;

pub struct SettingsRepository;

impl SettingsRepository {
    /// Get a single setting by key
    pub async fn get(db: &DatabaseConnection, key: &str) -> Result<Option<settings::Model>> {
        let setting = Setting::find()
            .filter(settings::Column::Key.eq(key))
            .filter(settings::Column::DeletedAt.is_null())
            .one(db)
            .await?;
        Ok(setting)
    }

    /// Get all settings in a category
    pub async fn get_by_category(
        db: &DatabaseConnection,
        category: &str,
    ) -> Result<Vec<settings::Model>> {
        let settings = Setting::find()
            .filter(settings::Column::Category.eq(category))
            .filter(settings::Column::DeletedAt.is_null())
            .all(db)
            .await?;
        Ok(settings)
    }

    /// Get all settings (non-deleted)
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<settings::Model>> {
        let settings = Setting::find()
            .filter(settings::Column::DeletedAt.is_null())
            .order_by_asc(settings::Column::Category)
            .order_by_asc(settings::Column::Key)
            .all(db)
            .await?;
        Ok(settings)
    }

    /// Set a setting value with audit trail
    pub async fn set(
        db: &DatabaseConnection,
        key: &str,
        value: String,
        updated_by: Uuid,
        change_reason: Option<String>,
        ip_address: Option<String>,
    ) -> Result<settings::Model> {
        // Start a transaction
        let txn = db.begin().await?;

        // Get existing setting
        let existing = Setting::find()
            .filter(settings::Column::Key.eq(key))
            .filter(settings::Column::DeletedAt.is_null())
            .one(&txn)
            .await?;

        let setting = if let Some(existing_setting) = existing {
            // Validate value against setting constraints
            Self::validate_value(&existing_setting, &value)?;

            // Create history record
            let history = settings_history::ActiveModel {
                id: Set(Uuid::new_v4()),
                setting_id: Set(existing_setting.id),
                key: Set(existing_setting.key.clone()),
                old_value: Set(Some(existing_setting.value.clone())),
                new_value: Set(value.clone()),
                changed_by: Set(updated_by),
                changed_at: Set(Utc::now()),
                change_reason: Set(change_reason),
                ip_address: Set(ip_address),
            };
            history.insert(&txn).await?;

            // Update setting
            let mut active_model: settings::ActiveModel = existing_setting.into();
            active_model.value = Set(value);
            active_model.updated_at = Set(Utc::now());
            active_model.updated_by = Set(Some(updated_by));
            active_model.version = Set(active_model.version.unwrap() + 1);
            active_model.update(&txn).await?
        } else {
            return Err(anyhow!("Setting '{}' not found", key));
        };

        // Commit transaction
        txn.commit().await?;

        Ok(setting)
    }

    /// Bulk update settings (transactional)
    pub async fn bulk_update(
        db: &DatabaseConnection,
        updates: Vec<(String, String)>, // (key, value) pairs
        updated_by: Uuid,
        change_reason: Option<String>,
        ip_address: Option<String>,
    ) -> Result<Vec<settings::Model>> {
        let txn = db.begin().await?;
        let mut results = Vec::new();

        for (key, value) in updates {
            let existing = Setting::find()
                .filter(settings::Column::Key.eq(&key))
                .filter(settings::Column::DeletedAt.is_null())
                .one(&txn)
                .await?;

            if let Some(existing_setting) = existing {
                // Validate value against setting constraints
                Self::validate_value(&existing_setting, &value)?;

                // Create history record
                let history = settings_history::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    setting_id: Set(existing_setting.id),
                    key: Set(existing_setting.key.clone()),
                    old_value: Set(Some(existing_setting.value.clone())),
                    new_value: Set(value.clone()),
                    changed_by: Set(updated_by),
                    changed_at: Set(Utc::now()),
                    change_reason: Set(change_reason.clone()),
                    ip_address: Set(ip_address.clone()),
                };
                history.insert(&txn).await?;

                // Update setting
                let mut active_model: settings::ActiveModel = existing_setting.into();
                active_model.value = Set(value);
                active_model.updated_at = Set(Utc::now());
                active_model.updated_by = Set(Some(updated_by));
                active_model.version = Set(active_model.version.unwrap() + 1);
                let updated = active_model.update(&txn).await?;
                results.push(updated);
            } else {
                txn.rollback().await?;
                return Err(anyhow!("Setting '{}' not found", key));
            }
        }

        txn.commit().await?;
        Ok(results)
    }

    /// Reset to default value
    pub async fn reset_to_default(
        db: &DatabaseConnection,
        key: &str,
        updated_by: Uuid,
        change_reason: Option<String>,
        ip_address: Option<String>,
    ) -> Result<settings::Model> {
        let setting = Setting::find()
            .filter(settings::Column::Key.eq(key))
            .filter(settings::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("Setting '{}' not found", key))?;

        let default_value = setting.default_value.clone();
        Self::set(
            db,
            key,
            default_value,
            updated_by,
            change_reason.or(Some("Reset to default".to_string())),
            ip_address,
        )
        .await
    }

    /// Get typed value with fallback to default
    pub async fn get_value<T: DeserializeOwned>(
        db: &DatabaseConnection,
        key: &str,
    ) -> Result<Option<T>> {
        let setting = Self::get(db, key).await?;

        if let Some(s) = setting {
            let value: T = Self::parse_value(&s.value, &s.value_type)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Get setting history
    pub async fn get_history(
        db: &DatabaseConnection,
        key: &str,
        limit: Option<u64>,
    ) -> Result<Vec<settings_history::Model>> {
        let setting = Self::get(db, key).await?;

        if let Some(s) = setting {
            let mut query = SettingHistory::find()
                .filter(settings_history::Column::SettingId.eq(s.id))
                .order_by_desc(settings_history::Column::ChangedAt);

            if let Some(l) = limit {
                query = query.limit(l);
            }

            let history = query.all(db).await?;
            Ok(history)
        } else {
            Ok(Vec::new())
        }
    }

    /// Validate value against rules
    pub fn validate_value(setting: &settings::Model, value: &str) -> Result<()> {
        match setting.value_type.as_str() {
            "Integer" => {
                let int_value: i64 = value
                    .parse()
                    .map_err(|_| anyhow!("Invalid integer value: {}", value))?;

                if let Some(min) = setting.min_value
                    && int_value < min
                {
                    return Err(anyhow!("Value {} is below minimum {}", int_value, min));
                }

                if let Some(max) = setting.max_value
                    && int_value > max
                {
                    return Err(anyhow!("Value {} is above maximum {}", int_value, max));
                }
            }
            "Float" => {
                let _: f64 = value
                    .parse()
                    .map_err(|_| anyhow!("Invalid float value: {}", value))?;
            }
            "Boolean" => {
                if value != "true" && value != "false" {
                    return Err(anyhow!("Invalid boolean value: {}", value));
                }
            }
            "String" => {
                // String values are always valid
            }
            _ => {
                // For other types (Array, Object), assume JSON parsing will validate
            }
        }

        Ok(())
    }

    /// Parse value from string based on type
    pub fn parse_value<T: DeserializeOwned>(value: &str, value_type: &str) -> Result<T> {
        match value_type {
            "Integer" | "Float" | "String" => serde_json::from_str(&format!("\"{}\"", value))
                .or_else(|_| serde_json::from_str(value))
                .map_err(|e| anyhow!("Failed to parse value: {}", e)),
            "Boolean" => {
                serde_json::from_str(value).map_err(|e| anyhow!("Failed to parse boolean: {}", e))
            }
            "Array" | "Object" => {
                serde_json::from_str(value).map_err(|e| anyhow!("Failed to parse JSON: {}", e))
            }
            _ => Err(anyhow!("Unknown value type: {}", value_type)),
        }
    }

    /// Get the application name from settings with fallback to default
    ///
    /// This is a convenience method for retrieving the `application.name` setting.
    /// Returns "Codex" if the setting is not found or an error occurs.
    pub async fn get_app_name(db: &DatabaseConnection) -> String {
        Self::get(db, "application.name")
            .await
            .ok()
            .flatten()
            .map(|s| s.value)
            .unwrap_or_else(|| "Codex".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::setup_test_db;

    #[tokio::test]
    async fn test_get_setting() {
        let db = setup_test_db().await;

        // The seeder should have created default settings
        // Use a setting that exists in the database (runtime-configurable)
        let setting = SettingsRepository::get(&db, "scanner.scan_timeout_minutes")
            .await
            .expect("Failed to get setting");

        assert!(setting.is_some());
        let s = setting.unwrap();
        assert_eq!(s.key, "scanner.scan_timeout_minutes");
        assert_eq!(s.value, "120");
    }

    #[tokio::test]
    async fn test_update_setting() {
        let db = setup_test_db().await;

        // Get the setting first - use a setting that exists in the database (runtime-configurable)
        let setting = SettingsRepository::get(&db, "scanner.scan_timeout_minutes")
            .await
            .expect("Failed to get setting")
            .expect("Setting not found");

        // Update without user_id (tests don't have users table populated)
        // In real usage, updated_by would reference a valid user
        let mut active: settings::ActiveModel = setting.into();
        active.value = sea_orm::Set("240".to_string());
        active.version = sea_orm::Set(2);
        active.updated_at = sea_orm::Set(chrono::Utc::now());

        let updated = active.update(&db).await.expect("Failed to update");

        assert_eq!(updated.value, "240");
        assert_eq!(updated.version, 2);
    }

    #[tokio::test]
    async fn test_validation() {
        let db = setup_test_db().await;
        let user_id = Uuid::new_v4();

        // Try to set value above maximum
        let result = SettingsRepository::set(
            &db,
            "scanner.max_concurrent_scans",
            "100".to_string(), // Max is 10
            user_id,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
    }
}
