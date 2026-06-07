use anyhow::{Context, Result};
use chrono::Utc;
use codex_api::permissions::{
    ADMIN_PERMISSIONS, MAINTAINER_PERMISSIONS, READER_PERMISSIONS, serialize_permissions,
};
use codex_config::{Config, EnvOverride};
use codex_db::Database;
use codex_db::entities::{api_keys, plugins::PluginPermission, users};
use codex_db::repositories::{
    api_key::ApiKeyRepository, library::CreateLibraryParams, library::LibraryRepository,
    plugins::PluginsRepository, user::UserRepository,
};
use codex_models::preprocessing::{AutoMatchConditions, PreprocessingRule};
use codex_models::{BookStrategy, NumberStrategy, SeriesStrategy};
use codex_services::plugin::protocol::PluginScope;
use codex_utils::password::hash_password;
use rand::RngExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

// =============================================================================
// Seed Config Types
// =============================================================================

/// Seed configuration loaded from a YAML file.
///
/// All sections are optional — you can seed just users, just plugins, or any combination.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SeedConfig {
    /// User password overrides (optional — random passwords if omitted)
    pub users: HashMap<String, SeedUserConfig>,
    /// Plugins to register
    pub plugins: Vec<SeedPluginConfig>,
    /// Libraries to create
    pub libraries: Vec<SeedLibraryConfig>,
}

/// Per-user seed configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SeedUserConfig {
    pub password: String,
}

/// Plugin seed configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SeedPluginConfig {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_plugin_type")]
    pub plugin_type: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub scopes: Vec<PluginScope>,
    #[serde(default = "default_credential_delivery")]
    pub credential_delivery: String,
    #[serde(default)]
    pub credentials: Option<serde_json::Value>,
    /// Optional admin-side plugin configuration (the same JSON object that
    /// the user would paste into "Configuration" in the plugin edit dialog).
    /// Persisted on the plugin row so the plugin process receives it via
    /// `InitializeParams.adminConfig` on first start.
    #[serde(default, alias = "admin_config")]
    pub config: Option<serde_json::Value>,
    /// Libraries (by name) this plugin is scoped to. Empty/absent = all
    /// libraries (matching `plugins.library_ids`). Names are resolved to
    /// library IDs at seed time, so the referenced libraries must be declared
    /// in the same seed config (libraries are seeded before plugins).
    #[serde(default)]
    pub libraries: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_plugin_type() -> String {
    "system".to_string()
}

fn default_credential_delivery() -> String {
    "init_message".to_string()
}

fn default_true() -> bool {
    true
}

/// Library seed configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SeedLibraryConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub series_strategy: Option<SeriesStrategy>,
    #[serde(default)]
    pub series_config: Option<serde_json::Value>,
    #[serde(default)]
    pub book_strategy: Option<BookStrategy>,
    #[serde(default)]
    pub book_config: Option<serde_json::Value>,
    #[serde(default)]
    pub number_strategy: Option<NumberStrategy>,
    #[serde(default)]
    pub number_config: Option<serde_json::Value>,
    #[serde(default)]
    pub default_reading_direction: Option<String>,
    #[serde(default)]
    pub allowed_formats: Option<Vec<String>>,
    #[serde(default)]
    pub excluded_patterns: Option<Vec<String>>,
    /// Title preprocessing rules, written as native YAML. Serialized to the JSON
    /// string the `libraries` table stores at seed time.
    #[serde(default)]
    pub title_preprocessing_rules: Option<Vec<PreprocessingRule>>,
    /// Auto-match conditions, written as native YAML. Serialized to the JSON
    /// string the `libraries` table stores at seed time.
    #[serde(default)]
    pub auto_match_conditions: Option<AutoMatchConditions>,
}

impl SeedConfig {
    /// Load seed config from a YAML file
    pub fn from_file(path: &str) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .context(format!("Failed to read seed config: {}", path))?;
        let config: SeedConfig =
            serde_yaml::from_str(&contents).context("Failed to parse seed config YAML")?;
        Ok(config)
    }
}

// =============================================================================
// Seed Command
// =============================================================================

