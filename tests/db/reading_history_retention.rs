//! Reading history outlives the books it was recorded against.
//!
//! `reading_sessions` is the only source of every reading statistic, and
//! `read_completions` is the only record that a book was ever finished. Both
//! used to cascade on `books.id`, so purging deleted books or removing a
//! library destroyed hours the user really did spend reading, with nothing left
//! to reconstruct them from.
//!
//! A session is a record of something that happened. It does not stop having
//! happened because the file was later removed, so the row survives the delete
//! and loses only its attribution.
//!
//! The user cascade is deliberately unchanged and tested here too: deleting a
//! user must still take their history with them.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex::db::ScanningStrategy;
use codex::db::entities::reading_sessions::SessionKind;
use codex::db::entities::{books, read_completions, reading_sessions, users};
use codex::db::repositories::{
    BookRepository, LibraryRepository, NewSession, ReadCompletionRepository,
    ReadProgressRepository, SeriesRepository, UserRepository,
};
use common::*;
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    Statement,
};
use tempfile::TempDir;
use uuid::Uuid;

fn unique() -> String {
    Uuid::new_v4().to_string()
}

async fn persist_user(db: &DatabaseConnection) -> Uuid {
    let handle = format!("reader-{}", unique());
    let model = create_test_user(&handle, &format!("{handle}@test.test"), "hash", true);
    UserRepository::create(db, &model).await.unwrap().id
}

async fn persist_book(db: &DatabaseConnection) -> Uuid {
    let library = LibraryRepository::create(
        db,
        "Lib",
        &format!("/lib/{}", unique()),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    let book = create_test_book(
        series.id,
        library.id,
        &format!("/lib/{}.cbz", unique()),
        "book",
        &format!("hash_{}", unique()),
        "cbz",
        200,
    );
    BookRepository::create(db, &book, None).await.unwrap().id
}

/// One measured sitting plus one banked completion, which is the pair a
/// finished read leaves behind.
async fn record_history(db: &DatabaseConnection, user: Uuid, book: Uuid) {
    let now = Utc::now();
    let session = NewSession::from_client(
        Uuid::new_v4(),
        user,
        book,
        "device-1",
        Some("Codex Web".to_string()),
        SessionKind::Progress,
        Some(900_000),
        Some(24),
        now - Duration::minutes(20),
        now - Duration::minutes(5),
    )
    .with_page(24);

    ReadProgressRepository::record_session(db, session)
        .await
        .unwrap();

    ReadCompletionRepository::record(db, user, book, now - Duration::minutes(20), now)
        .await
        .unwrap();
}

async fn sessions_for_user(db: &DatabaseConnection, user: Uuid) -> Vec<reading_sessions::Model> {
    reading_sessions::Entity::find()
        .filter(reading_sessions::Column::UserId.eq(user))
        .all(db)
        .await
        .unwrap()
}

async fn completions_for_user(db: &DatabaseConnection, user: Uuid) -> Vec<read_completions::Model> {
    read_completions::Entity::find()
        .filter(read_completions::Column::UserId.eq(user))
        .all(db)
        .await
        .unwrap()
}

/// Hard-deleting a book keeps the history and nulls only the attribution.
///
/// This is the whole point: `purge_deleted_in_library` and library deletion both
/// reach a real `DELETE`, and either one used to erase the statistics silently.
async fn exercise_history_survives_book_delete(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;
    record_history(db, user, book).await;

    assert_eq!(
        sessions_for_user(db, user).await.len(),
        1,
        "fixture should have recorded one session"
    );
    assert_eq!(
        completions_for_user(db, user).await.len(),
        1,
        "fixture should have recorded one completion"
    );

    books::Entity::delete_by_id(book).exec(db).await.unwrap();

    let sessions = sessions_for_user(db, user).await;
    assert_eq!(
        sessions.len(),
        1,
        "the session must outlive the book it was recorded against"
    );
    assert!(
        sessions[0].book_id.is_none(),
        "the surviving session must lose its attribution, not keep a dangling book id"
    );

    let completions = completions_for_user(db, user).await;
    assert_eq!(
        completions.len(),
        1,
        "the completion must outlive the book it was recorded against"
    );
    assert!(
        completions[0].book_id.is_none(),
        "the surviving completion must lose its attribution, not keep a dangling book id"
    );
}

/// Deleting the user still takes their history. That cascade is unchanged, and
/// a migration that relaxed it by accident would be a privacy defect.
async fn exercise_user_delete_still_removes_history(db: &DatabaseConnection) {
    let user = persist_user(db).await;
    let book = persist_book(db).await;
    record_history(db, user, book).await;

    users::Entity::delete_by_id(user).exec(db).await.unwrap();

    assert!(
        sessions_for_user(db, user).await.is_empty(),
        "deleting a user must still remove their sessions"
    );
    assert!(
        completions_for_user(db, user).await.is_empty(),
        "deleting a user must still remove their completions"
    );
}

#[tokio::test]
async fn history_survives_book_delete_sqlite() {
    let (db, _temp_dir) = setup_test_db().await;
    exercise_history_survives_book_delete(&db).await;
}

#[tokio::test]
async fn user_delete_still_removes_history_sqlite() {
    let (db, _temp_dir) = setup_test_db().await;
    exercise_user_delete_still_removes_history(&db).await;
}

/// Both of the above against PostgreSQL, sequenced in one test on purpose:
/// `setup_test_db_postgres` truncates a database shared by the whole run, so two
/// PostgreSQL tests running at once delete each other's fixtures.
///
/// This one matters more than most: the SQLite path rebuilds the table to change
/// the constraint while PostgreSQL alters it in place, so the two engines reach
/// the same schema by different routes and only this test says they agree.
#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn reading_history_retention_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };

    exercise_history_survives_book_delete(&db).await;
    exercise_user_delete_still_removes_history(&db).await;
}

