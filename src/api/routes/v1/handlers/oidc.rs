//! OIDC Authentication Handlers
//!
//! Handlers for OpenID Connect (OIDC) authentication endpoints.
//! These endpoints enable authentication via external identity providers.

use super::super::dto::{
    OidcCallbackQuery, OidcCallbackResponse, OidcLoginResponse, OidcProviderInfo,
    OidcProvidersResponse, UserInfo,
};
use super::auth::build_auth_cookie;
use crate::api::{error::ApiError, extractors::AppState, permissions::UserRole};
use crate::db::{
    entities::users,
    repositories::{OidcConnectionRepository, UserRepository},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Parse permissions from JSON value (stored as array of strings in database)
fn parse_permissions_json(json: &serde_json::Value) -> Vec<String> {
    json.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// List available OIDC providers
///
/// Returns the list of configured OIDC providers that users can authenticate with.
/// This endpoint is public and does not require authentication.
#[utoipa::path(
    get,
    path = "/api/v1/auth/oidc/providers",
    responses(
        (status = 200, description = "List of available OIDC providers", body = OidcProvidersResponse),
    ),
    tag = "Auth"
)]
pub async fn list_providers(State(state): State<Arc<AppState>>) -> Json<OidcProvidersResponse> {
    let (enabled, providers) = match &state.oidc_service {
        Some(service) if service.is_enabled() => {
            let providers = service
                .get_providers()
                .into_iter()
                .map(|p| OidcProviderInfo {
                    name: p.name.clone(),
                    display_name: p.display_name.clone(),
                    login_url: format!("/api/v1/auth/oidc/{}/login", p.name),
                })
                .collect();
            (true, providers)
        }
        _ => (false, vec![]),
    };

    Json(OidcProvidersResponse { enabled, providers })
}

/// Initiate OIDC login flow
///
/// Generates an authorization URL and returns it to the client.
/// The client should redirect the user to this URL to authenticate.
#[utoipa::path(
    post,
    path = "/api/v1/auth/oidc/{provider}/login",
    operation_id = "oidc_login",
    params(
        ("provider" = String, Path, description = "OIDC provider name (e.g., 'authentik', 'keycloak')")
    ),
    responses(
        (status = 200, description = "Authorization URL generated", body = OidcLoginResponse),
        (status = 400, description = "OIDC not enabled or unknown provider"),
        (status = 500, description = "Failed to generate authorization URL"),
    ),
    tag = "Auth"
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> Result<Json<OidcLoginResponse>, ApiError> {
    // Check if OIDC is enabled
    let oidc_service = state
        .oidc_service
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("OIDC authentication is not enabled".to_string()))?;

    if !oidc_service.is_enabled() {
        return Err(ApiError::BadRequest(
            "OIDC authentication is not enabled".to_string(),
        ));
    }

    // Check if provider exists
    if oidc_service.get_provider_config(&provider).is_none() {
        return Err(ApiError::BadRequest(format!(
            "Unknown OIDC provider: {}",
            provider
        )));
    }

    // Generate authorization URL
    let (redirect_url, state_token) =
        oidc_service
            .generate_auth_url(&provider)
            .await
            .map_err(|e| {
                warn!(error = %e, provider = %provider, "Failed to generate OIDC auth URL");
                ApiError::Internal(format!("Failed to initiate OIDC login: {}", e))
            })?;

    debug!(
        provider = %provider,
        state = %state_token,
        "Generated OIDC authorization URL"
    );

    Ok(Json(OidcLoginResponse { redirect_url }))
}

