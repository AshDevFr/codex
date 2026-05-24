//! Integration tests for access group endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::access_group::{
    AccessGroupDetailDto, AccessGroupDto, AccessGroupGrantDto, AccessGroupOidcMappingDto,
    AccessGroupSummaryDto, AddAccessGroupGrantRequest, AddAccessGroupMembersRequest,
    AddAccessGroupOidcMappingRequest, CreateAccessGroupRequest, EffectiveGrantsResponse,
    UpdateAccessGroupRequest,
};
use codex::db::entities::user_sharing_tags::AccessMode;
use codex::db::repositories::{SharingTagRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create admin and token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

// Helper to create a non-admin user and token
async fn create_reader_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("reader123").unwrap();
    let user = create_test_user(
        username,
        &format!("{}@example.com", username),
        &password_hash,
        false,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

// ============================================================================
// Access Group CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_list_access_groups_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/access-groups", &token);
    let (status, response): (StatusCode, Option<Vec<AccessGroupDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.unwrap().is_empty());
}

#[tokio::test]
async fn test_create_access_group() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let body = CreateAccessGroupRequest {
        name: "Manga Readers".to_string(),
        description: Some("Access to manga content".to_string()),
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (status, response): (StatusCode, Option<AccessGroupDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let group = response.unwrap();
    assert_eq!(group.name, "Manga Readers");
    assert_eq!(
        group.description.as_deref(),
        Some("Access to manga content")
    );
    assert_eq!(group.member_count, 0);
    assert_eq!(group.grant_count, 0);
}

#[tokio::test]
async fn test_create_access_group_duplicate_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;

    let body = CreateAccessGroupRequest {
        name: "Manga Readers".to_string(),
        description: None,
    };

    // First creation succeeds
    let app = create_test_router(state.clone()).await;
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (status, _): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);

    // Second creation fails
    let app = create_test_router(state).await;
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_access_group_detail() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;

    // Create a group
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Staff".to_string(),
        description: Some("Library staff".to_string()),
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Get detail
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (status, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let detail = response.unwrap();
    assert_eq!(detail.id, group.id);
    assert_eq!(detail.name, "Staff");
    assert!(detail.members.is_empty());
    assert!(detail.grants.is_empty());
    assert!(detail.oidc_mappings.is_empty());
}

#[tokio::test]
async fn test_update_access_group() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;

    // Create
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Old Name".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Update
    let app = create_test_router(state).await;
    let update = UpdateAccessGroupRequest {
        name: Some("New Name".to_string()),
        description: Some(Some("Now with description".to_string())),
    };
    let request = patch_json_request_with_auth(
        &format!("/api/v1/access-groups/{}", group.id),
        &update,
        &token,
    );
    let (status, response): (StatusCode, Option<AccessGroupDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.unwrap();
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.description.as_deref(), Some("Now with description"));
}

#[tokio::test]
async fn test_delete_access_group() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;

    // Create
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Doomed".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Delete
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify gone
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_access_group_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        &format!("/api/v1/access-groups/{}", uuid::Uuid::new_v4()),
        &token,
    );
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Permission Tests
// ============================================================================

#[tokio::test]
async fn test_access_groups_forbidden_for_non_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, reader_token) = create_reader_and_token(&db, &state, "reader1").await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/access-groups", &reader_token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_access_groups_unauthorized_without_token() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/access-groups");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Member Tests
// ============================================================================

