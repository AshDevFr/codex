//! User Preferences Repository
//!
//! Provides CRUD operations for per-user key-value settings storage.
//!
//! TODO: Remove allow(dead_code) once user preferences feature is fully implemented

#![allow(dead_code)]

use crate::db::entities::{user_preferences, user_preferences::Entity as UserPreferences};
use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

pub struct UserPreferencesRepository;

impl UserPreferencesRepository {
    /// Get a single preference by user_id and key
    pub async fn get_by_key(
        db: &DatabaseConnection,
        user_id: Uuid,
        key: &str,
    ) -> Result<Option<user_preferences::Model>> {
        let preference = UserPreferences::find()
            .filter(user_preferences::Column::UserId.eq(user_id))
            .filter(user_preferences::Column::Key.eq(key))
            .one(db)
            .await?;

        Ok(preference)
    }

    /// Get a typed preference value by key
    /// Returns None if the preference doesn't exist
    pub async fn get_value<T: DeserializeOwned>(
        db: &DatabaseConnection,
        user_id: Uuid,
        key: &str,
    ) -> Result<Option<T>> {
        let preference = Self::get_by_key(db, user_id, key).await?;

        match preference {
            Some(pref) => {
                let value: T = Self::parse_value(&pref.value, &pref.value_type)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Get a typed preference value with a default fallback
    pub async fn get_value_or_default<T: DeserializeOwned>(
        db: &DatabaseConnection,
        user_id: Uuid,
        key: &str,
        default: T,
    ) -> Result<T> {
        match Self::get_value(db, user_id, key).await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }

    /// Get all preferences for a user
    pub async fn get_all_by_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_preferences::Model>> {
        let preferences = UserPreferences::find()
            .filter(user_preferences::Column::UserId.eq(user_id))
            .order_by_asc(user_preferences::Column::Key)
            .all(db)
            .await?;

        Ok(preferences)
    }

    /// Get preferences matching a key prefix (e.g., "ui." for all UI preferences)
    pub async fn get_by_prefix(
        db: &DatabaseConnection,
        user_id: Uuid,
        prefix: &str,
    ) -> Result<Vec<user_preferences::Model>> {
        let preferences = UserPreferences::find()
            .filter(user_preferences::Column::UserId.eq(user_id))
            .filter(user_preferences::Column::Key.starts_with(prefix))
            .order_by_asc(user_preferences::Column::Key)
            .all(db)
            .await?;

        Ok(preferences)
    }

    /// Set a preference value (upsert)
    pub async fn set<T: Serialize>(
        db: &DatabaseConnection,
        user_id: Uuid,
        key: &str,
        value: &T,
    ) -> Result<user_preferences::Model> {
        let (serialized_value, value_type) = Self::serialize_value(value)?;
        Self::set_raw(db, user_id, key, &serialized_value, &value_type).await
    }

    /// Set a raw preference value (upsert) with explicit type
    pub async fn set_raw(
        db: &DatabaseConnection,
        user_id: Uuid,
        key: &str,
        value: &str,
        value_type: &str,
    ) -> Result<user_preferences::Model> {
        let existing = Self::get_by_key(db, user_id, key).await?;
        let now = Utc::now();

        if let Some(existing_model) = existing {
            // Update existing preference
            let mut active_model: user_preferences::ActiveModel = existing_model.into();
            active_model.value = Set(value.to_string());
            active_model.value_type = Set(value_type.to_string());
            active_model.updated_at = Set(now);

            let result = active_model.update(db).await?;
            Ok(result)
        } else {
            // Create new preference
            let new_preference = user_preferences::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_id),
                key: Set(key.to_string()),
                value: Set(value.to_string()),
                value_type: Set(value_type.to_string()),
                created_at: Set(now),
                updated_at: Set(now),
            };

            let result = new_preference.insert(db).await?;
            Ok(result)
        }
    }

    /// Set multiple preferences at once (bulk upsert)
    pub async fn set_bulk(
        db: &DatabaseConnection,
        user_id: Uuid,
        preferences: Vec<(String, serde_json::Value)>,
    ) -> Result<Vec<user_preferences::Model>> {
        let mut results = Vec::with_capacity(preferences.len());

        for (key, value) in preferences {
            let (serialized_value, value_type) = Self::serialize_json_value(&value)?;
            let result = Self::set_raw(db, user_id, &key, &serialized_value, &value_type).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Delete a preference by key
    pub async fn delete(db: &DatabaseConnection, user_id: Uuid, key: &str) -> Result<bool> {
        let result = UserPreferences::delete_many()
            .filter(user_preferences::Column::UserId.eq(user_id))
            .filter(user_preferences::Column::Key.eq(key))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Delete all preferences for a user
    pub async fn delete_all_by_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let result = UserPreferences::delete_many()
            .filter(user_preferences::Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    /// Delete preferences matching a key prefix
    pub async fn delete_by_prefix(
        db: &DatabaseConnection,
        user_id: Uuid,
        prefix: &str,
    ) -> Result<u64> {
        let result = UserPreferences::delete_many()
            .filter(user_preferences::Column::UserId.eq(user_id))
            .filter(user_preferences::Column::Key.starts_with(prefix))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    /// Parse a stored value into the requested type
    pub fn parse_value<T: DeserializeOwned>(value: &str, value_type: &str) -> Result<T> {
        match value_type {
            "string" => serde_json::from_value(serde_json::Value::String(value.to_string()))
                .map_err(|e| anyhow!("Failed to parse string value: {}", e)),
            "integer" => {
                let int_val: i64 = value
                    .parse()
                    .map_err(|e| anyhow!("Failed to parse integer value: {}", e))?;
                serde_json::from_value(serde_json::Value::Number(int_val.into()))
                    .map_err(|e| anyhow!("Failed to convert integer value: {}", e))
            }
            "float" => {
                let float_val: f64 = value
                    .parse()
                    .map_err(|e| anyhow!("Failed to parse float value: {}", e))?;
                let num = serde_json::Number::from_f64(float_val)
                    .ok_or_else(|| anyhow!("Invalid float value"))?;
                serde_json::from_value(serde_json::Value::Number(num))
                    .map_err(|e| anyhow!("Failed to convert float value: {}", e))
            }
            "boolean" => {
                let bool_val: bool = value
                    .parse()
                    .map_err(|e| anyhow!("Failed to parse boolean value: {}", e))?;
                serde_json::from_value(serde_json::Value::Bool(bool_val))
                    .map_err(|e| anyhow!("Failed to convert boolean value: {}", e))
            }
            "json" => serde_json::from_str(value)
                .map_err(|e| anyhow!("Failed to parse JSON value: {}", e)),
            _ => Err(anyhow!("Unknown value type: {}", value_type)),
        }
    }

    /// Serialize a value and determine its type
    fn serialize_value<T: Serialize>(value: &T) -> Result<(String, String)> {
        let json_value = serde_json::to_value(value)?;
        Self::serialize_json_value(&json_value)
    }

    /// Serialize a JSON value and determine its type
    fn serialize_json_value(value: &serde_json::Value) -> Result<(String, String)> {
        match value {
            serde_json::Value::String(s) => Ok((s.clone(), "string".to_string())),
            serde_json::Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    Ok((n.to_string(), "integer".to_string()))
                } else {
                    Ok((n.to_string(), "float".to_string()))
                }
            }
            serde_json::Value::Bool(b) => Ok((b.to_string(), "boolean".to_string())),
            _ => Ok((serde_json::to_string(value)?, "json".to_string())),
        }
    }

    /// Convert a model's value to a serde_json::Value
    pub fn to_json_value(model: &user_preferences::Model) -> Result<serde_json::Value> {
        match model.value_type.as_str() {
            "string" => Ok(serde_json::Value::String(model.value.clone())),
            "integer" => {
                let int_val: i64 = model
                    .value
                    .parse()
                    .map_err(|e| anyhow!("Failed to parse integer: {}", e))?;
                Ok(serde_json::Value::Number(int_val.into()))
            }
            "float" => {
                let float_val: f64 = model
                    .value
                    .parse()
                    .map_err(|e| anyhow!("Failed to parse float: {}", e))?;
                let num = serde_json::Number::from_f64(float_val)
                    .ok_or_else(|| anyhow!("Invalid float value"))?;
                Ok(serde_json::Value::Number(num))
            }
            "boolean" => {
                let bool_val: bool = model
                    .value
                    .parse()
                    .map_err(|e| anyhow!("Failed to parse boolean: {}", e))?;
                Ok(serde_json::Value::Bool(bool_val))
            }
            "json" => serde_json::from_str(&model.value)
                .map_err(|e| anyhow!("Failed to parse JSON: {}", e)),
            _ => Err(anyhow!("Unknown value type: {}", model.value_type)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::permissions::ADMIN_PERMISSIONS;
    use crate::db::entities::users;
    use crate::db::repositories::UserRepository;
    use crate::db::test_helpers::setup_test_db;
    use crate::utils::password;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        let permissions_vec: Vec<_> = ADMIN_PERMISSIONS.iter().cloned().collect();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("testuser_{}", Uuid::new_v4()),
            email: format!("test_{}@example.com", Uuid::new_v4()),
            password_hash,
            is_admin: true,
            is_active: true,
            email_verified: false,
            permissions: serde_json::to_value(&permissions_vec).unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    #[tokio::test]
    async fn test_set_and_get_string_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set a string preference
        let result = UserPreferencesRepository::set(&db, user.id, "ui.theme", &"dark")
            .await
            .unwrap();

        assert_eq!(result.key, "ui.theme");
        assert_eq!(result.value, "dark");
        assert_eq!(result.value_type, "string");

        // Get the preference
        let value: Option<String> = UserPreferencesRepository::get_value(&db, user.id, "ui.theme")
            .await
            .unwrap();

        assert_eq!(value, Some("dark".to_string()));
    }

    #[tokio::test]
    async fn test_set_and_get_integer_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set an integer preference
        let result = UserPreferencesRepository::set(&db, user.id, "reader.default_zoom", &100i64)
            .await
            .unwrap();

        assert_eq!(result.key, "reader.default_zoom");
        assert_eq!(result.value, "100");
        assert_eq!(result.value_type, "integer");

        // Get the preference
        let value: Option<i64> =
            UserPreferencesRepository::get_value(&db, user.id, "reader.default_zoom")
                .await
                .unwrap();

        assert_eq!(value, Some(100));
    }

    #[tokio::test]
    async fn test_set_and_get_boolean_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set a boolean preference
        let result = UserPreferencesRepository::set(&db, user.id, "ui.sidebar_collapsed", &true)
            .await
            .unwrap();

        assert_eq!(result.key, "ui.sidebar_collapsed");
        assert_eq!(result.value, "true");
        assert_eq!(result.value_type, "boolean");

        // Get the preference
        let value: Option<bool> =
            UserPreferencesRepository::get_value(&db, user.id, "ui.sidebar_collapsed")
                .await
                .unwrap();

        assert_eq!(value, Some(true));
    }

    #[tokio::test]
    async fn test_set_and_get_json_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set a JSON preference
        let json_value = serde_json::json!({
            "recent_series": ["uuid1", "uuid2"],
            "filters": {"status": "reading"}
        });

        let result = UserPreferencesRepository::set(&db, user.id, "library.state", &json_value)
            .await
            .unwrap();

        assert_eq!(result.key, "library.state");
        assert_eq!(result.value_type, "json");

        // Get the preference
        let value: Option<serde_json::Value> =
            UserPreferencesRepository::get_value(&db, user.id, "library.state")
                .await
                .unwrap();

        assert!(value.is_some());
        let v = value.unwrap();
        assert_eq!(v["recent_series"][0], "uuid1");
    }

    #[tokio::test]
    async fn test_get_value_or_default() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Get non-existent preference with default
        let value: i64 =
            UserPreferencesRepository::get_value_or_default(&db, user.id, "non.existent", 42)
                .await
                .unwrap();

        assert_eq!(value, 42);

        // Set the preference
        UserPreferencesRepository::set(&db, user.id, "non.existent", &100i64)
            .await
            .unwrap();

        // Now it should return the actual value
        let value: i64 =
            UserPreferencesRepository::get_value_or_default(&db, user.id, "non.existent", 42)
                .await
                .unwrap();

        assert_eq!(value, 100);
    }

    #[tokio::test]
    async fn test_update_existing_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set initial value
        UserPreferencesRepository::set(&db, user.id, "ui.theme", &"light")
            .await
            .unwrap();

        // Update value
        let updated = UserPreferencesRepository::set(&db, user.id, "ui.theme", &"dark")
            .await
            .unwrap();

        assert_eq!(updated.value, "dark");

        // Verify only one record exists
        let all = UserPreferencesRepository::get_all_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_get_all_by_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set multiple preferences
        UserPreferencesRepository::set(&db, user.id, "ui.theme", &"dark")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user.id, "ui.language", &"en")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user.id, "reader.zoom", &100i64)
            .await
            .unwrap();

