//! Engine-level round-trip: populate a SQLite source with an FK-connected
//! dataset, transfer it into a fresh SQLite target, and assert the target is a
//! byte-identical mirror. Also guards the entity registry against schema drift.

use codex_db::entities::{genres, series_genres};
use codex_db::test_helpers::create_test_db;
use codex_migrate::{registry, verify};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, Statement};
use uuid::Uuid;

/// Seed a small but FK-connected dataset: library → series, plus a genre and a
/// series↔genre junction row. Exercises UUID FKs, a JSON config column
/// (libraries), and a two-FK junction table. Returns the ids for later checks.
async fn seed_source(db: &codex_db::Database) -> (Uuid, Uuid, Uuid) {
    let library = db
        .create_library(
            "Comics",
            "/library/comics",
            codex_db::ScanningStrategy::Default,
        )
        .await
        .unwrap();
    let series = db.create_series(library.id, "Saga").await.unwrap();

    let conn = db.sea_orm_connection();
    let genre_id = Uuid::new_v4();
    genres::ActiveModel {
        id: Set(genre_id),
        name: Set("Action".to_string()),
        normalized_name: Set("action".to_string()),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(conn)
    .await
    .unwrap();

    series_genres::ActiveModel {
        series_id: Set(series.id),
        genre_id: Set(genre_id),
    }
    .insert(conn)
    .await
    .unwrap();

    (library.id, series.id, genre_id)
}

#[tokio::test]
async fn transfer_mirrors_source_into_empty_target() {
    let (src, _src_dir) = create_test_db().await;
    let (dst, _dst_dir) = create_test_db().await;

    let (library_id, series_id, genre_id) = seed_source(&src).await;

    let report = codex_migrate::transfer(
        src.sea_orm_connection(),
        dst.sea_orm_connection(),
        codex_migrate::Progress::Silent,
    )
    .await
    .expect("transfer should succeed");

    // Every non-empty table copied at least what we seeded.
    assert!(
        report.total_rows >= 4,
        "expected >=4 rows, got {}",
        report.total_rows
    );

    // Row-count parity across every table: no drops, no duplicates.
    let src_counts = registry::count_all(src.sea_orm_connection()).await.unwrap();
    let dst_counts = registry::count_all(dst.sea_orm_connection()).await.unwrap();
    let mismatches = verify::compare(&src_counts, &dst_counts);
    assert!(mismatches.is_empty(), "count mismatches: {mismatches:?}");

    // Spot-check that specific rows, UUID FKs, and the JSON column survived.
    let dst_conn = dst.sea_orm_connection();

    let library = codex_db::entities::libraries::Entity::find_by_id(library_id)
        .one(dst_conn)
        .await
        .unwrap()
        .expect("library present in target");
    assert_eq!(library.name, "Comics");

    let series = codex_db::entities::series::Entity::find_by_id(series_id)
        .one(dst_conn)
        .await
        .unwrap()
        .expect("series present in target");
    assert_eq!(series.name, "Saga");
    assert_eq!(series.library_id, library_id, "UUID FK preserved");

    let genre = genres::Entity::find_by_id(genre_id)
        .one(dst_conn)
        .await
        .unwrap()
        .expect("genre present in target");
    assert_eq!(genre.normalized_name, "action");

    // Junction row transferred with both FKs intact.
    let link = series_genres::Entity::find_by_id((series_id, genre_id))
        .one(dst_conn)
        .await
        .unwrap();
    assert!(link.is_some(), "series↔genre junction row preserved");
}

#[tokio::test]
async fn transfer_preserves_library_json_config_exactly() {
    let (src, _src_dir) = create_test_db().await;
    let (dst, _dst_dir) = create_test_db().await;
    let (library_id, _, _) = seed_source(&src).await;

    codex_migrate::transfer(
        src.sea_orm_connection(),
        dst.sea_orm_connection(),
        codex_migrate::Progress::Silent,
    )
    .await
    .unwrap();

    // Full-model equality confirms JSON config columns round-trip verbatim.
    let src_lib = codex_db::entities::libraries::Entity::find_by_id(library_id)
        .one(src.sea_orm_connection())
        .await
        .unwrap()
        .unwrap();
    let dst_lib = codex_db::entities::libraries::Entity::find_by_id(library_id)
        .one(dst.sea_orm_connection())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(src_lib, dst_lib);
}

#[tokio::test]
async fn transfer_overwrites_existing_target_data() {
    let (src, _src_dir) = create_test_db().await;
    let (dst, _dst_dir) = create_test_db().await;

    // Source has one library; target starts with unrelated content.
    seed_source(&src).await;
    let (dst_only_lib, _, _) = seed_source(&dst).await;

    // First transfer replaces the target's content with the source's.
    codex_migrate::transfer(
        src.sea_orm_connection(),
        dst.sea_orm_connection(),
        codex_migrate::Progress::Silent,
    )
    .await
    .unwrap();

    // The target's original library is gone; the target now mirrors the source.
    let stale = codex_db::entities::libraries::Entity::find_by_id(dst_only_lib)
        .one(dst.sea_orm_connection())
        .await
        .unwrap();
    assert!(stale.is_none(), "pre-existing target row should be cleared");

    let src_counts = registry::count_all(src.sea_orm_connection()).await.unwrap();
    let after_first = registry::count_all(dst.sea_orm_connection()).await.unwrap();
    assert!(verify::compare(&src_counts, &after_first).is_empty());

    // A second transfer is idempotent (truncate → reload leaves the mirror).
    codex_migrate::transfer(
        src.sea_orm_connection(),
        dst.sea_orm_connection(),
        codex_migrate::Progress::Silent,
    )
    .await
    .unwrap();
    let after_second = registry::count_all(dst.sea_orm_connection()).await.unwrap();
    assert!(verify::compare(&src_counts, &after_second).is_empty());
}

/// The registry must list exactly the tables the migrations create (minus
/// SeaORM's own bookkeeping table). Fails loudly if a new table is added to
/// the schema but not to the registry — or vice versa.
#[tokio::test]
async fn registry_covers_every_migration_table() {
    let (db, _dir) = create_test_db().await;
    let conn = db.sea_orm_connection();

    let rows = conn
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' \
             AND name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations'"
                .to_string(),
        ))
        .await
        .unwrap();

    let mut schema_tables: Vec<String> = rows
        .iter()
        .map(|r| r.try_get::<String>("", "name").unwrap())
        .collect();
    schema_tables.sort();

    let mut registry_tables = registry::table_names();
    registry_tables.sort();

    let missing_from_registry: Vec<_> = schema_tables
        .iter()
        .filter(|t| !registry_tables.contains(t))
        .collect();
    let unknown_in_registry: Vec<_> = registry_tables
        .iter()
        .filter(|t| !schema_tables.contains(t))
        .collect();

    assert!(
        missing_from_registry.is_empty() && unknown_in_registry.is_empty(),
        "registry drift.\n  tables missing from registry: {missing_from_registry:?}\n  registry tables not in schema: {unknown_in_registry:?}"
    );
}

