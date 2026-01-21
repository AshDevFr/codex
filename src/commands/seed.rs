use crate::api::permissions::{
    serialize_permissions, ADMIN_PERMISSIONS, MAINTAINER_PERMISSIONS, READER_PERMISSIONS,
};
use crate::config::{Config, EnvOverride};
use crate::db::entities::{api_keys, users};
use crate::db::repositories::{api_key::ApiKeyRepository, user::UserRepository};
use crate::db::Database;
use crate::utils::password::hash_password;
use anyhow::{Context, Result};
use chrono::Utc;
use rand::Rng;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

/// Seed command handler - creates initial admin user and API key
pub async fn seed_command(config_path: PathBuf) -> Result<()> {
    // Load configuration
    let mut config = Config::from_file(config_path.to_str().unwrap())?;
    config.apply_env_overrides("CODEX");

    // Initialize database connection
    let db = Database::new(&config.database).await?;

    // Run migrations to ensure database schema is up to date
    db.run_migrations()
        .await
        .context("Failed to run database migrations")?;

    let db_conn = db.sea_orm_connection();

    // Check if admin user already exists
    let existing_admin = UserRepository::get_by_username(db_conn, "admin").await?;

    if existing_admin.is_some() {
        warn!("Admin user already exists. Skipping seed.");
        println!("\n⚠️  Admin user already exists!");
        println!("If you need to reset the admin credentials, please delete the user first.\n");
        return Ok(());
    }

    use crate::api::permissions::UserRole;

    // Define users to create: (username, email, role, permissions for API key)
    let users_to_create = [
        (
            "admin",
            "admin@localhost",
            UserRole::Admin,
            &*ADMIN_PERMISSIONS,
        ),
        (
            "maintainer",
            "maintainer@localhost",
            UserRole::Maintainer,
            &*MAINTAINER_PERMISSIONS,
        ),
        (
            "reader",
            "reader@localhost",
            UserRole::Reader,
            &*READER_PERMISSIONS,
        ),
    ];

    let mut credentials: Vec<(String, String, String)> = Vec::new();

    for (username, email, role, permissions) in users_to_create {
        // Generate random password
        let password = generate_random_password(16);
        let password_hash =
            hash_password(&password).context(format!("Failed to hash {} password", username))?;

        // Create user
        info!("Creating {} user...", username);
        let user = users::Model {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash,
            role: role.to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]), // Custom permissions (empty = use role defaults)
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };

        let created_user = UserRepository::create(db_conn, &user)
            .await
            .context(format!("Failed to create {} user", username))?;

        info!(
            "{} user created successfully: {}",
            username, created_user.id
        );

        // Generate API key
        info!("Generating {} API key...", username);
        let (api_key_plain, api_key_model) = generate_api_key(
            created_user.id,
            format!("Initial {} Key", username.to_uppercase()),
            permissions,
        )?;

        ApiKeyRepository::create(db_conn, &api_key_model)
            .await
            .context(format!("Failed to create {} API key", username))?;

        info!("{} API key created successfully", username);

        credentials.push((username.to_string(), password, api_key_plain));
    }

    // Print credentials to console (once-only view)
    println!("\n========================================");
    println!("🎉 Codex Users Created!");
    println!("========================================\n");

    for (username, password, api_key) in &credentials {
        println!("User: {}", username);
        println!("  Password: {}", password);
        println!("  API Key:  {}\n", api_key);
    }

    println!("========================================");
    println!("⚠️  IMPORTANT: Save these credentials now!");
    println!("   They will NOT be shown again.");
    println!("========================================\n");

    Ok(())
}

/// Generate a random password of specified length
fn generate_random_password(length: usize) -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    let mut rng = rand::thread_rng();

    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate an API key and return both the plaintext key and the model to store
///
/// API key format: codex_<16hex>_<32hex>
/// Returns: (plaintext_key, api_key_model)
fn generate_api_key(
    user_id: Uuid,
    name: String,
    permissions: &std::collections::HashSet<crate::api::permissions::Permission>,
) -> Result<(String, api_keys::Model)> {
    let mut rng = rand::thread_rng();

    // Generate random components
    let prefix_random: String = (0..16)
        .map(|_| format!("{:x}", rng.gen::<u8>() % 16))
        .collect();
    let suffix_random: String = (0..32)
        .map(|_| format!("{:x}", rng.gen::<u8>() % 16))
        .collect();

    // Construct full key
    let api_key = format!("codex_{}_{}", prefix_random, suffix_random);

    // Hash the full key for storage
    let key_hash = hash_password(&api_key).context("Failed to hash API key")?;

    // Store prefix for lookup (must match auth extractor logic)
    let key_prefix = format!("codex_{}", prefix_random);

    let permissions_json = serialize_permissions(permissions);
    let api_key_model = api_keys::Model {
        id: Uuid::new_v4(),
        user_id,
        name,
        key_hash,
        key_prefix,
        permissions: serde_json::from_str(&permissions_json)
            .unwrap_or_else(|_| serde_json::json!([])),
        is_active: true,
        expires_at: None,
        last_used_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    Ok((api_key, api_key_model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_password() {
        let password = generate_random_password(16);
        assert_eq!(password.len(), 16);

        // Should be different each time
        let password2 = generate_random_password(16);
        assert_ne!(password, password2);
    }

    #[test]
    fn test_generate_api_key() {
        let user_id = Uuid::new_v4();
        let mut permissions = std::collections::HashSet::new();
        permissions.insert(crate::api::permissions::Permission::LibrariesRead);

        let (api_key, model) =
            generate_api_key(user_id, "Test Key".to_string(), &permissions).unwrap();

        // Check format
        assert!(api_key.starts_with("codex_"));
        let parts: Vec<&str> = api_key.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "codex");
        assert_eq!(parts[1].len(), 16); // 16 hex chars
        assert_eq!(parts[2].len(), 32); // 32 hex chars

        // Check model
        assert_eq!(model.user_id, user_id);
        assert_eq!(model.name, "Test Key");
        assert!(model.key_prefix.starts_with("codex_"));
        // Verify prefix is the full first two parts (codex_<16 hex chars>)
        assert_eq!(model.key_prefix, format!("codex_{}", parts[1]));
        assert_eq!(model.key_prefix.len(), 22); // "codex_" (6) + 16 hex chars
        assert!(model.is_active);
    }
}