        // Get all preferences
        let all = UserPreferencesRepository::get_all_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(all.len(), 3);
        // Should be sorted by key
        assert_eq!(all[0].key, "reader.zoom");
        assert_eq!(all[1].key, "ui.language");
        assert_eq!(all[2].key, "ui.theme");
    }

    #[tokio::test]
    async fn test_get_by_prefix() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set preferences in different categories
        UserPreferencesRepository::set(&db, user.id, "ui.theme", &"dark")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user.id, "ui.language", &"en")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user.id, "reader.zoom", &100i64)
            .await
            .unwrap();

        // Get only UI preferences
        let ui_prefs = UserPreferencesRepository::get_by_prefix(&db, user.id, "ui.")
            .await
            .unwrap();

        assert_eq!(ui_prefs.len(), 2);
        assert!(ui_prefs.iter().all(|p| p.key.starts_with("ui.")));
    }

    #[tokio::test]
    async fn test_delete_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set a preference
        UserPreferencesRepository::set(&db, user.id, "ui.theme", &"dark")
            .await
            .unwrap();

        // Delete it
        let deleted = UserPreferencesRepository::delete(&db, user.id, "ui.theme")
            .await
            .unwrap();

        assert!(deleted);

        // Verify it's gone
        let value: Option<String> = UserPreferencesRepository::get_value(&db, user.id, "ui.theme")
            .await
            .unwrap();

        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_delete_non_existent_preference() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Try to delete non-existent preference
        let deleted = UserPreferencesRepository::delete(&db, user.id, "non.existent")
            .await
            .unwrap();

        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_delete_by_prefix() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set preferences in different categories
        UserPreferencesRepository::set(&db, user.id, "ui.theme", &"dark")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user.id, "ui.language", &"en")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user.id, "reader.zoom", &100i64)
            .await
            .unwrap();

        // Delete all UI preferences
        let deleted = UserPreferencesRepository::delete_by_prefix(&db, user.id, "ui.")
            .await
            .unwrap();

        assert_eq!(deleted, 2);

        // Verify UI preferences are gone but reader preference remains
        let all = UserPreferencesRepository::get_all_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(all.len(), 1);
        assert_eq!(all[0].key, "reader.zoom");
    }

    #[tokio::test]
    async fn test_set_bulk() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Set multiple preferences at once
        let preferences = vec![
            ("ui.theme".to_string(), serde_json::json!("dark")),
            ("ui.language".to_string(), serde_json::json!("en")),
            ("reader.zoom".to_string(), serde_json::json!(100)),
        ];

        let results = UserPreferencesRepository::set_bulk(&db, user.id, preferences)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);

        // Verify all preferences exist
        let all = UserPreferencesRepository::get_all_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_to_json_value() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Test string
        let pref = UserPreferencesRepository::set(&db, user.id, "test.string", &"hello")
            .await
            .unwrap();
        let json = UserPreferencesRepository::to_json_value(&pref).unwrap();
        assert_eq!(json, serde_json::json!("hello"));

        // Test integer
        let pref = UserPreferencesRepository::set(&db, user.id, "test.int", &42i64)
            .await
            .unwrap();
        let json = UserPreferencesRepository::to_json_value(&pref).unwrap();
        assert_eq!(json, serde_json::json!(42));

        // Test boolean
        let pref = UserPreferencesRepository::set(&db, user.id, "test.bool", &true)
            .await
            .unwrap();
        let json = UserPreferencesRepository::to_json_value(&pref).unwrap();
        assert_eq!(json, serde_json::json!(true));
    }

    #[tokio::test]
    async fn test_user_isolation() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        // Set same key for both users
        UserPreferencesRepository::set(&db, user1.id, "ui.theme", &"dark")
            .await
            .unwrap();
        UserPreferencesRepository::set(&db, user2.id, "ui.theme", &"light")
            .await
            .unwrap();

        // Verify isolation
        let val1: Option<String> = UserPreferencesRepository::get_value(&db, user1.id, "ui.theme")
            .await
            .unwrap();
        let val2: Option<String> = UserPreferencesRepository::get_value(&db, user2.id, "ui.theme")
            .await
            .unwrap();

        assert_eq!(val1, Some("dark".to_string()));
        assert_eq!(val2, Some("light".to_string()));
    }
}
