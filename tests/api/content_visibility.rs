//! What a restricted reader can reach, on every surface.
//!
//! One fixture: a reader in a group that denies the Adult genre and the whole
//! "Secret" library. Each surface is then asked about a visible series/book
//! and a hidden one. A hidden item must answer exactly like a missing one
//! (404, or absent from a list); the visible one must still work, so an
//! endpoint that is simply broken cannot pass.

#[path = "../common/mod.rs"]
mod common;

use codex::db::ScanningStrategy;
use codex::db::entities::user_access_groups::MembershipSource;
use codex::db::entities::user_sharing_tags::AccessMode;
use codex::db::repositories::{
    AccessGroupRepository, AccessGroupRules, AccessGroupRulesRepository, BookMetadataRepository,
    BookRepository, GenreRepository, LibraryRepository, LibraryRule, NameRule, PageRepository,
    SeriesRepository, TagRepository, UserRepository,
};
use common::*;
use hyper::StatusCode;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

pub struct Item {
    pub series_id: Uuid,
    pub book_id: Uuid,
    pub library_id: Uuid,
}

pub struct World {
    pub state: Arc<codex::api::extractors::AppState>,
    pub reader_token: String,
    pub reader_id: Uuid,
    /// In the main library, no Adult genre
    pub visible: Item,
    /// In the main library, Adult genre
    pub hidden: Item,
    /// In the denied library
    pub secret: Item,
    _db_dir: TempDir,
    _files: TempDir,
}

