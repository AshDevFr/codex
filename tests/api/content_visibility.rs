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
