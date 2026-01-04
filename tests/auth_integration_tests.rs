use codex::api::permissions::{parse_permissions, serialize_permissions, Permission, ADMIN_PERMISSIONS, READONLY_PERMISSIONS};
use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::entities::{api_keys, users};
use codex::db::repositories::{ApiKeyRepository, UserRepository};
use codex::db::Database;
use codex::utils::{jwt::JwtService, password};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use tempfile::TempDir;
use uuid::Uuid;

/// Helper to create a test SQLite database with migrations applied
async fn setup_test_db() -> (sea_orm::DatabaseConnection, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let mut pragmas = HashMap::new();
    pragmas.insert("foreign_keys".to_string(), "ON".to_string());

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: Some(pragmas),
        }),
    };

    let db = Database::new(&config).await.unwrap();
    db.run_migrations().await.unwrap();
    let conn = db.sea_orm_connection().clone();
    (conn, temp_dir)
}

/// Test the complete user authentication flow
#[tokio::test]
async fn test_user_authentication_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // 1. Create a user with hashed password
    let plain_password = "secure_password_123";
    let password_hash = password::hash_password(plain_password).unwrap();

    let user = users::Model {
        id: Uuid::new_v4(),
        username: "authtest".to_string(),
        email: "authtest@example.com".to_string(),
        password_hash: password_hash.clone(),
        is_admin: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };

    let created_user = UserRepository::create(&db, &user).await.unwrap();
    assert_eq!(created_user.username, "authtest");

    // 2. Verify password (simulating login)
    let is_valid = password::verify_password(plain_password, &created_user.password_hash).unwrap();
    assert!(is_valid, "Password should verify correctly");

    let wrong_password = password::verify_password("wrong_password", &created_user.password_hash).unwrap();
    assert!(!wrong_password, "Wrong password should not verify");

    // 3. Update last login timestamp
    UserRepository::update_last_login(&db, created_user.id)
        .await
        .unwrap();

    let updated_user = UserRepository::get_by_id(&db, created_user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated_user.last_login_at.is_some(), "Last login should be set");
}

/// Test the complete JWT token flow
#[tokio::test]
async fn test_jwt_token_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a user
    let user = users::Model {
        id: Uuid::new_v4(),
        username: "jwttest".to_string(),
        email: "jwt@example.com".to_string(),
        password_hash: "hash123".to_string(),
        is_admin: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Generate JWT token
    let jwt_service = JwtService::new("test_secret_key".to_string(), 24);
    let token = jwt_service
        .generate_token(created_user.id, created_user.username.clone(), created_user.is_admin)
        .unwrap();

    assert!(!token.is_empty());

    // Verify token
    let claims = jwt_service.verify_token(&token).unwrap();
    assert_eq!(claims.sub, created_user.id.to_string());
    assert_eq!(claims.username, "jwttest");
    assert!(claims.is_admin);

    // Token from different service should fail
    let different_service = JwtService::new("different_secret".to_string(), 24);
    let verify_result = different_service.verify_token(&token);
    assert!(verify_result.is_err(), "Token with wrong secret should fail");
}

