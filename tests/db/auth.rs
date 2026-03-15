#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::api::permissions::{
    ADMIN_PERMISSIONS, Permission, READONLY_PERMISSIONS, parse_permissions, serialize_permissions,
};
use codex::db::repositories::{ApiKeyRepository, UserRepository};
use codex::utils::{jwt::JwtService, password};
use common::*;
use std::collections::HashSet;

/// Test the complete user authentication flow
#[tokio::test]
async fn test_user_authentication_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // 1. Create a user with hashed password
    let plain_password = "secure_password_123";
    let password_hash = password::hash_password(plain_password).unwrap();

    let user = create_test_user("authtest", "authtest@example.com", &password_hash, false);

    let created_user = UserRepository::create(&db, &user).await.unwrap();
    assert_eq!(created_user.username, "authtest");

    // 2. Verify password (simulating login)
    let is_valid = password::verify_password(plain_password, &created_user.password_hash).unwrap();
    assert!(is_valid, "Password should verify correctly");

    let wrong_password =
        password::verify_password("wrong_password", &created_user.password_hash).unwrap();
    assert!(!wrong_password, "Wrong password should not verify");

    // 3. Update last login timestamp
    UserRepository::update_last_login(&db, created_user.id)
        .await
        .unwrap();

    let updated_user = UserRepository::get_by_id(&db, created_user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        updated_user.last_login_at.is_some(),
        "Last login should be set"
    );
}

/// Test the complete JWT token flow
#[tokio::test]
async fn test_jwt_token_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a user
    let user = create_test_user("jwttest", "jwt@example.com", "hash123", true);

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Generate JWT token
    let jwt_service = JwtService::new("test_secret_key".to_string(), 24);
    let token = jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    assert!(!token.is_empty());

    // Verify token
    let claims = jwt_service.verify_token(&token).unwrap();
    assert_eq!(claims.sub, created_user.id.to_string());
    assert_eq!(claims.username, "jwttest");
    assert_eq!(claims.role, "admin");

    // Token from different service should fail
    let different_service = JwtService::new("different_secret".to_string(), 24);
    let verify_result = different_service.verify_token(&token);
    assert!(
        verify_result.is_err(),
        "Token with wrong secret should fail"
    );
}

/// Test the complete API key flow
#[tokio::test]
async fn test_api_key_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // 1. Create a user
    let user = create_test_user("apikeytest", "apikey@example.com", "hash123", false);

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // 2. Generate an API key
    let plain_key = "codex_abc123def456_xyz789uvw012";
    let key_hash = password::hash_password(plain_key).unwrap();

    let mut permissions = HashSet::new();
    permissions.insert(Permission::LibrariesRead);
    permissions.insert(Permission::BooksRead);
    permissions.insert(Permission::PagesRead);

    let permissions_json = serialize_permissions(&permissions);
    let api_key = create_test_api_key(
        created_user.id,
        "Test API Key",
        &key_hash,
        "codex_abc",
        serde_json::from_str(&permissions_json).unwrap(),
    );

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
    let parsed_perms: HashSet<Permission> = serde_json::from_value(found_key.permissions).unwrap();
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
    assert_eq!(READONLY_PERMISSIONS.len(), 7);

    // Test ADMIN permissions
    assert!(ADMIN_PERMISSIONS.contains(&Permission::SystemAdmin));
    assert!(ADMIN_PERMISSIONS.contains(&Permission::UsersWrite));
    assert!(ADMIN_PERMISSIONS.contains(&Permission::LibrariesDelete));
    assert_eq!(ADMIN_PERMISSIONS.len(), 23);

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
    let user = create_test_user("multikey", "multikey@example.com", "hash123", false);

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
        let permissions_json = serialize_permissions(&permissions);
        let api_key = create_test_api_key(
            created_user.id,
            name,
            &format!("hash_{}", name),
            &format!("codex_{}", name),
            serde_json::from_str(&permissions_json).unwrap(),
        );

        ApiKeyRepository::create(&db, &api_key).await.unwrap();
    }

    // List all keys for user
    let user_keys = ApiKeyRepository::list_by_user(&db, created_user.id)
        .await
        .unwrap();
    assert_eq!(user_keys.len(), 3);

    // Verify each key has correct permissions
    for key in &user_keys {
        let perms: HashSet<Permission> = serde_json::from_value(key.permissions.clone()).unwrap();
        if key.name == "Mobile App" {
            assert_eq!(perms.len(), 7); // READONLY
        } else if key.name == "Admin Tool" {
            assert_eq!(perms.len(), 23); // ADMIN
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

    let user = create_test_user("expiretest", "expire@example.com", "hash123", false);

    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Create API key with expiration in the past
    let permissions_json = serialize_permissions(&READONLY_PERMISSIONS);
    let mut expired_key = create_test_api_key(
        created_user.id,
        "Expired Key",
        "expired_hash",
        "codex_exp",
        serde_json::from_str(&permissions_json).unwrap(),
    );
    expired_key.expires_at = Some(Utc::now() - chrono::Duration::days(1));

    let created = ApiKeyRepository::create(&db, &expired_key).await.unwrap();

    // Key exists but is expired (application logic would check expires_at)
    let found = ApiKeyRepository::get_by_id(&db, created.id)
        .await
        .unwrap()
        .unwrap();

    assert!(found.expires_at.is_some());
    assert!(found.expires_at.unwrap() < Utc::now());
}