impl World {
    pub async fn new() -> Self {
        let (db, db_dir) = setup_test_db().await;
        let files = TempDir::new().unwrap();

        let main = LibraryRepository::create(
            &db,
            "Main",
            files.path().join("main").to_str().unwrap(),
            ScanningStrategy::Default,
        )
        .await
        .unwrap();
        let secret_lib = LibraryRepository::create(
            &db,
            "Secret",
            files.path().join("secret").to_str().unwrap(),
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let visible = seed(&db, &files, main.id, "Visible", &["Action"], "shonen").await;
        let hidden = seed(&db, &files, main.id, "Hidden", &["Adult"], "mature").await;
        let secret = seed(&db, &files, secret_lib.id, "Secret series", &[], "secret").await;

        let password_hash = codex::utils::password::hash_password("reader123").unwrap();
        let reader = UserRepository::create(
            &db,
            &create_test_user("kid", "kid@example.com", &password_hash, false),
        )
        .await
        .unwrap();
        let group = AccessGroupRepository::create(&db, "Kids", None)
            .await
            .unwrap();
        AccessGroupRulesRepository::replace(
            &db,
            group.id,
            &AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                libraries: vec![LibraryRule {
                    library_id: secret_lib.id,
                    access_mode: AccessMode::Deny,
                }],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        AccessGroupRepository::add_member(&db, group.id, reader.id, MembershipSource::Manual)
            .await
            .unwrap();

        let state = create_test_app_state(db.clone()).await;
        let reader_token = state
            .jwt_service
            .generate_token(reader.id, reader.username.clone(), reader.get_role())
            .unwrap();

        Self {
            state,
            reader_token,
            reader_id: reader.id,
            visible,
            hidden,
            secret,
            _db_dir: db_dir,
            _files: files,
        }
    }

    /// Router with every optional API enabled.
    pub fn router(&self) -> axum::Router {
        let mut config = create_test_config_with_komga();
        config.koreader_api.enabled = true;
        codex::api::routes::create_router(self.state.clone(), &config)
    }

    pub fn path(&self, template: &str, item: &Item) -> String {
        template
            .replace("{series}", &item.series_id.to_string())
            .replace("{book}", &item.book_id.to_string())
            .replace("{library}", &item.library_id.to_string())
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, Vec<u8>) {
        let request = get_request_with_auth(uri, &self.reader_token);
        make_raw_request(self.router(), request).await
    }

    pub async fn get_json(&self, uri: &str) -> (StatusCode, serde_json::Value) {
        let (status, body) = self.get(uri).await;
        let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }
}

async fn seed(
    db: &sea_orm::DatabaseConnection,
    files: &TempDir,
    library_id: Uuid,
    name: &str,
    genres: &[&str],
    tag: &str,
) -> Item {
    let series = SeriesRepository::create(db, library_id, name, None)
        .await
        .unwrap();
    for genre in genres {
        GenreRepository::add_genre_to_series(db, series.id, genre)
            .await
            .unwrap();
    }
    TagRepository::add_tag_to_series(db, series.id, tag)
        .await
        .unwrap();

    let dir = TempDir::new_in(files.path()).unwrap();
    let cbz = create_test_cbz(&dir, 3, true);
    let file = files.path().join(format!("{}.cbz", Uuid::new_v4()));
    std::fs::copy(&cbz, &file).unwrap();
    let book = BookRepository::create(db, &book_model(series.id, library_id, &file, 3), None)
        .await
        .unwrap();
    BookMetadataRepository::create_with_title_and_number(db, book.id, Some(name.to_string()), None)
        .await
        .unwrap();
    for n in 1..=3 {
        PageRepository::create(
            db,
            &codex::db::entities::pages::Model {
                id: Uuid::new_v4(),
                book_id: book.id,
                page_number: n,
                file_name: format!("page{n:03}.png"),
                format: "png".to_string(),
                width: 800,
                height: 1200,
                file_size: 50_000,
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();
    }

    Item {
        series_id: series.id,
        book_id: book.id,
        library_id,
    }
}

fn book_model(
    series_id: Uuid,
    library_id: Uuid,
    path: &Path,
    page_count: i32,
) -> codex::db::entities::books::Model {
    let now = chrono::Utc::now();
    codex::db::entities::books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        path: path.to_str().unwrap().to_string(),
        file_name: path.file_name().unwrap().to_str().unwrap().to_string(),
        file_size: std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0),
        file_hash: format!("hash_{}", Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count,
        deleted: false,
        analyzed: true,
        analysis_error: None,
        analysis_errors: None,
        modified_at: now,
        created_at: now,
        updated_at: now,
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: Some(format!("kohash-{}", Uuid::new_v4())),
        epub_positions: None,
        epub_spine_items: None,
    }
}

/// Assert a by-id endpoint hides the hidden and secret items and still serves
/// the visible one. Collects every failure before panicking, so one run shows
/// the whole list.
async fn assert_by_id_reads(world: &World, templates: &[&str]) {
    let mut failures = Vec::new();
    for template in templates {
        let visible = world.get(&world.path(template, &world.visible)).await.0;
        if visible == StatusCode::NOT_FOUND || visible.is_server_error() {
            failures.push(format!("{template}: visible item answered {visible}"));
        }
        for (label, item) in [("hidden", &world.hidden), ("secret", &world.secret)] {
            let status = world.get(&world.path(template, item)).await.0;
            if status != StatusCode::NOT_FOUND {
                failures.push(format!("{template}: {label} item answered {status}"));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

// ============================================================================
// v1 by-id reads
// ============================================================================

#[tokio::test]
async fn v1_book_reads_hide_hidden_books() {
    let world = World::new().await;
    assert_by_id_reads(
        &world,
        &[
            "/api/v1/books/{book}",
            "/api/v1/books/{book}/full",
            "/api/v1/books/{book}/file",
            "/api/v1/books/{book}/pages",
            "/api/v1/books/{book}/pages/1",
            "/api/v1/books/{book}/thumbnail",
            "/api/v1/books/{book}/adjacent",
            "/api/v1/books/{book}/metadata/locks",
            "/api/v1/books/{book}/external-ids",
            "/api/v1/books/{book}/external-links",
            "/api/v1/books/{book}/covers",
            "/api/v1/books/{book}/genres",
            "/api/v1/books/{book}/tags",
            "/api/v1/books/{book}/readlists",
            "/api/v1/books/{book}/progress",
            "/api/v1/books/{book}/progression",
            "/api/v1/books/{book}/read-history",
        ],
    )
    .await;
}

#[tokio::test]
async fn v1_series_reads_hide_hidden_series() {
    let world = World::new().await;
    assert_by_id_reads(
        &world,
        &[
            "/api/v1/series/{series}",
            "/api/v1/series/{series}/full",
            "/api/v1/series/{series}/books",
            "/api/v1/series/{series}/thumbnail",
            "/api/v1/series/{series}/download",
            "/api/v1/series/{series}/metadata",
            "/api/v1/series/{series}/metadata/locks",
            "/api/v1/series/{series}/genres",
            "/api/v1/series/{series}/tags",
            "/api/v1/series/{series}/rating",
            "/api/v1/series/{series}/alternate-titles",
            "/api/v1/series/{series}/external-ratings",
            "/api/v1/series/{series}/ratings/average",
            "/api/v1/series/{series}/external-links",
            "/api/v1/series/{series}/external-ids",
            "/api/v1/series/{series}/covers",
            "/api/v1/series/{series}/collections",
            "/api/v1/series/{series}/tracking",
            "/api/v1/series/{series}/aliases",
            "/api/v1/series/{series}/releases",
            "/api/v1/series/{series}/read-history",
            "/api/v1/user/series/{series}/reader-settings",
        ],
    )
    .await;
}

/// Needs a plugin to succeed, so the visible series fails too; what matters is
/// that a hidden series fails as "not found" before any plugin is looked up.
#[tokio::test]
async fn v1_series_search_title_hides_hidden_series() {
    let world = World::new().await;
    let template = "/api/v1/series/{series}/metadata/search-title?pluginId=00000000-0000-0000-0000-000000000000";

    let (_, visible) = world.get_json(&world.path(template, &world.visible)).await;
    assert_ne!(visible["message"], "Series not found", "{visible}");

    for item in [&world.hidden, &world.secret] {
        let (status, body) = world.get_json(&world.path(template, item)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["message"], "Series not found", "{body}");
    }
}

// ============================================================================
// v1 writes a reader can make
// ============================================================================

impl World {
    pub async fn send(
        &self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = hyper::Request::builder()
            .method(method)
            .uri(uri)
            .header("Authorization", format!("Bearer {}", self.reader_token));
        if body.is_some() {
            builder = builder.header("Content-Type", "application/json");
        }
        let request = builder
            .body(body.map(|b| b.to_string()).unwrap_or_default())
            .unwrap();
        let (status, bytes) = make_raw_request(self.router(), request).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }
}

/// Like [`assert_by_id_reads`], for writes. The visible item goes last for
/// each endpoint so a write on it cannot mask what the hidden items answer.
async fn assert_by_id_writes(world: &World, cases: &[(&str, &str, Option<serde_json::Value>)]) {
    let mut failures = Vec::new();
    for (method, template, body) in cases {
        for (label, item) in [("hidden", &world.hidden), ("secret", &world.secret)] {
            let (status, _) = world
                .send(method, &world.path(template, item), body.clone())
                .await;
            if status != StatusCode::NOT_FOUND {
                failures.push(format!(
                    "{method} {template}: {label} item answered {status}"
                ));
            }
        }
        let (status, response) = world
            .send(method, &world.path(template, &world.visible), body.clone())
            .await;
        if status == StatusCode::NOT_FOUND || status.is_server_error() {
            failures.push(format!(
                "{method} {template}: visible item answered {status} {response}"
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[tokio::test]
async fn v1_reader_writes_refuse_hidden_content() {
    use serde_json::json;
    let world = World::new().await;
    assert_by_id_writes(
        &world,
        &[
            (
                "PUT",
                "/api/v1/books/{book}/progress",
                Some(json!({ "currentPage": 1 })),
            ),
            ("DELETE", "/api/v1/books/{book}/progress", None),
            ("POST", "/api/v1/books/{book}/read", None),
            ("POST", "/api/v1/books/{book}/unread", None),
            ("DELETE", "/api/v1/books/{book}/read-history", None),
            (
                "PUT",
                "/api/v1/series/{series}/rating",
                Some(json!({ "rating": 80 })),
            ),
            ("DELETE", "/api/v1/series/{series}/rating", None),
            ("POST", "/api/v1/series/{series}/read", None),
            ("POST", "/api/v1/series/{series}/unread", None),
            ("DELETE", "/api/v1/series/{series}/read-history", None),
            (
                "PATCH",
                "/api/v1/user/series/{series}/reader-settings",
                Some(json!({ "readingDirection": "rtl" })),
            ),
            (
                "DELETE",
                "/api/v1/user/series/{series}/reader-settings",
                None,
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn v1_want_to_read_refuses_hidden_content() {
    use serde_json::json;
    let world = World::new().await;

    for item in [&world.hidden, &world.secret] {
        let (status, _) = world
            .send(
                "POST",
                "/api/v1/want-to-read",
                Some(json!({ "seriesId": item.series_id })),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = world
            .send(
                "POST",
                "/api/v1/want-to-read",
                Some(json!({ "bookId": item.book_id })),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    let (status, body) = world
        .send(
            "POST",
            "/api/v1/want-to-read/bulk",
            Some(json!({
                "seriesIds": [world.visible.series_id, world.hidden.series_id, world.secret.series_id],
                "bookIds": [world.visible.book_id, world.hidden.book_id, world.secret.book_id],
            })),
        )
        .await;
    assert!(status.is_success(), "{status} {body}");
    assert_eq!(body["added"], 2, "only the visible series and book: {body}");
}

#[tokio::test]
async fn v1_bulk_read_skips_hidden_content() {
    use serde_json::json;
    let world = World::new().await;
    let all_books = json!({
        "bookIds": [world.visible.book_id, world.hidden.book_id, world.secret.book_id]
    });
    let all_series = json!({
        "seriesIds": [world.visible.series_id, world.hidden.series_id, world.secret.series_id]
    });

    // Each step changes only the visible item, so each counts exactly one.
    let (_, body) = world
        .send("POST", "/api/v1/series/bulk/read", Some(all_series.clone()))
        .await;
    assert_eq!(body["count"], 1, "series read: {body}");
    let (_, body) = world
        .send("POST", "/api/v1/series/bulk/unread", Some(all_series))
        .await;
    assert_eq!(body["count"], 1, "series unread: {body}");
    let (_, body) = world
        .send("POST", "/api/v1/books/bulk/read", Some(all_books.clone()))
        .await;
    assert_eq!(body["count"], 1, "books read: {body}");
    let (_, body) = world
        .send("POST", "/api/v1/books/bulk/unread", Some(all_books))
        .await;
    assert_eq!(body["count"], 1, "books unread: {body}");
}

#[tokio::test]
async fn v1_reading_sessions_reject_hidden_books() {
    use serde_json::json;
    let world = World::new().await;
    let session = |book_id: Uuid| {
        let now = chrono::Utc::now();
        json!({
            "id": Uuid::new_v4(),
            "bookId": book_id,
            "deviceId": "device-1",
            "kind": "progress",
            "toPage": 2,
            "clientStartedAt": (now - chrono::Duration::minutes(5)).to_rfc3339(),
            "clientEndedAt": now.to_rfc3339(),
            "activeDurationMs": 60000,
            "pagesRead": 1
        })
    };
    let (status, body) = world
        .send(
            "POST",
            "/api/v1/reading-sessions",
            Some(json!({ "sessions": [
                session(world.visible.book_id),
                session(world.hidden.book_id),
                session(world.secret.book_id),
            ]})),
        )
        .await;
    assert!(status.is_success(), "{status} {body}");
    let rejected = body["rejected"].as_array().cloned().unwrap_or_default();
    assert_eq!(rejected.len(), 2, "{body}");
    assert!(
        rejected.iter().all(|r| r["reason"] == "book_not_found"),
        "{body}"
    );
}

// ============================================================================
// OPDS 1.2 and 2.0 by-id reads
// ============================================================================

#[tokio::test]
async fn opds_reads_hide_hidden_content() {
    let world = World::new().await;
    assert_by_id_reads(
        &world,
        &[
            "/opds/series/{series}",
            "/opds/books/{book}/pages",
            "/opds/books/{book}/pages/1",
            "/opds/v2/series/{series}",
        ],
    )
    .await;
}

// ============================================================================
// Komga by-id reads and writes
// ============================================================================

#[tokio::test]
async fn komga_reads_hide_hidden_content() {
    let world = World::new().await;
    assert_by_id_reads(
        &world,
        &[
            "/komga/api/v1/series/{series}",
            "/komga/api/v1/series/{series}/thumbnail",
            "/komga/api/v1/series/{series}/books",
            "/komga/api/v1/series/{series}/collections",
            "/komga/api/v1/books/{book}",
            "/komga/api/v1/books/{book}/thumbnail",
            "/komga/api/v1/books/{book}/file",
            "/komga/api/v1/books/{book}/pages",
            "/komga/api/v1/books/{book}/pages/1",
            "/komga/api/v1/books/{book}/pages/1/thumbnail",
            "/komga/api/v1/books/{book}/readlists",
            "/komga/api/v1/books/{book}/progression",
        ],
    )
    .await;
}

#[tokio::test]
async fn komga_writes_refuse_hidden_content() {
    use serde_json::json;
    let world = World::new().await;
    assert_by_id_writes(
        &world,
        &[
            (
                "PATCH",
                "/komga/api/v1/books/{book}/read-progress",
                Some(json!({ "page": 1 })),
            ),
            ("DELETE", "/komga/api/v1/books/{book}/read-progress", None),
            ("POST", "/komga/api/v1/series/{series}/read-progress", None),
            (
                "DELETE",
                "/komga/api/v1/series/{series}/read-progress",
                None,
            ),
        ],
    )
    .await;
}

// ============================================================================
// Libraries
// ============================================================================

#[tokio::test]
async fn library_by_id_endpoints_hide_a_denied_library() {
    let world = World::new().await;
    let mut failures = Vec::new();
    for template in [
        "/api/v1/libraries/{library}",
        "/api/v1/libraries/{library}/series",
        "/api/v1/libraries/{library}/books",
        "/api/v1/libraries/{library}/series/recently-added",
        "/api/v1/libraries/{library}/books/recently-added",
        "/opds/libraries/{library}",
        "/opds/v2/libraries/{library}",
        "/komga/api/v1/libraries/{library}",
    ] {
        let visible = world.get(&world.path(template, &world.visible)).await.0;
        if visible == StatusCode::NOT_FOUND || visible.is_server_error() {
            failures.push(format!("{template}: main library answered {visible}"));
        }
        let secret = world.get(&world.path(template, &world.secret)).await.0;
        if secret != StatusCode::NOT_FOUND {
            failures.push(format!("{template}: secret library answered {secret}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[tokio::test]
async fn library_lists_leave_out_a_denied_library() {
    let world = World::new().await;
    let secret = world.secret.library_id.to_string();
    let main = world.visible.library_id.to_string();

    let (status, body) = world.get_json("/api/v1/libraries").await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<String> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![main.clone()], "v1");

    let (status, body) = world.get_json("/komga/api/v1/libraries").await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![main.as_str()], "komga");

    for feed in ["/opds/libraries", "/opds/v2/libraries"] {
        let (status, body) = world.get(feed).await;
        assert_eq!(status, StatusCode::OK, "{feed}");
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains(&main), "{feed} lists the main library");
        assert!(!text.contains(&secret), "{feed} leaks the secret library");
        assert!(
            !text.contains("Secret"),
            "{feed} leaks the secret library name"
        );
    }
}

#[tokio::test]
async fn library_counts_only_include_visible_content() {
    let world = World::new().await;
    let (_, body) = world
        .get_json(&world.path("/api/v1/libraries/{library}", &world.visible))
        .await;
    // The main library holds the visible and the hidden series
    assert_eq!(body["seriesCount"], 1, "{body}");
    assert_eq!(body["bookCount"], 1, "{body}");
}

// ============================================================================
// Lists
// ============================================================================

/// The ids in a response, wherever they appear.
fn mentions(body: &[u8], id: Uuid) -> bool {
    String::from_utf8_lossy(body).contains(&id.to_string())
}

/// Assert a list response mentions the visible item and neither hidden one.
fn assert_list(label: &str, body: &[u8], pick: impl Fn(&Item) -> Uuid, world: &World) {
    assert!(
        mentions(body, pick(&world.visible)),
        "{label}: visible item missing"
    );
    assert!(
        !mentions(body, pick(&world.hidden)),
        "{label}: hidden item listed"
    );
    assert!(
        !mentions(body, pick(&world.secret)),
        "{label}: secret item listed"
    );
}

#[tokio::test]
async fn komga_lists_leave_out_hidden_content() {
    use serde_json::json;
    let world = World::new().await;
    let series = |i: &Item| i.series_id;
    let book = |i: &Item| i.book_id;

    for uri in [
        "/komga/api/v1/series?size=100",
        "/komga/api/v1/series/new?size=100",
        "/komga/api/v1/series/updated?size=100",
    ] {
        let (status, body) = world.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_list(uri, &body, series, &world);
    }

    let (status, body) = world
        .send(
            "POST",
            "/komga/api/v1/series/list?size=100",
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_list(
        "POST series/list",
        body.to_string().as_bytes(),
        series,
        &world,
    );

    let (status, body) = world
        .send("POST", "/komga/api/v1/books/list?size=100", Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_list("POST books/list", body.to_string().as_bytes(), book, &world);
}

#[tokio::test]
async fn genre_and_tag_lists_leave_out_hidden_names() {
    let world = World::new().await;

    let (_, body) = world.get_json("/api/v1/genres?pageSize=100").await;
    let names: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Action"], "v1 genres");
    assert_eq!(body["data"][0]["seriesCount"], 1);

    let (_, body) = world.get_json("/api/v1/tags?pageSize=100").await;
    let names: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["shonen"], "v1 tags");

    let (_, body) = world.get_json("/komga/api/v1/genres").await;
    assert_eq!(body, serde_json::json!(["Action"]), "komga genres");
    let (_, body) = world.get_json("/komga/api/v1/tags").await;
    assert_eq!(body, serde_json::json!(["shonen"]), "komga tags");
}

#[tokio::test]
async fn opds_search_leaves_out_hidden_series() {
    let world = World::new().await;
    for uri in ["/opds/search?q=e", "/opds/v2/search?query=e"] {
        let (status, body) = world.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains(&world.visible.series_id.to_string()),
            "{uri}: visible missing"
        );
        assert!(
            !text.contains(&world.hidden.series_id.to_string()),
            "{uri}: hidden listed"
        );
        assert!(
            !text.contains(&world.secret.series_id.to_string()),
            "{uri}: secret listed"
        );
    }
}

#[tokio::test]
async fn books_with_errors_leave_out_hidden_books() {
    use codex::db::entities::books;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let world = World::new().await;
    let errors = r#"{"parser":{"message":"broken","occurred_at":"2026-01-01T00:00:00Z"}}"#;
    for item in [&world.visible, &world.hidden, &world.secret] {
        let book = books::Entity::find_by_id(item.book_id)
            .one(&world.state.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: books::ActiveModel = book.into();
        active.analysis_errors = Set(Some(errors.to_string()));
        active.update(&world.state.db).await.unwrap();
    }

    let (status, body) = world.get("/api/v1/books/errors").await;
    assert_eq!(status, StatusCode::OK);
    assert_list("books/errors", &body, |i| i.book_id, &world);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["totalBooksWithErrors"], 1, "{json}");
}

#[tokio::test]
async fn duplicate_groups_only_show_visible_members() {
    use codex::db::entities::{book_duplicates, series_duplicates};
    use sea_orm::{ActiveModelTrait, Set};
    let world = World::new().await;
    let now = chrono::Utc::now();

    // Visible + hidden: only one visible member left, so the group disappears
    book_duplicates::ActiveModel {
        id: Set(Uuid::new_v4()),
        file_hash: Set("h".to_string()),
        book_ids: Set(
            serde_json::to_string(&[world.visible.book_id, world.hidden.book_id]).unwrap(),
        ),
        duplicate_count: Set(2),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&world.state.db)
    .await
    .unwrap();
    series_duplicates::ActiveModel {
        id: Set(Uuid::new_v4()),
        match_type: Set("title".to_string()),
        match_key: Set("hidden".to_string()),
        library_id: Set(Some(world.visible.library_id)),
        series_ids: Set(
            serde_json::to_string(&[world.visible.series_id, world.hidden.series_id]).unwrap(),
        ),
        duplicate_count: Set(2),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&world.state.db)
    .await
    .unwrap();

    let (status, body) = world.get_json("/api/v1/duplicates").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["totalGroups"], 0, "{body}");
    let (status, body) = world.get_json("/api/v1/duplicates/series").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["totalGroups"], 0, "{body}");
}

#[tokio::test]
async fn inventory_metrics_are_refused_to_restricted_readers() {
    let world = World::new().await;
    let (status, _) = world.get("/api/v1/metrics/inventory").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Recommendations say whether the recommended series is already in Codex;
/// for a hidden series that would reveal it exists.
#[tokio::test]
async fn recommendations_do_not_reveal_hidden_series() {
    use codex::db::repositories::{
        PluginsRepository, SeriesExternalIdRepository, UserPluginDataRepository,
        UserPluginsRepository,
    };
    use serde_json::json;

    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if std::env::var("CODEX_ENCRYPTION_KEY").is_err() {
            // SAFETY: set once, before any test in this process reads it.
            unsafe {
                std::env::set_var(
                    "CODEX_ENCRYPTION_KEY",
                    "dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleTEyMzQ=",
                );
            }
        }
    });

    let world = World::new().await;
    let db = &world.state.db;
    let plugin = PluginsRepository::create(
        db,
        "recs",
        "Recs",
        None,
        "user",
        "echo",
        vec![],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "none",
        None,
        true,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    PluginsRepository::update_manifest(
        db,
        plugin.id,
        Some(json!({
            "name": "recs",
            "displayName": "Recs",
            "version": "1.0.0",
            "protocolVersion": "1.0",
            "pluginType": "user",
            "capabilities": {
                "userRecommendationProvider": true,
                "externalIdSource": "anilist"
            }
        })),
    )
    .await
    .unwrap();
    let (status, _) = world
        .send(
            "POST",
            &format!("/api/v1/user/plugins/{}/enable", plugin.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    for (item, external_id) in [
        (&world.visible, "1"),
        (&world.hidden, "2"),
        (&world.secret, "3"),
    ] {
        SeriesExternalIdRepository::create(db, item.series_id, "anilist", external_id, None, None)
            .await
            .unwrap();
    }
    let reader = UserRepository::get_by_username(db, "kid")
        .await
        .unwrap()
        .unwrap();
    let instance = UserPluginsRepository::get_by_user_and_plugin(db, reader.id, plugin.id)
        .await
        .unwrap()
        .unwrap();
    UserPluginDataRepository::set(
        db,
        instance.id,
        "recommendations",
        json!({
            "recommendations": [
                { "externalId": "1", "title": "Visible", "score": 0.9, "reason": "r" },
                { "externalId": "2", "title": "Hidden", "score": 0.9, "reason": "r" },
                { "externalId": "3", "title": "Secret", "score": 0.9, "reason": "r" }
            ],
            "generatedAt": "2026-01-01T00:00:00Z",
            "cached": true
        }),
        None,
    )
    .await
    .unwrap();

    let (status, body) = world.get_json("/api/v1/user/recommendations").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let recs = body["recommendations"].as_array().unwrap();
    let in_codex = |id: &str| {
        recs.iter()
            .find(|r| r["externalId"] == id)
            .map(|r| r["inCodex"].as_bool().unwrap_or(false))
    };
    assert_eq!(in_codex("1"), Some(true), "{body}");
    assert_eq!(in_codex("2"), Some(false), "{body}");
    assert_eq!(in_codex("3"), Some(false), "{body}");
    let text = body.to_string();
    assert!(!text.contains(&world.hidden.series_id.to_string()));
    assert!(!text.contains(&world.secret.series_id.to_string()));
}

// ============================================================================
// Event streams
// ============================================================================

#[tokio::test]
async fn entity_events_about_hidden_content_are_not_sent() {
    use codex::api::visibility::entity_event_visible;
    use codex::events::{EntityChangeEvent, EntityEvent, EntityType};
    let world = World::new().await;
    let visible_to_reader = |event: EntityEvent| {
        let event = EntityChangeEvent::new(event, None);
        let db = world.state.db.clone();
        let user = world.reader_id;
        async move { entity_event_visible(&db, user, &event).await }
    };

    for (label, item, expected) in [
        ("visible", &world.visible, true),
        ("hidden", &world.hidden, false),
        ("secret", &world.secret, false),
    ] {
        let events = [
            EntityEvent::BookUpdated {
                book_id: item.book_id,
                series_id: item.series_id,
                library_id: item.library_id,
                fields: None,
            },
            EntityEvent::SeriesUpdated {
                series_id: item.series_id,
                library_id: item.library_id,
                fields: None,
            },
            EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: item.book_id,
                library_id: Some(item.library_id),
            },
            EntityEvent::CoverUpdated {
                entity_type: EntityType::Series,
                entity_id: item.series_id,
                library_id: Some(item.library_id),
            },
        ];
        for event in events {
            let debug = format!("{event:?}");
            assert_eq!(visible_to_reader(event).await, expected, "{label}: {debug}");
        }
    }

    // A library-level event for the denied library is withheld
    assert!(
        !visible_to_reader(EntityEvent::LibraryUpdated {
            library_id: world.secret.library_id
        })
        .await
    );
    assert!(
        visible_to_reader(EntityEvent::LibraryUpdated {
            library_id: world.visible.library_id
        })
        .await
    );
}

#[tokio::test]
async fn task_events_about_hidden_content_are_not_sent() {
    use codex::api::visibility::task_event_visible;
    use codex::events::TaskProgressEvent;
    let world = World::new().await;

    for (item, expected) in [
        (&world.visible, true),
        (&world.hidden, false),
        (&world.secret, false),
    ] {
        let event = TaskProgressEvent::started(
            Uuid::new_v4(),
            "analyze_book",
            Some(item.library_id),
            Some(item.series_id),
            Some(item.book_id),
        );
        assert_eq!(
            task_event_visible(&world.state.db, world.reader_id, &event).await,
            expected
        );
    }
}

// ============================================================================
// KOReader
// ============================================================================

#[tokio::test]
async fn koreader_sync_treats_hidden_books_as_missing() {
    use codex::db::entities::books;
    use sea_orm::EntityTrait;
    let world = World::new().await;
    let hash_of = |item: &Item| {
        let db = world.state.db.clone();
        let id = item.book_id;
        async move {
            books::Entity::find_by_id(id)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .koreader_hash
                .unwrap()
        }
    };

    for (item, visible) in [
        (&world.visible, true),
        (&world.hidden, false),
        (&world.secret, false),
    ] {
        let hash = hash_of(item).await;

        let (status, body) = world
            .send(
                "PUT",
                "/koreader/syncs/progress",
                Some(serde_json::json!({
                    "document": hash,
                    "progress": "2",
                    "percentage": 0.5,
                    "device": "Kobo",
                    "device_id": "kobo-1"
                })),
            )
            .await;
        if visible {
            assert!(status.is_success(), "visible PUT: {status} {body}");
        } else {
            assert_eq!(status, StatusCode::NOT_FOUND, "hidden PUT: {body}");
        }

        let (status, body) = world
            .get_json(&format!("/koreader/syncs/progress/{hash}"))
            .await;
        if visible {
            assert!(status.is_success(), "visible GET: {status} {body}");
        } else {
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(body["message"], "No book found with this hash", "{body}");
        }
    }
}

// ============================================================================
// Visibility preview (admin tool)
// ============================================================================

#[tokio::test]
async fn admins_can_preview_what_a_reader_sees_and_why() {
    let world = World::new().await;
    let password_hash = codex::utils::password::hash_password("admin123").unwrap();
    let admin = UserRepository::create(
        &world.state.db,
        &create_test_user("boss", "boss@example.com", &password_hash, true),
    )
    .await
    .unwrap();
    let admin_token = world
        .state
        .jwt_service
        .generate_token(admin.id, admin.username.clone(), admin.get_role())
        .unwrap();
    let get = |uri: String, token: String| {
        let app = world.router();
        async move {
            let (status, body) = make_raw_request(app, get_request_with_auth(&uri, &token)).await;
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default(),
            )
        }
    };
    let preview = format!("/api/v1/users/{}/visibility-preview", world.reader_id);

    let (status, body) = get(preview.clone(), admin_token.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["restricted"], true);
    assert_eq!(body["roleBypass"], false);
    assert_eq!(body["visibleSeriesCount"], 1);
    assert_eq!(body["hiddenSeriesCount"], 2);
    let library_visible = |id: Uuid| {
        body["libraries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["id"] == id.to_string())
            .map(|l| l["visible"].clone())
    };
    assert_eq!(library_visible(world.visible.library_id), Some(true.into()));
    assert_eq!(library_visible(world.secret.library_id), Some(false.into()));

    let explain = |item: &Item| format!("{preview}?seriesId={}", item.series_id);
    let (_, body) = get(explain(&world.hidden), admin_token.clone()).await;
    let series = &body["series"];
    assert_eq!(series["visible"], false, "{body}");
    assert_eq!(series["decision"], "denied");
    assert_eq!(series["deniedBy"][0]["ruleType"], "genre");
    assert_eq!(series["deniedBy"][0]["value"], "Adult");
    assert_eq!(series["deniedBy"][0]["source"]["groupName"], "Kids");

    let (_, body) = get(explain(&world.secret), admin_token.clone()).await;
    assert_eq!(
        body["series"]["deniedBy"][0]["ruleType"], "library",
        "{body}"
    );
    assert_eq!(body["series"]["deniedBy"][0]["value"], "Secret");

    let (_, body) = get(explain(&world.visible), admin_token.clone()).await;
    assert_eq!(body["series"]["visible"], true, "{body}");
    assert_eq!(body["series"]["decision"], "open");

    // Admins are reported as exempt
    let (_, body) = get(
        format!("/api/v1/users/{}/visibility-preview", admin.id),
        admin_token,
    )
    .await;
    assert_eq!(body["roleBypass"], true, "{body}");
    assert_eq!(body["restricted"], false);

    // Readers cannot use it
    let (status, _) = get(preview, world.reader_token.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