/// Test the complete API key flow
#[tokio::test]
async fn test_api_key_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // 1. Create a user
    let user = users::Model {
        id: Uuid::new_v4(),
        username: "apikeytest".to_string(),
        email: "apikey@example.com".to_string(),
        password_hash: "hash123".to_string(),
        is_admin: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // 2. Generate an API key
    let plain_key = "codex_abc123def456_xyz789uvw012";
    let key_hash = password::hash_password(plain_key).unwrap();

    let mut permissions = HashSet::new();
    permissions.insert(Permission::LibrariesRead);
    permissions.insert(Permission::BooksRead);
    permissions.insert(Permission::PagesRead);

    let api_key = api_keys::Model {
        id: Uuid::new_v4(),
        user_id: created_user.id,
        name: "Test API Key".to_string(),
        key_hash: key_hash.clone(),
        key_prefix: "codex_abc".to_string(),
        permissions: serialize_permissions(&permissions),
        is_active: true,
        expires_at: None,
        last_used_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created_key = ApiKeyRepository::create(&db, &api_key).await.unwrap();
    assert_eq!(created_key.name, "Test API Key");

    // 3. Verify API key (simulating authentication)
    let found_key = ApiKeyRepository::get_by_hash(&db, &key_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(found_key.name, "Test API Key");
    assert!(found_key.is_active);

    // 4. Parse permissions
    let parsed_perms = parse_permissions(&found_key.permissions).unwrap();
    assert_eq!(parsed_perms.len(), 3);
    assert!(parsed_perms.contains(&Permission::LibrariesRead));
    assert!(parsed_perms.contains(&Permission::BooksRead));
    assert!(parsed_perms.contains(&Permission::PagesRead));
    assert!(!parsed_perms.contains(&Permission::LibrariesWrite));

    // 5. Update last used timestamp
    ApiKeyRepository::update_last_used(&db, created_key.id)
        .await
        .unwrap();

    let updated_key = ApiKeyRepository::get_by_id(&db, created_key.id)
        .await
        .unwrap()
        .unwrap();
    assert!(updated_key.last_used_at.is_some());

    // 6. Revoke API key
    ApiKeyRepository::revoke(&db, created_key.id).await.unwrap();

    // Should not be findable by hash when revoked (get_by_hash filters by is_active)
    let revoked_result = ApiKeyRepository::get_by_hash(&db, &key_hash).await.unwrap();
    assert!(revoked_result.is_none(), "Revoked key should not be found");
}

/// Test permission sets
#[tokio::test]
async fn test_permission_sets() {
    // Test READONLY permissions
    assert!(READONLY_PERMISSIONS.contains(&Permission::LibrariesRead));
    assert!(READONLY_PERMISSIONS.contains(&Permission::BooksRead));
    assert!(!READONLY_PERMISSIONS.contains(&Permission::LibrariesWrite));
    assert_eq!(READONLY_PERMISSIONS.len(), 5);

    // Test ADMIN permissions
    assert!(ADMIN_PERMISSIONS.contains(&Permission::SystemAdmin));
    assert!(ADMIN_PERMISSIONS.contains(&Permission::UsersWrite));
    assert!(ADMIN_PERMISSIONS.contains(&Permission::LibrariesDelete));
    assert_eq!(ADMIN_PERMISSIONS.len(), 18);

    // Test permission serialization roundtrip
    let perms = READONLY_PERMISSIONS.clone();
    let serialized = serialize_permissions(&perms);
    let deserialized = parse_permissions(&serialized).unwrap();
    assert_eq!(perms, deserialized);
}

/// Test user with multiple API keys
#[tokio::test]
async fn test_user_with_multiple_api_keys() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a user
    let user = users::Model {
        id: Uuid::new_v4(),
        username: "multikey".to_string(),
        email: "multikey@example.com".to_string(),
        password_hash: "hash123".to_string(),
        is_admin: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Create multiple API keys with different permissions
    let keys = vec![
        ("Mobile App", READONLY_PERMISSIONS.clone()),
        ("Admin Tool", ADMIN_PERMISSIONS.clone()),
        ("CI/CD", {
            let mut perms = HashSet::new();
            perms.insert(Permission::BooksRead);
            perms
        }),
    ];

    for (name, permissions) in keys {
        let api_key = api_keys::Model {
            id: Uuid::new_v4(),
            user_id: created_user.id,
            name: name.to_string(),
            key_hash: format!("hash_{}", name),
            key_prefix: format!("codex_{}", name),
            permissions: serialize_permissions(&permissions),
            is_active: true,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        ApiKeyRepository::create(&db, &api_key).await.unwrap();
    }

    // List all keys for user
    let user_keys = ApiKeyRepository::list_by_user(&db, created_user.id)
        .await
        .unwrap();
    assert_eq!(user_keys.len(), 3);

    // Verify each key has correct permissions
    for key in &user_keys {
        let perms = parse_permissions(&key.permissions).unwrap();
        if key.name == "Mobile App" {
            assert_eq!(perms.len(), 5); // READONLY
        } else if key.name == "Admin Tool" {
            assert_eq!(perms.len(), 18); // ADMIN
        } else if key.name == "CI/CD" {
            assert_eq!(perms.len(), 1);
            assert!(perms.contains(&Permission::BooksRead));
        }
    }
}

/// Test password hashing and verification with edge cases
#[tokio::test]
async fn test_password_edge_cases() {
    // Empty password (should work but not recommended)
    let empty_hash = password::hash_password("").unwrap();
    assert!(password::verify_password("", &empty_hash).unwrap());

    // Very long password
    let long_password = "a".repeat(1000);
    let long_hash = password::hash_password(&long_password).unwrap();
    assert!(password::verify_password(&long_password, &long_hash).unwrap());

    // Special characters
    let special_password = "p@ssw0rd!#$%^&*()";
    let special_hash = password::hash_password(special_password).unwrap();
    assert!(password::verify_password(special_password, &special_hash).unwrap());

    // Unicode characters
    let unicode_password = "пароль密码🔒";
    let unicode_hash = password::hash_password(unicode_password).unwrap();
    assert!(password::verify_password(unicode_password, &unicode_hash).unwrap());
}

/// Test API key expiration logic
#[tokio::test]
async fn test_api_key_expiration() {
    let (db, _temp_dir) = setup_test_db().await;

    let user = users::Model {
        id: Uuid::new_v4(),
        username: "expiretest".to_string(),
        email: "expire@example.com".to_string(),
        password_hash: "hash123".to_string(),
        is_admin: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Create API key with expiration in the past
    let expired_key = api_keys::Model {
        id: Uuid::new_v4(),
        user_id: created_user.id,
        name: "Expired Key".to_string(),
        key_hash: "expired_hash".to_string(),
        key_prefix: "codex_exp".to_string(),
        permissions: serialize_permissions(&READONLY_PERMISSIONS),
        is_active: true,
        expires_at: Some(Utc::now() - chrono::Duration::days(1)),
        last_used_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created = ApiKeyRepository::create(&db, &expired_key).await.unwrap();

    // Key exists but is expired (application logic would check expires_at)
    let found = ApiKeyRepository::get_by_id(&db, created.id)
        .await
        .unwrap()
        .unwrap();

    assert!(found.expires_at.is_some());
    assert!(found.expires_at.unwrap() < Utc::now());
}