/// Seed command handler - creates initial admin user and API key
pub async fn seed_command(config_path: PathBuf, seed_config_path: Option<PathBuf>) -> Result<()> {
    // Load seed config if provided
    let seed_config = if let Some(ref path) = seed_config_path {
        let config = SeedConfig::from_file(path.to_str().unwrap())?;
        info!("Loaded seed config from {}", path.display());
        Some(config)
    } else {
        None
    };

    // Load application configuration
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
        warn!("Admin user already exists. Skipping user creation.");
        println!("\n⚠️  Admin user already exists! Skipping user creation.");
        println!("If you need to reset the admin credentials, please delete the user first.\n");
    } else {
        seed_users(db_conn, seed_config.as_ref()).await?;
    }

    // Seed libraries and plugins (these are idempotent, always attempt).
    // Libraries are seeded BEFORE plugins so that a plugin's `libraries` scope
    // (referenced by name) can be resolved to library IDs.
    if let Some(ref seed_cfg) = seed_config {
        if !seed_cfg.libraries.is_empty() {
            seed_libraries(db_conn, &seed_cfg.libraries).await?;
        }

        if !seed_cfg.plugins.is_empty() {
            seed_plugins(db_conn, &seed_cfg.plugins).await?;
        }
    }

    Ok(())
}

// =============================================================================
// User Seeding
// =============================================================================

