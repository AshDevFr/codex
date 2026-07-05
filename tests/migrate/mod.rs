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

    transfer(
        src.sea_orm_connection(),
        &pg,
        codex::migrate::Progress::Silent,
    )
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

// ---------------------------------------------------------------------------
// Export/import across every engine pair.
// ---------------------------------------------------------------------------

use codex::db::entities::genres;
use codex::migrate::archive::{export_archive, import_archive};
use codex::migrate::database_config_from_url;
use common::setup_test_db;
use sea_orm::{ConnectionTrait, DatabaseConnection};

/// Seed an engine-neutral fixture on any connection: one genre (UUID + text +
/// timestamp) and one user (UUID + JSON permissions + bool). No FKs, so it
/// inserts on SQLite or Postgres identically.
async fn seed_min(conn: &DatabaseConnection) {
    let gid = Uuid::new_v4();
    genres::ActiveModel {
        id: Set(gid),
        name: Set("Action".to_string()),
        normalized_name: Set("action".to_string()),
        created_at: Set(Utc::now()),
    }
    .insert(conn)
    .await
    .unwrap();

    let uid = Uuid::new_v4();
    users::ActiveModel {
        id: Set(uid),
        username: Set(format!("u-{uid}")),
        email: Set(format!("{uid}@example.com")),
        password_hash: Set("h".to_string()),
        role: Set("reader".to_string()),
        is_active: Set(true),
        email_verified: Set(false),
        permissions: Set(json!(["books:read"])),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        last_login_at: Set(None),
    }
    .insert(conn)
    .await
    .unwrap();
}

/// Create a fresh, migrated PostgreSQL database named `name` on the test
/// server. Returns `None` if PostgreSQL is unreachable.
async fn fresh_pg(name: &str) -> Option<DatabaseConnection> {
    let base = std::env::var("POSTGRES_TEST_URL")
        .unwrap_or_else(|_| "postgres://codex:codex@localhost:54321/codex_test".to_string());

    // Connect to the default test DB to issue CREATE DATABASE.
    let admin = Database::new(&database_config_from_url(&base).ok()?)
        .await
        .ok()?;
    let ac = admin.sea_orm_connection();
    ac.execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
        .await
        .ok()?;
    ac.execute_unprepared(&format!("CREATE DATABASE {name}"))
        .await
        .ok()?;

    let mut cfg = database_config_from_url(&base).ok()?;
    cfg.postgres.as_mut().unwrap().database_name = name.to_string();
    let db = Database::new(&cfg).await.ok()?;
    db.run_migrations().await.ok()?;
    Some(db.sea_orm_connection().clone())
}

async fn drop_pg(name: &str) {
    let base = std::env::var("POSTGRES_TEST_URL")
        .unwrap_or_else(|_| "postgres://codex:codex@localhost:54321/codex_test".to_string());
    if let Ok(cfg) = database_config_from_url(&base)
        && let Ok(admin) = Database::new(&cfg).await
    {
        let _ = admin
            .sea_orm_connection()
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
            .await;
    }
}

/// Export `src` to an archive and import it into `tgt`, then assert the target
/// mirrors the source (row-count parity) and the JSON permissions survived.
async fn export_import_pair(src: &DatabaseConnection, tgt: &DatabaseConnection, label: &str) {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("export.tar.gz");

    export_archive(src, &archive, &[], codex::migrate::Progress::Silent)
        .await
        .unwrap_or_else(|e| panic!("{label}: export failed: {e:#}"));
    import_archive(tgt, &archive, &[], codex::migrate::Progress::Silent)
        .await
        .unwrap_or_else(|e| panic!("{label}: import failed: {e:#}"));

    let src_counts = registry::count_all(src).await.unwrap();
    let tgt_counts = registry::count_all(tgt).await.unwrap();
    let mismatches = verify::compare(&src_counts, &tgt_counts);
    assert!(
        mismatches.is_empty(),
        "{label}: count mismatch: {mismatches:?}"
    );

    // JSON permissions (text JSON <-> JSONB) survived across the pair.
    let user = users::Entity::find()
        .one(tgt)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("{label}: seeded user missing after import"));
    assert_eq!(
        user.permissions,
        json!(["books:read"]),
        "{label}: permissions"
    );
}

#[tokio::test]
#[ignore] // Postgres pairs require a test database (see `make test-up`)
async fn export_import_across_all_engine_pairs() {
    // --- SQLite -> SQLite (always runs). ---
    {
        let (src, _sd) = setup_test_db().await;
        let (tgt, _td) = setup_test_db().await;
        seed_min(&src).await;
        export_import_pair(&src, &tgt, "sqlite->sqlite").await;
    }

    // --- Pairs involving Postgres (skip when unavailable). ---
    let Some(pg_a) = fresh_pg("codex_test_pair_a").await else {
        eprintln!("PostgreSQL unavailable; ran sqlite->sqlite only");
        return;
    };
    let pg_b = fresh_pg("codex_test_pair_b")
        .await
        .expect("second Postgres database");
    seed_min(&pg_a).await;

    // SQLite -> Postgres
    {
        let (src, _sd) = setup_test_db().await;
        seed_min(&src).await;
        export_import_pair(&src, &pg_b, "sqlite->postgres").await;
    }
    // Postgres -> SQLite
    {
        let (tgt, _td) = setup_test_db().await;
        export_import_pair(&pg_a, &tgt, "postgres->sqlite").await;
    }
    // Postgres -> Postgres
    export_import_pair(&pg_a, &pg_b, "postgres->postgres").await;

    drop(pg_a);
    drop(pg_b);
    drop_pg("codex_test_pair_a").await;
    drop_pg("codex_test_pair_b").await;
}