// ============================================================================
// The SQLite table rebuild
// ============================================================================

const HISTORY_TABLES: [&str; 2] = ["reading_sessions", "read_completions"];

/// A SQLite database migrated up to, but not including, the migration that
/// relaxes the cascade: the schema exactly as the original create migrations
/// left it.
async fn sqlite_before_retention_migration() -> (Database, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        }),
        ..DatabaseConfig::default()
    };
    let db = Database::new(&config).await.unwrap();

    let migrations = Migrator::migrations();
    let target = migrations
        .iter()
        .position(|m| m.name().contains("reading_history_survives_book_delete"))
        .expect("the retention migration should be registered");
    Migrator::up(db.sea_orm_connection(), Some(target as u32))
        .await
        .unwrap();
    (db, temp_dir)
}

async fn apply_retention_migration(db: &DatabaseConnection) {
    Migrator::up(db, Some(1)).await.unwrap();
}

async fn strings(db: &DatabaseConnection, sql: &str) -> Vec<String> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.try_get_by_index::<String>(0).unwrap())
    .collect()
}

/// Every index on the table, as the SQL that created it. Comparing the SQL
/// catches a dropped index, a lost column and a lost `DESC` alike.
async fn index_definitions(db: &DatabaseConnection, table: &str) -> Vec<String> {
    strings(
        db,
        &format!(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = '{table}' \
             AND sql IS NOT NULL ORDER BY name"
        ),
    )
    .await
}

/// `name type notnull default pk` per column, in column order.
async fn column_shapes(db: &DatabaseConnection, table: &str) -> Vec<String> {
    strings(
        db,
        &format!(
            "SELECT name || ' ' || type || ' ' || \"notnull\" || ' ' || \
             COALESCE(dflt_value, '-') || ' ' || pk FROM pragma_table_info('{table}') ORDER BY cid"
        ),
    )
    .await
}

/// `column -> table on_delete` per foreign key.
async fn foreign_keys(db: &DatabaseConnection, table: &str) -> Vec<String> {
    strings(
        db,
        &format!(
            "SELECT \"from\" || ' -> ' || \"table\" || ' ' || on_delete \
             FROM pragma_foreign_key_list('{table}') ORDER BY \"from\""
        ),
    )
    .await
}