async fn seed_users(
    db_conn: &sea_orm::DatabaseConnection,
    seed_config: Option<&SeedConfig>,
) -> Result<()> {
    use codex_api::permissions::UserRole;

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
        // Use password from seed config if provided, otherwise generate random
        let password = seed_config
            .and_then(|cfg| cfg.users.get(username))
            .map(|u| u.password.clone())
            .unwrap_or_else(|| generate_random_password(16));

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
            permissions: serde_json::json!([]),
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

// =============================================================================
// Plugin Seeding
// =============================================================================

async fn seed_plugins(
    db_conn: &sea_orm::DatabaseConnection,
    plugins: &[SeedPluginConfig],
) -> Result<()> {
    println!("\n----------------------------------------");
    println!("📦 Seeding Plugins...");
    println!("----------------------------------------\n");

    let mut created = 0;
    let mut skipped = 0;

    // Build a library name → id index so plugins can be scoped by library name.
    // Libraries are seeded first, so this reflects the just-seeded set.
    let library_index: HashMap<String, Uuid> = LibraryRepository::list_all(db_conn)
        .await?
        .into_iter()
        .map(|lib| (lib.name, lib.id))
        .collect();

    for plugin_cfg in plugins {
        // Resolve the plugin's library scope (by name) to IDs. Empty = all.
        let library_ids: Vec<Uuid> = plugin_cfg
            .libraries
            .iter()
            .map(|name| {
                library_index.get(name).copied().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Plugin '{}' references unknown library '{}'. Declare the library in the seed config's `libraries` section.",
                        plugin_cfg.name,
                        name
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Check if plugin already exists by name
        let existing = PluginsRepository::get_by_name(db_conn, &plugin_cfg.name).await?;

        if existing.is_some() {
            println!(
                "  ⏭  Plugin '{}' already exists, skipping.",
                plugin_cfg.name
            );
            skipped += 1;
            continue;
        }

        info!("Creating plugin '{}'...", plugin_cfg.name);

        let env_pairs: Vec<(String, String)> = plugin_cfg
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        PluginsRepository::create(
            db_conn,
            &plugin_cfg.name,
            &plugin_cfg.display_name,
            plugin_cfg.description.as_deref(),
            &plugin_cfg.plugin_type,
            &plugin_cfg.command,
            plugin_cfg.args.clone(),
            env_pairs,
            None, // working_directory
            plugin_cfg.permissions.clone(),
            plugin_cfg.scopes.clone(),
            library_ids,                     // library scope (empty = all libraries)
            plugin_cfg.credentials.as_ref(), // credentials
            &plugin_cfg.credential_delivery, // credential_delivery
            plugin_cfg.config.clone(),       // admin config
            plugin_cfg.enabled,
            None, // created_by
            None, // rate_limit_requests_per_minute
            None, // request_timeout_seconds
        )
        .await
        .context(format!("Failed to create plugin '{}'", plugin_cfg.name))?;

        println!("  ✅ Plugin '{}' created.", plugin_cfg.name);
        created += 1;
    }

    println!(
        "\nPlugins: {} created, {} skipped (already exist).",
        created, skipped
    );

    Ok(())
}

// =============================================================================
// Library Seeding
// =============================================================================

async fn seed_libraries(
    db_conn: &sea_orm::DatabaseConnection,
    libraries: &[SeedLibraryConfig],
) -> Result<()> {
    println!("\n----------------------------------------");
    println!("📚 Seeding Libraries...");
    println!("----------------------------------------\n");

    let mut created = 0;
    let mut skipped = 0;

    for lib_cfg in libraries {
        // Check if library already exists by path
        let existing = LibraryRepository::get_by_path(db_conn, &lib_cfg.path).await?;

        if existing.is_some() {
            println!(
                "  ⏭  Library '{}' ({}) already exists, skipping.",
                lib_cfg.name, lib_cfg.path
            );
            skipped += 1;
            continue;
        }

        info!(
            "Creating library '{}' at '{}'...",
            lib_cfg.name, lib_cfg.path
        );

        let mut params = CreateLibraryParams::new(&lib_cfg.name, &lib_cfg.path);
        if let Some(strategy) = lib_cfg.series_strategy {
            params.series_strategy = strategy;
        }
        if let Some(ref config) = lib_cfg.series_config {
            params.series_config = Some(config.clone());
        }
        if let Some(strategy) = lib_cfg.book_strategy {
            params.book_strategy = strategy;
        }
        if let Some(ref config) = lib_cfg.book_config {
            params.book_config = Some(config.clone());
        }
        if let Some(strategy) = lib_cfg.number_strategy {
            params.number_strategy = strategy;
        }
        if let Some(ref config) = lib_cfg.number_config {
            params.number_config = Some(config.clone());
        }
        params.default_reading_direction = lib_cfg.default_reading_direction.clone();
        params.allowed_formats = lib_cfg
            .allowed_formats
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        params.excluded_patterns = lib_cfg.excluded_patterns.as_ref().map(|v| v.join("\n"));
        params.title_preprocessing_rules = lib_cfg
            .title_preprocessing_rules
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context(format!(
                "Failed to serialize title_preprocessing_rules for library '{}'",
                lib_cfg.name
            ))?;
        params.auto_match_conditions = lib_cfg
            .auto_match_conditions
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context(format!(
                "Failed to serialize auto_match_conditions for library '{}'",
                lib_cfg.name
            ))?;

        LibraryRepository::create_with_params(db_conn, params)
            .await
            .context(format!("Failed to create library '{}'", lib_cfg.name))?;

        println!(
            "  ✅ Library '{}' created at '{}'.",
            lib_cfg.name, lib_cfg.path
        );
        created += 1;
    }

    println!(
        "\nLibraries: {} created, {} skipped (already exist).",
        created, skipped
    );

    Ok(())
}

// =============================================================================
// Helpers
// =============================================================================

/// Generate a random password of specified length
fn generate_random_password(length: usize) -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    let mut rng = rand::rng();

    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
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
    permissions: &std::collections::HashSet<codex_api::permissions::Permission>,
) -> Result<(String, api_keys::Model)> {
    let mut rng = rand::rng();

    // Generate random components
    let prefix_random: String = (0..16)
        .map(|_| format!("{:x}", rng.random::<u8>() % 16))
        .collect();
    let suffix_random: String = (0..32)
        .map(|_| format!("{:x}", rng.random::<u8>() % 16))
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
        permissions.insert(codex_api::permissions::Permission::LibrariesRead);

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

    #[test]
    fn test_seed_config_parsing_full() {
        let yaml = r#"
users:
  admin:
    password: "admin123"
  maintainer:
    password: "maint123"
  reader:
    password: "read123"

plugins:
  - name: metadata-echo
    display_name: Echo
    description: Test echo plugin
    command: node
    args: ["/opt/codex/plugins/metadata-echo/dist/index.js"]
    permissions:
      - "metadata:write:*"
      - "metadata:read"
    scopes:
      - "series:detail"
      - "series:bulk"
    credential_delivery: env

  - name: metadata-mangabaka
    display_name: MangaBaka
    command: node
    args: ["/opt/codex/plugins/metadata-mangabaka/dist/index.js"]
    credential_delivery: init_message
    credentials:
      api_key: "test-key-123"

libraries:
  - name: Comics
    path: /libraries/comics
  - name: Manga
    path: /libraries/manga
    series_strategy: series_volume_chapter
    default_reading_direction: RIGHT_TO_LEFT
    excluded_patterns:
      - "*.txt"
      - "thumbs.db"
  - name: Books
    path: /libraries/books
    series_strategy: calibre
    book_strategy: metadata_first
    series_config:
      strip_id_suffix: true
      series_mode: from_metadata
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();

        // Users
        assert_eq!(config.users.len(), 3);
        assert_eq!(config.users["admin"].password, "admin123");
        assert_eq!(config.users["maintainer"].password, "maint123");
        assert_eq!(config.users["reader"].password, "read123");

        // Plugin 0: echo (explicit credential_delivery, no credentials)
        assert_eq!(config.plugins.len(), 2);
        assert_eq!(config.plugins[0].name, "metadata-echo");
        assert_eq!(config.plugins[0].display_name, "Echo");
        assert_eq!(
            config.plugins[0].description.as_deref(),
            Some("Test echo plugin")
        );
        assert_eq!(config.plugins[0].command, "node");
        assert_eq!(
            config.plugins[0].args,
            vec!["/opt/codex/plugins/metadata-echo/dist/index.js"]
        );
        assert_eq!(config.plugins[0].permissions.len(), 2);
        assert!(
            config.plugins[0]
                .permissions
                .contains(&PluginPermission::MetadataWriteAll)
        );
        assert!(
            config.plugins[0]
                .permissions
                .contains(&PluginPermission::MetadataRead)
        );
        assert_eq!(config.plugins[0].scopes.len(), 2);
        assert!(
            config.plugins[0]
                .scopes
                .contains(&PluginScope::SeriesDetail)
        );
        assert!(config.plugins[0].scopes.contains(&PluginScope::SeriesBulk));
        assert_eq!(config.plugins[0].plugin_type, "system");
        assert!(config.plugins[0].enabled);
        assert_eq!(config.plugins[0].credential_delivery, "env");
        assert!(config.plugins[0].credentials.is_none());

        // Plugin 1: mangabaka (with credentials)
        assert_eq!(config.plugins[1].name, "metadata-mangabaka");
        assert_eq!(config.plugins[1].credential_delivery, "init_message");
        let creds = config.plugins[1].credentials.as_ref().unwrap();
        assert_eq!(creds["api_key"], "test-key-123");

        // Library 0: Comics (defaults only)
        assert_eq!(config.libraries.len(), 3);
        assert_eq!(config.libraries[0].name, "Comics");
        assert_eq!(config.libraries[0].path, "/libraries/comics");
        assert!(config.libraries[0].series_strategy.is_none());
        assert!(config.libraries[0].series_config.is_none());
        assert!(config.libraries[0].default_reading_direction.is_none());
        assert!(config.libraries[0].excluded_patterns.is_none());

        // Library 1: Manga (with strategy overrides)
        assert_eq!(config.libraries[1].name, "Manga");
        assert_eq!(config.libraries[1].path, "/libraries/manga");
        assert_eq!(
            config.libraries[1].series_strategy,
            Some(SeriesStrategy::SeriesVolumeChapter)
        );
        assert_eq!(
            config.libraries[1].default_reading_direction.as_deref(),
            Some("RIGHT_TO_LEFT")
        );
        assert_eq!(
            config.libraries[1].excluded_patterns.as_deref(),
            Some(["*.txt", "thumbs.db"].map(String::from).as_slice())
        );

        // Library 2: Books (with calibre series_config)
        assert_eq!(config.libraries[2].name, "Books");
        assert_eq!(config.libraries[2].path, "/libraries/books");
        assert_eq!(
            config.libraries[2].series_strategy,
            Some(SeriesStrategy::Calibre)
        );
        assert_eq!(
            config.libraries[2].book_strategy,
            Some(BookStrategy::MetadataFirst)
        );
        let series_cfg = config.libraries[2].series_config.as_ref().unwrap();
        assert_eq!(series_cfg["strip_id_suffix"], true);
        assert_eq!(series_cfg["series_mode"], "from_metadata");
    }

    #[test]
    fn test_seed_library_preprocessing_and_conditions_parsing() {
        use codex_models::preprocessing::{ConditionMode, ConditionOperator};

        // Single-quoted YAML scalars keep regex backslashes literal — no
        // JSON-style double-escaping required.
        let yaml = r#"
libraries:
  - name: Comics
    path: /libraries/comics
    title_preprocessing_rules:
      - pattern: '\s*\(Digital\)$'
        replacement: ''
        description: Remove (Digital) suffix
        enabled: true
      - pattern: '_'
        replacement: ' '
    auto_match_conditions:
      mode: any
      rules:
        - field: external_ids.plugin:mangabaka
          operator: is_null
        - field: book_count
          operator: gte
          value: 1
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();
        let lib = &config.libraries[0];

        // Preprocessing rules deserialize as native structs.
        let rules = lib.title_preprocessing_rules.as_ref().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].pattern, r"\s*\(Digital\)$");
        assert_eq!(rules[0].replacement, "");
        assert_eq!(
            rules[0].description.as_deref(),
            Some("Remove (Digital) suffix")
        );
        assert!(rules[0].enabled);
        // `enabled` defaults to true when omitted.
        assert!(rules[1].enabled);

        // Auto-match conditions deserialize as native structs.
        let conditions = lib.auto_match_conditions.as_ref().unwrap();
        assert_eq!(conditions.mode, ConditionMode::Any);
        assert_eq!(conditions.rules.len(), 2);
        assert_eq!(conditions.rules[0].operator, ConditionOperator::IsNull);
        assert_eq!(conditions.rules[1].operator, ConditionOperator::Gte);

        // The seed loop serializes these to the JSON strings the libraries table
        // stores; the regex round-trips through JSON unchanged.
        let rules_json = serde_json::to_string(rules).unwrap();
        assert!(rules_json.contains(r"\\s*\\(Digital\\)$"));
        let parsed =
            codex_models::preprocessing::parse_preprocessing_rules(Some(&rules_json)).unwrap();
        assert_eq!(parsed, *rules);
    }

    #[test]
    fn test_seed_config_parsing_empty() {
        let yaml = "{}";
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(config.users.is_empty());
        assert!(config.plugins.is_empty());
        assert!(config.libraries.is_empty());
    }

    #[test]
    fn test_seed_config_parsing_partial_users_only() {
        let yaml = r#"
users:
  admin:
    password: "test"
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.users.len(), 1);
        assert_eq!(config.users["admin"].password, "test");
        assert!(config.plugins.is_empty());
        assert!(config.libraries.is_empty());
    }

    #[test]
    fn test_seed_config_parsing_partial_plugins_only() {
        let yaml = r#"
plugins:
  - name: my-plugin
    display_name: My Plugin
    command: node
    args: ["/path/to/plugin.js"]
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(config.users.is_empty());
        assert_eq!(config.plugins.len(), 1);
        assert_eq!(config.plugins[0].name, "my-plugin");
        // Defaults
        assert_eq!(config.plugins[0].plugin_type, "system");
        assert!(config.plugins[0].enabled);
        assert!(config.plugins[0].permissions.is_empty());
        assert!(config.plugins[0].scopes.is_empty());
        assert!(config.plugins[0].env.is_empty());
        assert_eq!(config.plugins[0].credential_delivery, "init_message");
        assert!(config.plugins[0].credentials.is_none());
        assert!(config.libraries.is_empty());
    }

    #[test]
    fn test_seed_config_from_file_not_found() {
        let result = SeedConfig::from_file("/nonexistent/path.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_seed_plugin_libraries_parsing() {
        let yaml = r#"
plugins:
  - name: anilist-manga
    display_name: AniList Manga
    command: node
    libraries: [Manga, Comics]
  - name: anilist-all
    display_name: AniList All
    command: node
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.plugins[0].libraries, vec!["Manga", "Comics"]);
        // Absent `libraries` defaults to empty (all libraries).
        assert!(config.plugins[1].libraries.is_empty());
    }

    #[tokio::test]
    async fn test_seed_plugins_resolves_library_scope_by_name() {
        use codex_db::test_helpers::create_test_db;

        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let yaml = r#"
libraries:
  - name: Manga
    path: /libraries/manga
  - name: Comics
    path: /libraries/comics
plugins:
  - name: anilist-manga
    display_name: AniList Manga
    command: node
    libraries: [Manga]
  - name: anilist-all
    display_name: AniList All
    command: node
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();

        // Seed libraries first, then plugins (the order seed_command uses).
        seed_libraries(conn, &config.libraries).await.unwrap();
        seed_plugins(conn, &config.plugins).await.unwrap();

        let manga = LibraryRepository::get_by_path(conn, "/libraries/manga")
            .await
            .unwrap()
            .unwrap();

        // Scoped plugin resolved the name to the library's UUID.
        let scoped = PluginsRepository::get_by_name(conn, "anilist-manga")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scoped.library_ids_vec(), vec![manga.id]);

        // Unscoped plugin applies to all libraries (empty).
        let all = PluginsRepository::get_by_name(conn, "anilist-all")
            .await
            .unwrap()
            .unwrap();
        assert!(all.library_ids_vec().is_empty());
    }

    #[tokio::test]
    async fn test_seed_plugins_errors_on_unknown_library() {
        use codex_db::test_helpers::create_test_db;

        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let yaml = r#"
plugins:
  - name: broken
    display_name: Broken
    command: node
    libraries: [DoesNotExist]
"#;
        let config: SeedConfig = serde_yaml::from_str(yaml).unwrap();

        let err = seed_plugins(conn, &config.plugins).await.unwrap_err();
        assert!(
            err.to_string().contains("unknown library"),
            "expected unknown-library error, got: {err}"
        );
    }
}