// ---------------------------------------------------------------------------
// Wide-table batch: a table with many columns must not exceed the destination's
// bind-parameter limit (PostgreSQL 65535 / SQLite 32766).
// ---------------------------------------------------------------------------

use codex::db::entities::book_metadata;
use sea_orm::PaginatorTrait;

async fn seed_wide_rows(db: &Database, n: usize) {
    let library = db
        .create_library("Comics", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = db.create_series(library.id, "Saga").await.unwrap();
    let conn = db.sea_orm_connection();

    let mut books_am = Vec::with_capacity(n);
    let mut meta_am = Vec::with_capacity(n);
    for i in 0..n {
        let book_id = Uuid::new_v4();
        books_am.push(books::ActiveModel {
            id: Set(book_id),
            series_id: Set(series.id),
            library_id: Set(library.id),
            path: Set(format!("/lib/{i}.cbz")),
            file_name: Set(format!("{i}.cbz")),
            file_size: Set(1),
            file_hash: Set(format!("h{i}")),
            partial_hash: Set(format!("p{i}")),
            format: Set("cbz".to_string()),
            page_count: Set(1),
            deleted: Set(false),
            analyzed: Set(true),
            modified_at: Set(Utc::now()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        });
        meta_am.push(book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            search_title: Set(format!("title {i}")),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        });
    }
    // Seed in modest chunks so seeding itself stays under the SQLite limit.
    for chunk in books_am.chunks(400) {
        books::Entity::insert_many(chunk.to_vec())
            .exec(conn)
            .await
            .unwrap();
    }
    for chunk in meta_am.chunks(400) {
        book_metadata::Entity::insert_many(chunk.to_vec())
            .exec(conn)
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore] // requires a PostgreSQL test database
async fn copy_wide_table_exceeding_param_limit() {
    let (src, _sd) = setup_test_db_wrapper().await;
    let Some(pg) = fresh_pg("codex_test_wide").await else {
        return; // no PostgreSQL available
    };

    // 1000 book_metadata rows × 66 columns = 66000 bind params in one naive
    // batch — over PostgreSQL's 65535 limit. The batch cap must split it.
    seed_wide_rows(&src, 1000).await;

    transfer(
        src.sea_orm_connection(),
        &pg,
        codex::migrate::Progress::Silent,
    )
    .await
    .expect("wide-table copy must not exceed the bind-parameter limit");

    let n = book_metadata::Entity::find().count(&pg).await.unwrap();
    assert_eq!(n, 1000, "all wide rows copied");

    drop(pg);
    drop_pg("codex_test_wide").await;
}

// ---------------------------------------------------------------------------
// The FK suppression must work as an ordinary (non-superuser) database owner,
// which is what managed Postgres gives you. Before the drop/recreate approach
// this failed with "permission denied to set parameter session_replication_role".
// ---------------------------------------------------------------------------

fn pg_base_url() -> String {
    std::env::var("POSTGRES_TEST_URL")
        .unwrap_or_else(|_| "postgres://codex:codex@localhost:54321/codex_test".to_string())
}

#[tokio::test]
#[ignore] // requires a PostgreSQL test database with a superuser admin
async fn copy_works_as_non_superuser_database_owner() {
    let base = pg_base_url();
    let Ok(admin) = Database::new(&database_config_from_url(&base).unwrap()).await else {
        return; // no PostgreSQL available
    };
    let ac = admin.sea_orm_connection();

    // Clean slate, then a NOSUPERUSER role that owns a fresh database.
    ac.execute_unprepared("DROP DATABASE IF EXISTS codex_test_nosuper WITH (FORCE)")
        .await
        .ok();
    ac.execute_unprepared("DROP ROLE IF EXISTS codex_nosuper")
        .await
        .ok();
    ac.execute_unprepared("CREATE ROLE codex_nosuper LOGIN PASSWORD 'nosuperpw' NOSUPERUSER")
        .await
        .unwrap();
    ac.execute_unprepared("CREATE DATABASE codex_test_nosuper OWNER codex_nosuper")
        .await
        .unwrap();

    // As admin, hand the schema to the role so it can create tables (PG-version safe).
    let mut admin_newdb = database_config_from_url(&base).unwrap();
    admin_newdb.postgres.as_mut().unwrap().database_name = "codex_test_nosuper".to_string();
    let admin2 = Database::new(&admin_newdb).await.unwrap();
    admin2
        .sea_orm_connection()
        .execute_unprepared("ALTER SCHEMA public OWNER TO codex_nosuper")
        .await
        .ok();

    // Connect AS the non-superuser owner and run migrations + transfer.
    let mut cfg = database_config_from_url(&base).unwrap();
    {
        let pg = cfg.postgres.as_mut().unwrap();
        pg.username = "codex_nosuper".to_string();
        pg.password = "nosuperpw".to_string();
        pg.database_name = "codex_test_nosuper".to_string();
    }
    let target = Database::new(&cfg).await.unwrap();
    target.run_migrations().await.unwrap();

    let (src, _sd) = setup_test_db_wrapper().await;
    seed_min(src.sea_orm_connection()).await;

    // The crux: this must succeed without superuser.
    transfer(
        src.sea_orm_connection(),
        target.sea_orm_connection(),
        codex::migrate::Progress::Silent,
    )
    .await
    .expect("copy must work as a non-superuser database owner");

    let n = users::Entity::find()
        .count(target.sea_orm_connection())
        .await
        .unwrap();
    assert_eq!(n, 1);

    // Cleanup.
    drop(target);
    drop(admin2);
    ac.execute_unprepared("DROP DATABASE IF EXISTS codex_test_nosuper WITH (FORCE)")
        .await
        .ok();
    ac.execute_unprepared("DROP ROLE IF EXISTS codex_nosuper")
        .await
        .ok();
}
