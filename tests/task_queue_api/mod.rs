#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{
    BookMetadataRepository, LibraryRepository, SeriesRepository, TaskRepository, UserRepository,
};
use codex::models::ScanningStrategy;
use codex::tasks::types::TaskType;
use codex::utils::password;
use common::{
    create_test_app_state, create_test_book_with_hash, create_test_router_with_app_state,
    create_test_user_with_permissions, delete_request_with_auth, get_request_with_auth,
    make_json_request, make_request, post_json_request_with_auth, post_request_with_auth,
    setup_test_db,
};
use hyper::StatusCode;
use serde_json::json;

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

    let state = create_test_app_state(db.clone()).await;
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
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
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

    let state = create_test_app_state(db.clone()).await;
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
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
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

    let state = create_test_app_state(db.clone()).await;
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
        "taskType": {
            "type": "generate_thumbnails",
            "libraryId": null
        },
        "priority": 5
    });

    let request = post_json_request_with_auth("/api/v1/tasks", &create_request, &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
    assert!(response.unwrap()["taskId"].is_string());
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

    let state = create_test_app_state(db.clone()).await;
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
    assert!(stats["byType"].is_object());
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

    let state = create_test_app_state(db.clone()).await;
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
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
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

    let state = create_test_app_state(db.clone()).await;
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
        TaskType::GenerateThumbnails {
            library_id: None,
            series_id: None,
            series_ids: None,
            book_ids: None,
            force: false,
        },
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

    let state = create_test_app_state(db.clone()).await;
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

    let state = create_test_app_state(db.clone()).await;
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

/// Verify that GET /api/v1/tasks resolves bookTitle / seriesTitle / libraryName
/// from the joined metadata tables, so the active-tasks UI can render labels
/// without follow-up requests.
#[tokio::test]
async fn test_api_list_tasks_resolves_target_titles() {
    let (db, _temp_dir) = setup_test_db().await;

    // Auth: a user with tasks-read permission
    let password = "test_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        "tasks_reader",
        "tasks_reader@example.com",
        &password_hash,
        false,
        vec!["tasks-read".to_string()],
    );
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());

    let login_request = json!({"username": "tasks_reader", "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (login_status, login_response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(login_status, StatusCode::OK);
    let token = login_response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();

    // Build a library / series / book with metadata so the joins have something to resolve.
    let library = LibraryRepository::create(
        &db,
        "Manga Library",
        "/lib/manga",
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Naruto", None)
        .await
        .unwrap();
    let book = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "ignored",
        "/lib/manga/naruto/v12.cbz",
        "hash_v12",
    )
    .await;
    BookMetadataRepository::create_with_title_and_number(
        &db,
        book.id,
        Some("Naruto Vol. 12".to_string()),
        None,
    )
    .await
    .unwrap();

    // Enqueue tasks at all three scopes so we can assert each title field independently.
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeBook {
            book_id: book.id,
            force: false,
        },
        None,
    )
    .await
    .unwrap();
    TaskRepository::enqueue(
        &db,
        TaskType::AnalyzeSeries {
            series_id: series.id,
        },
        None,
    )
    .await
    .unwrap();
    TaskRepository::enqueue(
        &db,
        TaskType::ScanLibrary {
            library_id: library.id,
            mode: "normal".to_string(),
        },
        None,
    )
    .await
    .unwrap();

    let request = get_request_with_auth("/api/v1/tasks", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let body = body.expect("response body");
    let tasks = body.as_array().expect("array of tasks");
    assert!(
        tasks.len() >= 3,
        "expected at least three tasks, got {}",
        tasks.len()
    );

    let book_task = tasks
        .iter()
        .find(|t| t["taskType"] == "analyze_book")
        .expect("analyze_book task missing");
    assert_eq!(book_task["bookTitle"], "Naruto Vol. 12");
    // seriesTitle / libraryName are skipped via skip_serializing_if when None.
    assert!(book_task.get("seriesTitle").is_none());
    assert!(book_task.get("libraryName").is_none());

    let series_task = tasks
        .iter()
        .find(|t| t["taskType"] == "analyze_series")
        .expect("analyze_series task missing");
    assert_eq!(series_task["seriesTitle"], "Naruto");
    assert!(series_task.get("bookTitle").is_none());
    assert!(series_task.get("libraryName").is_none());

    let library_task = tasks
        .iter()
        .find(|t| t["taskType"] == "scan_library")
        .expect("scan_library task missing");
    assert_eq!(library_task["libraryName"], "Manga Library");
    assert!(library_task.get("bookTitle").is_none());
    assert!(library_task.get("seriesTitle").is_none());
}

// ============================================================================
// Surfaces that only work because finished tasks now survive
// ============================================================================

/// Log in as an admin and return the token.
async fn admin_token(
    db: &sea_orm::DatabaseConnection,
    app: axum::Router,
    username: &str,
) -> String {
    let password = "admin_password";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user_with_permissions(
        username,
        &format!("{username}@example.com"),
        &password_hash,
        true,
        vec![],
    );
    UserRepository::create(db, &user).await.unwrap();

    let login_request = json!({"username": username, "password": password});
    let request = post_json_request_with_auth("/api/v1/auth/login", &login_request, "");
    let (_, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    response.unwrap()["accessToken"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Drive a task into the terminal `failed` state.
///
/// `mark_failed` reschedules a first attempt rather than failing it, so a task
/// only reaches `failed` once its attempts are exhausted. Set the row directly.
async fn force_failed(db: &sea_orm::DatabaseConnection, task_id: uuid::Uuid, error: &str) {
    use codex::db::entities::tasks;
    use sea_orm::{ActiveModelTrait, Set};

    let task = TaskRepository::get_by_id(db, task_id)
        .await
        .unwrap()
        .unwrap();
    let mut active: tasks::ActiveModel = task.into();
    active.status = Set("failed".to_string());
    active.last_error = Set(Some(error.to_string()));
    active.completed_at = Set(Some(chrono::Utc::now()));
    active.update(db).await.unwrap();
}

/// A failed task has to be listable and retryable. Both were unreachable while
/// the cleanup sweep deleted finished tasks after ten seconds: `last_error` is
/// the only record of why a task failed, and retry rejects anything that is not
/// `failed`.
#[tokio::test]
async fn test_api_failed_task_can_be_listed_and_retried() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());
    let token = admin_token(&db, app.clone(), "admin").await;

    let task_id = TaskRepository::enqueue(&db, TaskType::CleanupPluginData, None)
        .await
        .unwrap();
    force_failed(&db, task_id, "disk went away").await;

    // The failure is visible, along with the reason.
    let request = get_request_with_auth("/api/v1/tasks?status=failed", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);
    let tasks = response.unwrap();
    let listed = tasks
        .as_array()
        .expect("task array")
        .iter()
        .find(|t| t["id"].as_str() == Some(&task_id.to_string()))
        .expect("the failed task must be listed");
    assert_eq!(
        listed["lastError"], "disk went away",
        "the reason for a failure is only recorded on the task row",
    );

    // And it can be retried.
    let request = post_request_with_auth(&format!("/api/v1/tasks/{}/retry", task_id), &token);
    let (status, _) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK, "a failed task must be retryable");

    let after = TaskRepository::get_by_id(&db, task_id)
        .await
        .unwrap()
        .expect("retried task still exists");
    assert_eq!(
        after.status, "pending",
        "retry puts the task back in the queue"
    );
}

/// The purge endpoint now speaks the same unit as the retention setting. It
/// spoke in days while the automatic sweep spoke in seconds, so it could never
/// delete anything: nothing survived long enough to be a day old.
#[tokio::test]
async fn test_api_purge_tasks_accepts_seconds_and_deletes() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());
    let token = admin_token(&db, app.clone(), "admin").await;

    let task_id = TaskRepository::enqueue(&db, TaskType::CleanupPluginData, None)
        .await
        .unwrap();
    force_failed(&db, task_id, "boom").await;

    let request = delete_request_with_auth("/api/v1/tasks/purge?seconds=0", &token);
    let (status, body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let parsed: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(
        parsed["deleted"], 1,
        "a cutoff of zero seconds must delete a task that has already finished",
    );
    assert!(
        TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .is_none(),
        "the task is gone",
    );
}

/// `days` still works, so an existing caller is unaffected by the new unit.
#[tokio::test]
async fn test_api_purge_tasks_still_accepts_days() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state.clone());
    let token = admin_token(&db, app.clone(), "admin").await;

    let task_id = TaskRepository::enqueue(&db, TaskType::CleanupPluginData, None)
        .await
        .unwrap();
    force_failed(&db, task_id, "boom").await;

    // Thirty days: the task finished moments ago, so it must survive.
    let request = delete_request_with_auth("/api/v1/tasks/purge?days=30", &token);
    let (status, body) = make_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let parsed: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(parsed["deleted"], 0);
    assert!(
        TaskRepository::get_by_id(&db, task_id)
            .await
            .unwrap()
            .is_some(),
        "a task that finished seconds ago is not thirty days old",
    );
}