#[tokio::test]
async fn test_add_and_remove_members() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    // Create group
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Test Group".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Add member
    let app = create_test_router(state.clone()).await;
    let add_body = AddAccessGroupMembersRequest {
        user_ids: vec![user_id],
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/members", group.id),
        &add_body,
        &token,
    );
    let (status, response): (
        StatusCode,
        Option<Vec<codex::api::routes::v1::dto::access_group::AccessGroupMemberDto>>,
    ) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let members = response.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user_id, user_id);
    assert_eq!(members[0].username, "alice");
    assert_eq!(members[0].source, "manual");

    // Verify in group detail
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (_, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;
    assert_eq!(response.unwrap().members.len(), 1);

    // Remove member
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(
        &format!("/api/v1/access-groups/{}/members/{}", group.id, user_id),
        &token,
    );
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify removed
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (_, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;
    assert!(response.unwrap().members.is_empty());
}

// ============================================================================
// Grant Tests
// ============================================================================

#[tokio::test]
async fn test_add_and_remove_grants() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;

    // Create sharing tag
    let tag = SharingTagRepository::create(&db, "manga", None)
        .await
        .unwrap();

    // Create group
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Manga Group".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Add grant
    let app = create_test_router(state.clone()).await;
    let grant_body = AddAccessGroupGrantRequest {
        sharing_tag_id: tag.id,
        access_mode: AccessMode::Allow,
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/grants", group.id),
        &grant_body,
        &token,
    );
    let (status, response): (StatusCode, Option<AccessGroupGrantDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let grant = response.unwrap();
    assert_eq!(grant.sharing_tag_id, tag.id);
    assert_eq!(grant.sharing_tag_name, "manga");
    assert_eq!(grant.access_mode, AccessMode::Allow);

    // Verify in detail
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (_, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;
    let detail = response.unwrap();
    assert_eq!(detail.grants.len(), 1);
    assert_eq!(detail.grants.len(), 1);

    // Remove grant
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(
        &format!("/api/v1/access-groups/{}/grants/{}", group.id, tag.id),
        &token,
    );
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify removed
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (_, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;
    assert!(response.unwrap().grants.is_empty());
}

// ============================================================================
// OIDC Mapping Tests
// ============================================================================

#[tokio::test]
async fn test_add_and_remove_oidc_mappings() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;

    // Create group
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Staff".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Add OIDC mapping
    let app = create_test_router(state.clone()).await;
    let oidc_body = AddAccessGroupOidcMappingRequest {
        oidc_group_name: "library-staff".to_string(),
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/oidc-mappings", group.id),
        &oidc_body,
        &token,
    );
    let (status, response): (StatusCode, Option<AccessGroupOidcMappingDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let mapping = response.unwrap();
    assert_eq!(mapping.oidc_group_name, "library-staff");

    // Verify in detail
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (_, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;
    assert_eq!(response.unwrap().oidc_mappings.len(), 1);

    // Remove OIDC mapping
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(
        &format!(
            "/api/v1/access-groups/{}/oidc-mappings/{}",
            group.id, mapping.id
        ),
        &token,
    );
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify removed
    let app = create_test_router(state).await;
    let request = get_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (_, response): (StatusCode, Option<AccessGroupDetailDto>) =
        make_json_request(app, request).await;
    assert!(response.unwrap().oidc_mappings.is_empty());
}

// ============================================================================
// User Access Groups Tests
// ============================================================================

#[tokio::test]
async fn test_get_user_access_groups() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    // Create two groups
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Group A".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group_a = response.unwrap();

    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Group B".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group_b = response.unwrap();

    // Add user to both groups
    for gid in [group_a.id, group_b.id] {
        let app = create_test_router(state.clone()).await;
        let add_body = AddAccessGroupMembersRequest {
            user_ids: vec![user_id],
        };
        let request = post_json_request_with_auth(
            &format!("/api/v1/access-groups/{}/members", gid),
            &add_body,
            &token,
        );
        let (status, _): (StatusCode, Option<serde_json::Value>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Get user's groups
    let app = create_test_router(state).await;
    let request =
        get_request_with_auth(&format!("/api/v1/users/{}/access-groups", user_id), &token);
    let (status, response): (StatusCode, Option<Vec<AccessGroupSummaryDto>>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let groups = response.unwrap();
    assert_eq!(groups.len(), 2);
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"Group A"));
    assert!(names.contains(&"Group B"));
}

// ============================================================================
// Effective Grants Tests
// ============================================================================

#[tokio::test]
async fn test_effective_grants_user_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    // Create a tag and grant it to the user directly
    let tag = SharingTagRepository::create(&db, "manga", None)
        .await
        .unwrap();
    SharingTagRepository::set_user_grant(&db, user_id, tag.id, AccessMode::Allow)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/users/{}/effective-grants", user_id),
        &token,
    );
    let (status, response): (StatusCode, Option<EffectiveGrantsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let grants_resp = response.unwrap();
    assert_eq!(grants_resp.user_id, user_id);
    assert_eq!(grants_resp.grants.len(), 1);
    assert_eq!(grants_resp.grants[0].sharing_tag_name, "manga");
    assert_eq!(grants_resp.grants[0].access_mode, AccessMode::Allow);
    assert_eq!(grants_resp.grants[0].sources.len(), 1);
    assert_eq!(grants_resp.grants[0].sources[0].kind, "user");
}

#[tokio::test]
async fn test_effective_grants_with_group_and_user_sources() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    // Create tags
    let manga_tag = SharingTagRepository::create(&db, "manga", None)
        .await
        .unwrap();
    let mature_tag = SharingTagRepository::create(&db, "18+", None)
        .await
        .unwrap();

    // Create group with manga allow
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Manga Readers".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Add manga allow grant to group
    let app = create_test_router(state.clone()).await;
    let grant_body = AddAccessGroupGrantRequest {
        sharing_tag_id: manga_tag.id,
        access_mode: AccessMode::Allow,
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/grants", group.id),
        &grant_body,
        &token,
    );
    let (status, _): (StatusCode, Option<AccessGroupGrantDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Add user to group
    let app = create_test_router(state.clone()).await;
    let add_body = AddAccessGroupMembersRequest {
        user_ids: vec![user_id],
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/members", group.id),
        &add_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Add user-level deny for 18+
    SharingTagRepository::set_user_grant(&db, user_id, mature_tag.id, AccessMode::Deny)
        .await
        .unwrap();

    // Get effective grants
    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/users/{}/effective-grants", user_id),
        &token,
    );
    let (status, response): (StatusCode, Option<EffectiveGrantsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let grants_resp = response.unwrap();
    assert_eq!(grants_resp.grants.len(), 2);

    // Find the manga grant (from group)
    let manga_grant = grants_resp
        .grants
        .iter()
        .find(|g| g.sharing_tag_name == "manga")
        .expect("manga grant not found");
    assert_eq!(manga_grant.access_mode, AccessMode::Allow);
    assert_eq!(manga_grant.sources.len(), 1);
    assert_eq!(manga_grant.sources[0].kind, "group");
    assert_eq!(
        manga_grant.sources[0].group_name.as_deref(),
        Some("Manga Readers")
    );

    // Find the 18+ grant (from user)
    let mature_grant = grants_resp
        .grants
        .iter()
        .find(|g| g.sharing_tag_name == "18+")
        .expect("18+ grant not found");
    assert_eq!(mature_grant.access_mode, AccessMode::Deny);
    assert_eq!(mature_grant.sources.len(), 1);
    assert_eq!(mature_grant.sources[0].kind, "user");
}

#[tokio::test]
async fn test_effective_grants_no_grants() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/users/{}/effective-grants", user_id),
        &token,
    );
    let (status, response): (StatusCode, Option<EffectiveGrantsResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let grants_resp = response.unwrap();
    assert!(grants_resp.grants.is_empty());
}

// ============================================================================
// Cascade Delete Test
// ============================================================================

#[tokio::test]
async fn test_delete_group_cascades_memberships_and_grants() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    let tag = SharingTagRepository::create(&db, "manga", None)
        .await
        .unwrap();

    // Create group, add member and grant
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Doomed Group".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Add member
    let app = create_test_router(state.clone()).await;
    let add_body = AddAccessGroupMembersRequest {
        user_ids: vec![user_id],
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/members", group.id),
        &add_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Add grant
    let app = create_test_router(state.clone()).await;
    let grant_body = AddAccessGroupGrantRequest {
        sharing_tag_id: tag.id,
        access_mode: AccessMode::Allow,
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/grants", group.id),
        &grant_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Delete the group
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(&format!("/api/v1/access-groups/{}", group.id), &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify user no longer in any groups
    let app = create_test_router(state).await;
    let request =
        get_request_with_auth(&format!("/api/v1/users/{}/access-groups", user_id), &token);
    let (status, response): (StatusCode, Option<Vec<AccessGroupSummaryDto>>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(response.unwrap().is_empty());
}

// ============================================================================
// List with Counts
// ============================================================================

#[tokio::test]
async fn test_list_access_groups_with_counts() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_admin_and_token(&db, &state).await;
    let (user_id, _) = create_reader_and_token(&db, &state, "alice").await;

    let tag = SharingTagRepository::create(&db, "manga", None)
        .await
        .unwrap();

    // Create group
    let app = create_test_router(state.clone()).await;
    let body = CreateAccessGroupRequest {
        name: "Manga Readers".to_string(),
        description: None,
    };
    let request = post_json_request_with_auth("/api/v1/access-groups", &body, &token);
    let (_, response): (StatusCode, Option<AccessGroupDto>) = make_json_request(app, request).await;
    let group = response.unwrap();

    // Add member and grant
    let app = create_test_router(state.clone()).await;
    let add_body = AddAccessGroupMembersRequest {
        user_ids: vec![user_id],
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/members", group.id),
        &add_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let app = create_test_router(state.clone()).await;
    let grant_body = AddAccessGroupGrantRequest {
        sharing_tag_id: tag.id,
        access_mode: AccessMode::Allow,
    };
    let request = post_json_request_with_auth(
        &format!("/api/v1/access-groups/{}/grants", group.id),
        &grant_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // List groups and verify counts
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/access-groups", &token);
    let (status, response): (StatusCode, Option<Vec<AccessGroupDto>>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let groups = response.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].member_count, 1);
    assert_eq!(groups[0].grant_count, 1);
}