/// Handle OIDC callback from identity provider
///
/// This endpoint receives the callback from the identity provider after
/// the user has authenticated. It exchanges the authorization code for tokens,
/// validates the response, and either creates a new user or links to an existing one.
#[utoipa::path(
    get,
    path = "/api/v1/auth/oidc/{provider}/callback",
    params(
        ("provider" = String, Path, description = "OIDC provider name"),
        ("code" = String, Query, description = "Authorization code from IdP"),
        ("state" = String, Query, description = "State parameter for CSRF protection"),
    ),
    responses(
        (status = 302, description = "Redirect to frontend with auth cookie set"),
        (status = 400, description = "Invalid callback parameters or OIDC error"),
        (status = 500, description = "Internal server error during authentication"),
    ),
    tag = "Auth"
)]
pub async fn callback(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(query): Query<OidcCallbackQuery>,
) -> Result<Response, ApiError> {
    // Check for IdP-reported errors first
    if let Some(error) = query.error {
        let description = query
            .error_description
            .unwrap_or_else(|| "Authentication was denied".to_string());
        warn!(
            provider = %provider,
            error = %error,
            description = %description,
            "OIDC authentication failed at IdP"
        );
        // Redirect to login page with error
        return Ok(Redirect::to(&format!(
            "/login?error={}&error_description={}",
            urlencoding::encode(&error),
            urlencoding::encode(&description)
        ))
        .into_response());
    }

    // Check if OIDC is enabled
    let oidc_service = state
        .oidc_service
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("OIDC authentication is not enabled".to_string()))?;

    if !oidc_service.is_enabled() {
        return Err(ApiError::BadRequest(
            "OIDC authentication is not enabled".to_string(),
        ));
    }

    // Exchange code for tokens and validate
    let auth_result = oidc_service
        .exchange_code(&provider, &query.code, &query.state)
        .await
        .map_err(|e| {
            warn!(error = %e, provider = %provider, "OIDC code exchange failed");
            ApiError::Unauthorized(format!("Authentication failed: {}", e))
        })?;

    info!(
        provider = %provider,
        subject = %auth_result.subject,
        email = ?auth_result.email,
        role = %auth_result.mapped_role,
        "OIDC authentication successful"
    );

    // Try to find existing OIDC connection
    let existing_connection = OidcConnectionRepository::find_by_provider_subject(
        &state.db,
        &provider,
        &auth_result.subject,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let (user, is_new_account) = if let Some(connection) = existing_connection {
        // Existing connection found - get the user
        let user = UserRepository::get_by_id(&state.db, connection.user_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| {
                warn!(
                    user_id = %connection.user_id,
                    "OIDC connection exists but user not found"
                );
                ApiError::Internal("User account not found".to_string())
            })?;

        // Update last used and groups
        OidcConnectionRepository::update_groups_and_last_used(
            &state.db,
            connection.id,
            Some(serde_json::to_value(&auth_result.groups).unwrap_or_default()),
            auth_result.email.clone(),
            auth_result.display_name.clone(),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update OIDC connection: {}", e)))?;

        // Sync role from IdP groups on every login
        let user = sync_role_from_groups(&state.db, user, &auth_result.mapped_role).await?;

        (user, false)
    } else {
        // No existing connection - try to find or create user
        let email = auth_result.email.as_ref().ok_or_else(|| {
            ApiError::BadRequest(
                "Email is required for OIDC authentication. Please ensure your IdP is configured to include email in claims.".to_string()
            )
        })?;

        // Try to find user by email
        let existing_user = UserRepository::get_by_email(&state.db, email)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        let (user, is_new) = if let Some(user) = existing_user {
            // Existing user - link the OIDC connection
            info!(
                user_id = %user.id,
                email = %email,
                provider = %provider,
                "Linking OIDC connection to existing user"
            );
            // Sync role from IdP groups
            let user = sync_role_from_groups(&state.db, user, &auth_result.mapped_role).await?;
            (user, false)
        } else {
            // No existing user - check if auto-create is enabled
            if !oidc_service.auto_create_users() {
                return Err(ApiError::Forbidden(
                    "Account creation via OIDC is disabled. Please contact an administrator."
                        .to_string(),
                ));
            }

            // Create new user
            let username = generate_unique_username(
                &state.db,
                auth_result.username.as_deref(),
                auth_result.display_name.as_deref(),
                email,
            )
            .await?;

            // Map role string to UserRole
            let role = match auth_result.mapped_role.as_str() {
                "admin" => UserRole::Admin,
                "maintainer" => UserRole::Maintainer,
                _ => UserRole::Reader,
            };

            let new_user = users::Model {
                id: Uuid::new_v4(),
                username: username.clone(),
                email: email.clone(),
                // OIDC users don't have a local password - set a random hash
                // They must authenticate via OIDC
                password_hash: format!("oidc:{}", Uuid::new_v4()),
                role: role.to_string(),
                is_active: true,
                email_verified: true, // Trust IdP's email verification
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: Some(Utc::now()),
            };

            let created = UserRepository::create(&state.db, &new_user)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to create user: {}", e)))?;

            info!(
                user_id = %created.id,
                username = %username,
                email = %email,
                role = %role,
                provider = %provider,
                "Created new user via OIDC"
            );

            (created, true)
        };

        // Create OIDC connection
        let connection = crate::db::entities::oidc_connections::Model {
            id: Uuid::new_v4(),
            user_id: user.id,
            provider_name: provider.clone(),
            subject: auth_result.subject.clone(),
            email: auth_result.email.clone(),
            display_name: auth_result.display_name.clone(),
            groups: Some(serde_json::to_value(&auth_result.groups).unwrap_or_default()),
            access_token_hash: None, // Not storing tokens for now
            refresh_token_encrypted: None,
            token_expires_at: auth_result.token_expires_at,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: Some(Utc::now()),
        };

        OidcConnectionRepository::create(&state.db, &connection)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create OIDC connection: {}", e)))?;

        (user, is_new)
    };

    // Check if user is active
    if !user.is_active {
        return Err(ApiError::Unauthorized(
            "Your account has been deactivated. Please contact an administrator.".to_string(),
        ));
    }

    // Update last login
    UserRepository::update_last_login(&state.db, user.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update last login: {}", e)))?;

    // Generate JWT token
    let access_token = state
        .jwt_service
        .generate_token(user.id, user.username.clone(), user.get_role())
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {}", e)))?;

    // Build response data for frontend
    let role = user.get_role().to_string();
    let permissions = parse_permissions_json(&user.permissions);
    let response = OidcCallbackResponse {
        access_token: access_token.clone(),
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.jwt_expiry_hours as u64 * 3600,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            role,
            email_verified: user.email_verified,
            permissions,
        },
        new_account: is_new_account,
        provider: provider.clone(),
    };

    // Create HTTP-only auth cookie (for image/resource requests)
    let cookie = build_auth_cookie(
        &access_token,
        state.auth_config.jwt_expiry_hours as u64 * 3600,
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to create cookie header".to_string()))?,
    );

    // Encode auth data as URL-safe base64 in a URL fragment.
    // Fragments are never sent to the server, preventing token leakage via referrer headers.
    let response_json = serde_json::to_string(&response)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize response: {}", e)))?;
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(response_json.as_bytes());

    // Redirect to frontend callback page with auth data in URL fragment
    let redirect_url = format!("/login/oidc/complete#{}", encoded);
    Ok((headers, Redirect::to(&redirect_url)).into_response())
}

/// Sync user role from IdP group mapping on every OIDC login.
///
/// If the mapped role differs from the current role, updates the user in the database.
async fn sync_role_from_groups(
    db: &sea_orm::DatabaseConnection,
    mut user: users::Model,
    mapped_role: &str,
) -> Result<users::Model, ApiError> {
    if user.role != mapped_role {
        info!(
            user_id = %user.id,
            username = %user.username,
            old_role = %user.role,
            new_role = %mapped_role,
            "Updating user role from OIDC group mapping"
        );
        user.role = mapped_role.to_string();
        user.updated_at = Utc::now();
        let updated = UserRepository::update(db, &user)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update user role: {}", e)))?;
        Ok(updated)
    } else {
        Ok(user)
    }
}

/// Generate a unique username for a new OIDC user
///
/// Tries the preferred username first, then falls back to variations.
async fn generate_unique_username(
    db: &sea_orm::DatabaseConnection,
    preferred_username: Option<&str>,
    display_name: Option<&str>,
    email: &str,
) -> Result<String, ApiError> {
    // Build candidate usernames in order of preference
    let mut candidates = Vec::new();

    // First choice: preferred_username from IdP
    if let Some(username) = preferred_username {
        let sanitized = sanitize_username(username);
        if !sanitized.is_empty() {
            candidates.push(sanitized);
        }
    }

    // Second choice: display name
    if let Some(name) = display_name {
        let sanitized = sanitize_username(name);
        if !sanitized.is_empty() && !candidates.contains(&sanitized) {
            candidates.push(sanitized);
        }
    }

    // Third choice: email prefix
    if let Some(prefix) = email.split('@').next() {
        let sanitized = sanitize_username(prefix);
        if !sanitized.is_empty() && !candidates.contains(&sanitized) {
            candidates.push(sanitized);
        }
    }

    // Fallback: random
    if candidates.is_empty() {
        candidates.push(format!("user_{}", &Uuid::new_v4().to_string()[..8]));
    }

    // Try each candidate, adding a number suffix if needed
    for candidate in candidates {
        // Check if base username is available
        if UserRepository::get_by_username(db, &candidate)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .is_none()
        {
            return Ok(candidate);
        }

        // Try with numeric suffixes
        for i in 1..=99 {
            let with_suffix = format!("{}_{}", candidate, i);
            if UserRepository::get_by_username(db, &with_suffix)
                .await
                .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
                .is_none()
            {
                return Ok(with_suffix);
            }
        }
    }

    // Last resort: fully random
    Ok(format!("user_{}", Uuid::new_v4()))
}

/// Sanitize a string to be used as a username
///
/// Removes or replaces invalid characters and ensures minimum length.
fn sanitize_username(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .take(50) // Max length
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_username() {
        assert_eq!(sanitize_username("JohnDoe"), "johndoe");
        assert_eq!(sanitize_username("john.doe"), "johndoe");
        assert_eq!(sanitize_username("john_doe-123"), "john_doe-123");
        assert_eq!(sanitize_username("john@example.com"), "johnexamplecom");
        assert_eq!(sanitize_username(""), "");
        assert_eq!(sanitize_username("a".repeat(100).as_str()).len(), 50);
    }

    #[test]
    fn test_sanitize_username_unicode() {
        // is_alphanumeric() includes Unicode letters/digits, so they are preserved
        assert_eq!(sanitize_username("jöhn_dœ"), "jöhn_dœ");
        assert_eq!(sanitize_username("用户123"), "用户123");
        // Emoji are not alphanumeric and get filtered
        assert_eq!(sanitize_username("🎉party"), "party");
    }

    #[test]
    fn test_sanitize_username_special_chars() {
        assert_eq!(sanitize_username("john doe"), "johndoe"); // spaces removed
        assert_eq!(sanitize_username("john\tdoe"), "johndoe"); // tabs removed
        assert_eq!(sanitize_username("john/doe"), "johndoe"); // slashes removed
        assert_eq!(sanitize_username("john\\doe"), "johndoe"); // backslashes removed
        assert_eq!(sanitize_username("<script>"), "script"); // HTML stripped
    }

    #[test]
    fn test_sanitize_username_preserves_hyphens_and_underscores() {
        assert_eq!(sanitize_username("my-user_name"), "my-user_name");
        assert_eq!(sanitize_username("___---"), "___---");
        assert_eq!(sanitize_username("a-b_c-d"), "a-b_c-d");
    }

    #[test]
    fn test_parse_permissions_json() {
        let json = serde_json::json!(["read", "write"]);
        assert_eq!(parse_permissions_json(&json), vec!["read", "write"]);

        let empty = serde_json::json!([]);
        assert!(parse_permissions_json(&empty).is_empty());

        let invalid = serde_json::json!("not an array");
        assert!(parse_permissions_json(&invalid).is_empty());
    }

    #[test]
    fn test_parse_permissions_json_with_mixed_types() {
        // Array with non-string values should filter them out
        let json = serde_json::json!(["read", 42, "write", null, true]);
        let result = parse_permissions_json(&json);
        assert_eq!(result, vec!["read", "write"]);
    }

    #[test]
    fn test_parse_permissions_json_null() {
        let json = serde_json::json!(null);
        assert!(parse_permissions_json(&json).is_empty());
    }

    #[test]
    fn test_parse_permissions_json_object() {
        let json = serde_json::json!({"read": true});
        assert!(parse_permissions_json(&json).is_empty());
    }

    // Integration tests for async functions that need a database
    mod db_tests {
        use super::*;
        use crate::db::repositories::UserRepository;
        use sea_orm::Database;

        async fn setup_test_db() -> sea_orm::DatabaseConnection {
            let db = Database::connect("sqlite::memory:")
                .await
                .expect("Failed to connect to in-memory database");

            // Run migrations
            use migration::{Migrator, MigratorTrait};
            Migrator::up(&db, None)
                .await
                .expect("Failed to run migrations");

            db
        }

        #[tokio::test]
        async fn test_generate_unique_username_preferred() {
            let db = setup_test_db().await;

            let result = generate_unique_username(
                &db,
                Some("johndoe"),
                Some("John Doe"),
                "john@example.com",
            )
            .await
            .unwrap();
            assert_eq!(result, "johndoe");
        }

        #[tokio::test]
        async fn test_generate_unique_username_with_suffix_when_taken() {
            let db = setup_test_db().await;

            // Create user with "johndoe" username to force numeric suffix
            let user = users::Model {
                id: Uuid::new_v4(),
                username: "johndoe".to_string(),
                email: "existing@example.com".to_string(),
                password_hash: "hash".to_string(),
                role: "reader".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            };
            UserRepository::create(&db, &user).await.unwrap();

            let result =
                generate_unique_username(&db, Some("johndoe"), Some("John D"), "john@example.com")
                    .await
                    .unwrap();
            // Tries "johndoe" first (taken), then "johndoe_1" (available)
            assert_eq!(result, "johndoe_1");
        }

        #[tokio::test]
        async fn test_generate_unique_username_tries_all_candidates() {
            let db = setup_test_db().await;

            // Fill up "johndoe" and all its suffixes, plus "johnd" and its suffixes
            // to force fallback to email prefix
            for username in ["johndoe", "johnd"] {
                let user = users::Model {
                    id: Uuid::new_v4(),
                    username: username.to_string(),
                    email: format!("{}@ex.com", username),
                    password_hash: "hash".to_string(),
                    role: "reader".to_string(),
                    is_active: true,
                    email_verified: false,
                    permissions: serde_json::json!([]),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    last_login_at: None,
                };
                UserRepository::create(&db, &user).await.unwrap();
            }

            // "johndoe" is taken -> tries "johndoe_1" which is available
            let result = generate_unique_username(
                &db,
                Some("johndoe"),
                Some("John D"),
                "newuser@example.com",
            )
            .await
            .unwrap();
            assert_eq!(result, "johndoe_1");
        }

        #[tokio::test]
        async fn test_generate_unique_username_with_suffix() {
            let db = setup_test_db().await;

            // Create user "testuser" to force numeric suffix
            let user = users::Model {
                id: Uuid::new_v4(),
                username: "testuser".to_string(),
                email: "existing@example.com".to_string(),
                password_hash: "hash".to_string(),
                role: "reader".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            };
            UserRepository::create(&db, &user).await.unwrap();

            let result =
                generate_unique_username(&db, Some("testuser"), None, "testuser@example.com")
                    .await
                    .unwrap();
            // Should get testuser_1 since testuser is taken
            assert_eq!(result, "testuser_1");
        }

        #[tokio::test]
        async fn test_generate_unique_username_no_inputs() {
            let db = setup_test_db().await;

            let result = generate_unique_username(
                &db,
                None,        // No preferred username
                None,        // No display name
                "@nodomain", // Email with no useful prefix
            )
            .await
            .unwrap();
            // Should generate a random username starting with "user_"
            assert!(result.starts_with("user_"));
        }

        #[tokio::test]
        async fn test_sync_role_from_groups_updates_role() {
            let db = setup_test_db().await;

            let user = users::Model {
                id: Uuid::new_v4(),
                username: "roletest".to_string(),
                email: "role@example.com".to_string(),
                password_hash: "hash".to_string(),
                role: "reader".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            };
            let created = UserRepository::create(&db, &user).await.unwrap();

            // Sync role to admin
            let updated = sync_role_from_groups(&db, created, "admin").await.unwrap();
            assert_eq!(updated.role, "admin");

            // Verify persisted
            let fetched = UserRepository::get_by_id(&db, updated.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(fetched.role, "admin");
        }

        #[tokio::test]
        async fn test_sync_role_from_groups_no_change() {
            let db = setup_test_db().await;

            let user = users::Model {
                id: Uuid::new_v4(),
                username: "norolechange".to_string(),
                email: "nochange@example.com".to_string(),
                password_hash: "hash".to_string(),
                role: "admin".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            };
            let created = UserRepository::create(&db, &user).await.unwrap();
            let original_updated_at = created.updated_at;

            // Sync with same role - should not update
            let result = sync_role_from_groups(&db, created, "admin").await.unwrap();
            assert_eq!(result.role, "admin");
            assert_eq!(result.updated_at, original_updated_at);
        }

        #[tokio::test]
        async fn test_sync_role_downgrades() {
            let db = setup_test_db().await;

            let user = users::Model {
                id: Uuid::new_v4(),
                username: "downgrade".to_string(),
                email: "downgrade@example.com".to_string(),
                password_hash: "hash".to_string(),
                role: "admin".to_string(),
                is_active: true,
                email_verified: false,
                permissions: serde_json::json!([]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_login_at: None,
            };
            let created = UserRepository::create(&db, &user).await.unwrap();

            // Role downgrade from admin to reader
            let updated = sync_role_from_groups(&db, created, "reader").await.unwrap();
            assert_eq!(updated.role, "reader");
        }
    }
}