#[tokio::test]
async fn transfer_reads_text_stored_uuids_from_sqlite() {
    // Some databases written by older toolchains store UUIDs as 36-char
    // hyphenated text rather than 16-byte blobs. The reader must handle both.
    let (src, _s) = create_test_db().await;
    let (dst, _d) = create_test_db().await;
    let sconn = src.sea_orm_connection();

    let id = "550e8400-e29b-41d4-a716-446655440000";
    sconn
        .execute_unprepared(&format!(
            "INSERT INTO genres (id, name, normalized_name, created_at) \
             VALUES ('{id}','Action','action','2020-01-01 00:00:00+00:00')"
        ))
        .await
        .unwrap();

    // Sanity: the id really is stored as text, not a blob.
    let row = sconn
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT typeof(id) AS t FROM genres".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<String>("", "t").unwrap(), "text");

    codex_migrate::transfer(
        sconn,
        dst.sea_orm_connection(),
        codex_migrate::Progress::Silent,
    )
    .await
    .expect("transfer must read text-stored UUIDs");

    let uuid = Uuid::parse_str(id).unwrap();
    let genre = genres::Entity::find_by_id(uuid)
        .one(dst.sea_orm_connection())
        .await
        .unwrap();
    assert!(
        genre.is_some(),
        "genre with a text-stored UUID should transfer"
    );
    assert_eq!(genre.unwrap().name, "Action");
}
