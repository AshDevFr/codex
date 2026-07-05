//! Cross-engine migration tests.
//!
//! The headline case: a faithful 1:1 transfer from SQLite to PostgreSQL, which
//! exercises the representation differences SeaORM papers over — UUID blobs vs
//! native `uuid`, text JSON vs JSONB, 0/1 vs `bool`, and timestamps. Gated
//! like the other PostgreSQL tests: `#[ignore]`, and skips gracefully when no
//! test database is reachable.

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::db::Database;
use codex::db::entities::{api_keys, books, libraries, read_progress, user_series_ratings, users};
use codex::migrate::{registry, transfer, verify};
use codex::models::ScanningStrategy;
use common::{setup_test_db_postgres, setup_test_db_wrapper};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;
use uuid::Uuid;

struct Ids {
    library: Uuid,
    book: Uuid,
    user: Uuid,
    progress: Uuid,
    rating: Uuid,
    api_key: Uuid,
}

/// Seed a connected fixture exercising JSON, bool, floats, ints, UUID FKs, and
/// timestamps: library → series → book, a user (JSON permissions), reading
/// progress, a series rating, and an API key (JSON permissions).
async fn seed_source(db: &Database) -> Ids {
    let library = db
        .create_library("Comics", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = db.create_series(library.id, "Saga").await.unwrap();
    let conn = db.sea_orm_connection();

    let book = Uuid::new_v4();
    books::ActiveModel {
        id: Set(book),
        series_id: Set(series.id),
        library_id: Set(library.id),
        path: Set("/lib/Saga/1.cbz".to_string()),
        file_name: Set("1.cbz".to_string()),
        file_size: Set(999),
        file_hash: Set("h".to_string()),
        partial_hash: Set("p".to_string()),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        deleted: Set(false),
        analyzed: Set(true),
        modified_at: Set(Utc::now()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await
    .unwrap();

    let user = Uuid::new_v4();
    users::ActiveModel {
        id: Set(user),
        username: Set("reader1".to_string()),
        email: Set("reader1@example.com".to_string()),
        password_hash: Set("hash".to_string()),
        role: Set("reader".to_string()),
        is_active: Set(true),
        email_verified: Set(true),
        permissions: Set(json!(["books:read"])),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        last_login_at: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();

    let progress = Uuid::new_v4();
    read_progress::ActiveModel {
        id: Set(progress),
        user_id: Set(user),
        book_id: Set(book),
        current_page: Set(42),
        progress_percentage: Set(Some(0.5)),
        completed: Set(false),
        started_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        completed_at: Set(None),
        r2_progression: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();

    let rating = Uuid::new_v4();
    user_series_ratings::ActiveModel {
        id: Set(rating),
        user_id: Set(user),
        series_id: Set(series.id),
        rating: Set(85),
        notes: Set(Some("great".to_string())),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(conn)
    .await
    .unwrap();

    let api_key = Uuid::new_v4();
    api_keys::ActiveModel {
        id: Set(api_key),
        user_id: Set(user),
        name: Set("cli".to_string()),
        key_hash: Set("kh".to_string()),
        key_prefix: Set("cdx_".to_string()),
        permissions: Set(json!(["books:read"])),
        is_active: Set(true),
        expires_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(conn)
    .await
    .unwrap();

    Ids {
        library: library.id,
        book,
        user,
        progress,
        rating,
        api_key,
    }
}

#[tokio::test]
#[ignore] // requires a PostgreSQL test database (see `make test-up`)
async fn sqlite_to_postgres_roundtrip_mirrors_all_data() {
    let (src, _src_dir) = setup_test_db_wrapper().await;
    let Some(pg) = setup_test_db_postgres().await else {
        return; // no PostgreSQL available — skip
    };
    let ids = seed_source(&src).await;

    transfer(src.sea_orm_connection(), &pg)
        .await
        .expect("SQLite -> PostgreSQL transfer should succeed");

    // Row-count parity across every table.
    let src_counts = registry::count_all(src.sea_orm_connection()).await.unwrap();
    let pg_counts = registry::count_all(&pg).await.unwrap();
    let mismatches = verify::compare(&src_counts, &pg_counts);
    assert!(mismatches.is_empty(), "count mismatches: {mismatches:?}");

    // JSON columns (text JSON on SQLite -> JSONB on Postgres) survive verbatim.
    let user = users::Entity::find_by_id(ids.user)
        .one(&pg)
        .await
        .unwrap()
        .expect("user present");
    assert_eq!(user.permissions, json!(["books:read"]));
    assert!(user.is_active);

    let key = api_keys::Entity::find_by_id(ids.api_key)
        .one(&pg)
        .await
        .unwrap()
        .expect("api key present");
    assert_eq!(key.permissions, json!(["books:read"]));

    // Reading progress: ints, a float, and a bool.
    let progress = read_progress::Entity::find_by_id(ids.progress)
        .one(&pg)
        .await
        .unwrap()
        .expect("read progress present");
    assert_eq!(progress.current_page, 42);
    assert_eq!(progress.progress_percentage, Some(0.5));
    assert!(!progress.completed);

    // Series rating value and note.
    let rating = user_series_ratings::Entity::find_by_id(ids.rating)
        .one(&pg)
        .await
        .unwrap()
        .expect("rating present");
    assert_eq!(rating.rating, 85);
    assert_eq!(rating.notes.as_deref(), Some("great"));

    // Book bool/int columns and UUID FKs resolve.
    let book = books::Entity::find_by_id(ids.book)
        .one(&pg)
        .await
        .unwrap()
        .expect("book present");
    assert_eq!(book.library_id, ids.library);
    assert_eq!(book.page_count, 10);
    assert!(book.analyzed && !book.deleted);

    // Library JSON config columns match across engines.
    let src_lib = libraries::Entity::find_by_id(ids.library)
        .one(src.sea_orm_connection())
        .await
        .unwrap()
        .unwrap();
    let pg_lib = libraries::Entity::find_by_id(ids.library)
        .one(&pg)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(src_lib.name, pg_lib.name);
    assert_eq!(src_lib.series_config, pg_lib.series_config);
}
