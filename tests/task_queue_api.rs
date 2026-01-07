mod common;

use codex::db::repositories::{TaskRepository, UserRepository};
use codex::tasks::types::TaskType;
use codex::utils::password;
use common::{
    create_test_app_state, create_test_router_with_app_state, create_test_user_with_permissions,
    delete_request_with_auth, get_request_with_auth, make_json_request, make_request,
    post_json_request_with_auth, post_request_with_auth, setup_test_db,
};
use hyper::StatusCode;
use serde_json::json;
use uuid::Uuid;

/// Test listing tasks via API
#[tokio::test]
async fn test_api_list_tasks() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user and get auth token
    let password = "test_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "testuser",
        "test@example.com",
        &password_hash,
        false,
        vec!["tasks-read".to_string()],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login to get token
    let login_request = json!({
        "username": "testuser",
        "password": password,
    });
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    // Create some tasks
    TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to create task");

    let request = get_request_with_auth("/api/v1/tasks", &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
}

/// Test getting task by ID via API
#[tokio::test]
async fn test_api_get_task() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create admin user
    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "admin",
        "admin@example.com",
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "admin", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a task
    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to create task");

    let request = get_request_with_auth(&format!("/api/v1/tasks/{}", task_id), &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
}

/// Test creating task via API
#[tokio::test]
async fn test_api_create_task() {
    let (db, _temp_dir) = setup_test_db().await;

    let password = "test_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "testuser",
        "test@example.com",
        &password_hash,
        false,
        vec!["tasks-write".to_string()],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "testuser", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    let create_request = json!({
        "task_type": {
            "type": "generate_thumbnails",
            "library_id": null
        },
        "priority": 5
    });

    let request = post_json_request_with_auth("/api/v1/tasks", &create_request, &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
    assert!(response.unwrap()["task_id"].is_string());
}

/// Test getting task stats via API
#[tokio::test]
async fn test_api_task_stats() {
    let (db, _temp_dir) = setup_test_db().await;

    let password = "test_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "testuser",
        "test@example.com",
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "testuser", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    let request = get_request_with_auth("/api/v1/tasks/stats", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let stats = response.unwrap();
    assert!(stats["pending"].is_number());
    assert!(stats["processing"].is_number());
    assert!(stats["completed"].is_number());
    assert!(stats["failed"].is_number());
    assert!(stats["stale"].is_number());
    assert!(stats["total"].is_number());
    assert!(stats["by_type"].is_object());
}

/// Test cancelling task via API
#[tokio::test]
async fn test_api_cancel_task() {
    let (db, _temp_dir) = setup_test_db().await;

    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "admin",
        "admin@example.com",
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "admin", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a task
    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to create task");

    let request = post_request_with_auth(&format!("/api/v1/tasks/{}/cancel", task_id), &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
}

/// Test unlocking task via API
#[tokio::test]
async fn test_api_unlock_task() {
    let (db, _temp_dir) = setup_test_db().await;

    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "admin",
        "admin@example.com",
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "admin", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    // Create and claim a task
    let task_id = TaskRepository::enqueue(
        &db,
        TaskType::GenerateThumbnails { library_id: None },
        0,
        None,
    )
    .await
    .expect("Failed to create task");

    TaskRepository::claim_next(&db, "worker-1", 300)
        .await
        .unwrap();

    let request = post_request_with_auth(&format!("/api/v1/tasks/{}/unlock", task_id), &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
}

/// Test purging old tasks via API
#[tokio::test]
async fn test_api_purge_tasks() {
    let (db, _temp_dir) = setup_test_db().await;

    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "admin",
        "admin@example.com",
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "admin", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    let request = delete_request_with_auth("/api/v1/tasks/purge?days=30", &token);
    let (status, _body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
}

/// Test nuking all tasks via API (admin only)
#[tokio::test]
async fn test_api_nuke_tasks_admin_only() {
    let (db, _temp_dir) = setup_test_db().await;

    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "admin",
        "admin@example.com",
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone());
    let app = create_test_router_with_app_state(state.clone());

    // Login
    let login_request = json!({"username": "admin", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    let token = response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    let request = delete_request_with_auth("/api/v1/tasks/nuke", &token);
    let (status, _body) = make_request(app, request).await;

    // Should succeed for admin
    assert_eq!(status, StatusCode::OK);
}