/// The rebuild keeps every row and attribution, reproduces the original
/// indexes and columns exactly, and changes only `book_id`'s nullability and
/// delete action.
#[tokio::test]
async fn sqlite_rebuild_keeps_rows_and_indexes() {
    let (db, _temp_dir) = sqlite_before_retention_migration().await;
    let conn = db.sea_orm_connection();

    let user = persist_user(conn).await;
    let mut books = Vec::new();
    for _ in 0..3 {
        let book = persist_book(conn).await;
        record_history(conn, user, book).await;
        books.push(book);
    }

    let mut indexes_before = Vec::new();
    let mut columns_before = Vec::new();
    for table in HISTORY_TABLES {
        indexes_before.push(index_definitions(conn, table).await);
        columns_before.push(column_shapes(conn, table).await);
    }
    assert_eq!(
        indexes_before[0].len(),
        3,
        "reading_sessions starts with 3 indexes"
    );
    assert_eq!(
        indexes_before[1].len(),
        2,
        "read_completions starts with 2 indexes"
    );

    apply_retention_migration(conn).await;

    for (i, table) in HISTORY_TABLES.into_iter().enumerate() {
        assert_eq!(
            index_definitions(conn, table).await,
            indexes_before[i],
            "{table}: the rebuild must recreate every index exactly"
        );

        let expected_columns: Vec<String> = columns_before[i]
            .iter()
            .map(|shape| {
                if shape.starts_with("book_id ") {
                    shape.replacen(" 1 ", " 0 ", 1)
                } else {
                    shape.clone()
                }
            })
            .collect();
        assert_eq!(
            column_shapes(conn, table).await,
            expected_columns,
            "{table}: only book_id's nullability may change"
        );

        assert_eq!(
            foreign_keys(conn, table).await,
            vec![
                "book_id -> books SET NULL".to_string(),
                "user_id -> users CASCADE".to_string(),
            ],
            "{table}: book deletes null the column, user deletes still cascade"
        );
    }

    let sessions = sessions_for_user(conn, user).await;
    let completions = completions_for_user(conn, user).await;
    assert_eq!(sessions.len(), 3, "every session survives the rebuild");
    assert_eq!(
        completions.len(),
        3,
        "every completion survives the rebuild"
    );
    for book in &books {
        assert!(sessions.iter().any(|s| s.book_id == Some(*book)));
        assert!(completions.iter().any(|c| c.book_id == Some(*book)));
    }

    // Enforcement is on for application connections, so this proves the new
    // constraint is live and not merely declared.
    books::Entity::delete_by_id(books[0])
        .exec(conn)
        .await
        .unwrap();
    let sessions = sessions_for_user(conn, user).await;
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions.iter().filter(|s| s.book_id.is_none()).count(), 1);

    db.close().await;
}

/// Rolling back restores the original constraint, which orphaned rows cannot
/// satisfy, so it discards exactly those and keeps everything attributed.
#[tokio::test]
async fn sqlite_rollback_drops_only_orphans() {
    let (db, _temp_dir) = setup_test_db_wrapper().await;
    let conn = db.sea_orm_connection();

    let user = persist_user(conn).await;
    let kept = persist_book(conn).await;
    let removed = persist_book(conn).await;
    record_history(conn, user, kept).await;
    record_history(conn, user, removed).await;
    books::Entity::delete_by_id(removed)
        .exec(conn)
        .await
        .unwrap();

    Migrator::down(conn, Some(1)).await.unwrap();

    for table in HISTORY_TABLES {
        assert!(
            column_shapes(conn, table)
                .await
                .iter()
                .any(|shape| shape.starts_with("book_id ") && shape.contains(" 1 ")),
            "{table}: book_id is required again"
        );
        assert!(
            foreign_keys(conn, table)
                .await
                .contains(&"book_id -> books CASCADE".to_string()),
            "{table}: book deletes cascade again"
        );
    }

    let sessions = sessions_for_user(conn, user).await;
    let completions = completions_for_user(conn, user).await;
    assert_eq!(sessions.len(), 1, "only the orphaned session is discarded");
    assert_eq!(sessions[0].book_id, Some(kept));
    assert_eq!(
        completions.len(),
        1,
        "only the orphaned completion is discarded"
    );
    assert_eq!(completions[0].book_id, Some(kept));

    // And forward again, so the migration is re-runnable.
    Migrator::up(conn, None).await.unwrap();
    assert_eq!(sessions_for_user(conn, user).await.len(), 1);

    db.close().await;
}
