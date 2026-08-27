//! Test database helpers.
//!
//! Gated behind the `test-utils` feature so downstream crates can opt in via
//! a dev-dependency feature flag (`codex-db = { ..., features = ["test-utils"] }`)
//! without dragging the helpers into release builds.

use crate::Database;
use codex_config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use tempfile::TempDir;

/// Helper to create a test SQLite database with migrations applied
///
/// This function creates a temporary SQLite database, runs all migrations,
/// and returns both the database connection and the temp directory (to keep it alive).
pub async fn create_test_db() -> (Database, TempDir) {
    use std::collections::HashMap;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Enable foreign keys for SQLite (required for foreign key constraints)
    let mut pragmas = HashMap::new();
    pragmas.insert("foreign_keys".to_string(), "ON".to_string());

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: Some(pragmas),
            ..SQLiteConfig::default()
        }),
        ..DatabaseConfig::default()
    };

    let db = Database::new(&config).await.unwrap();
    db.run_migrations().await.unwrap();
    (db, temp_dir)
}

/// Simplified helper that returns the `DatabaseConnection` and keeps the temp dir alive.
pub async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let (db, temp_dir) = create_test_db().await;
    let conn = db.sea_orm_connection().clone();
    // Leak the temp_dir so it stays alive for the duration of the test
    // This is acceptable in test code
    std::mem::forget(temp_dir);
    conn
}

// ---------------------------------------------------------------------------
// Pagination tie fixtures
// ---------------------------------------------------------------------------
//
// An `ORDER BY` on a non-unique column feeding `OFFSET`/`LIMIT` has no defined
// total order, so the rows that tie may come back in any order and two runs of
// the same query need not agree. Testing that requires rows whose sort keys are
// *byte-identical*, which is not what a loop of `Utc::now()` produces.
//
// These fixtures build that state deliberately, and they arrange ids so the
// intended order is distinguishable from the order an untied query happens to
// return. See `assert_exact_order` for why that matters.

use crate::entities::{books, libraries, read_progress, series, users};
use crate::repositories::{BookRepository, LibraryRepository, SeriesRepository, UserRepository};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

/// Books that all carry the same `created_at`/`updated_at`.
///
/// Mirrors what a library scan produces: `process_series_batched` captures one
/// `Utc::now()` before its chunk loop and stamps every book it creates with it,
/// so a scan batch is one large tie group on the column `recently-added` sorts
/// by.
pub struct TiedBooks {
    /// The single timestamp every book in the batch shares.
    pub timestamp: DateTime<Utc>,
    /// The books, in the order they were inserted.
    pub inserted: Vec<books::Model>,
}

/// Series that all have the same number of books.
///
/// Book count is an aggregate over a small range, so a sort by it produces far
/// wider tie groups than any timestamp does: a library of single-volume series
/// is one tie group.
pub struct TiedSeries {
    /// How many books each series holds.
    pub books_each: usize,
    /// The series, in the order they were inserted.
    pub inserted: Vec<series::Model>,
}

/// Read-progress rows that all carry the same `updated_at`.
pub struct TiedProgress {
    /// The single timestamp every row shares.
    pub timestamp: DateTime<Utc>,
    /// The rows, in the order they were inserted.
    pub inserted: Vec<read_progress::Model>,
}

impl TiedBooks {
    /// Ids ascending: what a `books.id ASC` tiebreaker must produce.
    pub fn ids_ascending(&self) -> Vec<Uuid> {
        sorted_ids(self.inserted.iter().map(|b| b.id))
    }

    /// Ids descending.
    pub fn ids_descending(&self) -> Vec<Uuid> {
        let mut ids = self.ids_ascending();
        ids.reverse();
        ids
    }

    /// Insertion order, which is what SQLite returns for an untied query.
    pub fn ids_as_inserted(&self) -> Vec<Uuid> {
        self.inserted.iter().map(|b| b.id).collect()
    }
}

impl TiedSeries {
    /// Ids ascending: what a `series.id ASC` tiebreaker must produce.
    pub fn ids_ascending(&self) -> Vec<Uuid> {
        sorted_ids(self.inserted.iter().map(|s| s.id))
    }

    /// Insertion order, which is what SQLite returns for an untied query.
    pub fn ids_as_inserted(&self) -> Vec<Uuid> {
        self.inserted.iter().map(|s| s.id).collect()
    }
}

impl TiedProgress {
    /// The book ids, ascending. The lists under test return books, not progress
    /// rows, so this is the sequence a `books.id ASC` tiebreaker must produce.
    pub fn book_ids_ascending(&self) -> Vec<Uuid> {
        sorted_ids(self.inserted.iter().map(|p| p.book_id))
    }

    /// Book ids in insertion order.
    pub fn book_ids_as_inserted(&self) -> Vec<Uuid> {
        self.inserted.iter().map(|p| p.book_id).collect()
    }
}

fn sorted_ids(ids: impl Iterator<Item = Uuid>) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = ids.collect();
    ids.sort();
    ids
}

/// Create a library to hang fixtures off.
pub async fn seed_library(db: &DatabaseConnection, name: &str) -> libraries::Model {
    LibraryRepository::create(
        db,
        name,
        &format!("/fixtures/{name}"),
        crate::ScanningStrategy::Default,
    )
    .await
    .expect("failed to seed library")
}

/// Create a series (and its metadata row) to hang fixtures off.
pub async fn seed_series(db: &DatabaseConnection, library_id: Uuid, name: &str) -> series::Model {
    SeriesRepository::create(db, library_id, name, None)
        .await
        .expect("failed to seed series")
}

/// Create a user to attribute read progress to.
pub async fn seed_user(db: &DatabaseConnection, username: &str) -> users::Model {
    let user = users::Model {
        id: Uuid::new_v4(),
        username: username.to_string(),
        email: format!("{username}@example.test"),
        password_hash: codex_utils::password::hash_password("password").unwrap(),
        role: "admin".to_string(),
        is_active: true,
        email_verified: false,
        permissions: serde_json::json!([]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };
    UserRepository::create(db, &user)
        .await
        .expect("failed to seed user")
}

/// Insert `count` books that all share one `created_at` and `updated_at`.
///
/// The books are inserted in **descending** id order, so insertion order is the
/// exact reverse of `ids_ascending()`. That makes the fixture discriminating
/// rather than merely probable: a query with no tiebreaker returns SQLite's scan
/// order, which is insertion order, and that can never coincide with the id
/// ordering a correct query produces.
pub async fn seed_tied_books(
    db: &DatabaseConnection,
    library_id: Uuid,
    series_id: Uuid,
    count: usize,
) -> TiedBooks {
    // One timestamp for the whole batch, captured once, exactly as a scan does.
    let timestamp = Utc::now();

    let mut ids: Vec<Uuid> = (0..count).map(|_| Uuid::new_v4()).collect();
    ids.sort();
    ids.reverse();

    let mut inserted = Vec::with_capacity(count);
    for (i, id) in ids.into_iter().enumerate() {
        let model = books::Model {
            id,
            series_id,
            library_id,
            path: format!("/fixtures/tied/{id}.cbz"),
            file_name: format!("tied-{i:04}.cbz"),
            file_size: 1024,
            file_hash: format!("hash-{id}"),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: true,
            analysis_error: None,
            analysis_errors: None,
            modified_at: timestamp,
            created_at: timestamp,
            updated_at: timestamp,
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        inserted.push(
            BookRepository::create(db, &model, None)
                .await
                .expect("failed to seed tied book"),
        );
    }

    TiedBooks {
        timestamp,
        inserted,
    }
}

/// Create `series_count` series that each hold `books_each` books, so every
/// series ties on book count.
///
/// `SeriesRepository::create` mints its own id, so insertion order cannot be
/// forced the way it can for books. The batch is rebuilt until insertion order
/// differs from id order, which keeps the fixture discriminating instead of
/// leaving it to a one-in-`n!` coincidence.
pub async fn seed_tied_series_by_book_count(
    db: &DatabaseConnection,
    library_id: Uuid,
    series_count: usize,
    books_each: usize,
) -> TiedSeries {
    for attempt in 0..8 {
        let mut inserted = Vec::with_capacity(series_count);
        for i in 0..series_count {
            let series = seed_series(db, library_id, &format!("Tied {attempt}-{i:04}")).await;
            seed_tied_books(db, library_id, series.id, books_each).await;
            inserted.push(series);
        }

        let candidate = TiedSeries {
            books_each,
            inserted,
        };
        if series_count < 2 || candidate.ids_as_inserted() != candidate.ids_ascending() {
            return candidate;
        }
        // Random ids happened to come out ascending, which would make the
        // fixture unable to tell a tiebreaker from its absence. Try again.
    }
    panic!("could not build a discriminating tied-series fixture in 8 attempts");
}

/// Write one `read_progress` row per book, all sharing a single `updated_at`.
///
/// This writes the projection directly rather than going through the session
/// log, because the fixture's job is to put the database in a specific state,
/// not to exercise the write path. The realistic route to the same state is a
/// client sending a batch of sessions that share a `client_ended_at`, since the
/// fold takes `updated_at` from that client-supplied value.
pub async fn seed_tied_progress(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_ids: &[Uuid],
    completed: bool,
) -> TiedProgress {
    let timestamp = Utc::now();

    let mut inserted = Vec::with_capacity(book_ids.len());
    for book_id in book_ids {
        let row = read_progress::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            book_id: Set(*book_id),
            current_page: Set(3),
            progress_percentage: Set(None),
            completed: Set(completed),
            started_at: Set(timestamp),
            updated_at: Set(timestamp),
            completed_at: Set(if completed { Some(timestamp) } else { None }),
            r2_progression: Set(None),
        };
        inserted.push(
            row.insert(db)
                .await
                .expect("failed to seed tied read progress"),
        );
    }

    TiedProgress {
        timestamp,
        inserted,
    }
}

/// Assert that `actual` is exactly `expected`, in order.
///
/// Pin the order against a sequence the test computes from its own fixture data.
/// Never against the output of a previous run: an assertion seeded that way
/// records whatever the planner did that day, which is the behaviour under test.
///
/// Comparing two page sizes to each other is not a substitute. SQLite plans
/// `LIMIT 12` and `LIMIT 100` the same way over the same query, so the two agree
/// today and such a test passes against code that has no tiebreaker at all.
pub fn assert_exact_order(actual: &[Uuid], expected: &[Uuid], what: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{what}: expected {} rows, got {}",
        expected.len(),
        actual.len()
    );
    if actual != expected {
        let first_divergence = actual
            .iter()
            .zip(expected)
            .position(|(a, e)| a != e)
            .unwrap_or(0);
        panic!(
            "{what}: order diverges at index {first_divergence}\n  expected: {expected:?}\n  actual:   {actual:?}"
        );
    }
}

/// Assert that a sequence of pages partitions `expected`: every row appears
/// exactly once across the pages, and none is missing.
///
/// This is the failure that costs data rather than merely looking odd. Without a
/// tiebreaker a row can sort into page 1 for one query and out of it for the
/// next, so it is returned by neither.
pub fn assert_pages_partition(pages: &[Vec<Uuid>], expected: &[Uuid], what: &str) {
    let seen: Vec<Uuid> = pages.iter().flatten().copied().collect();

    let mut duplicates: Vec<Uuid> = Vec::new();
    let mut unique = std::collections::HashSet::new();
    for id in &seen {
        if !unique.insert(*id) {
            duplicates.push(*id);
        }
    }
    assert!(
        duplicates.is_empty(),
        "{what}: {} row(s) returned on more than one page: {duplicates:?}",
        duplicates.len()
    );

    let missing: Vec<Uuid> = expected
        .iter()
        .copied()
        .filter(|id| !unique.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "{what}: {} row(s) never returned by any page: {missing:?}",
        missing.len()
    );

    assert_eq!(
        seen.len(),
        expected.len(),
        "{what}: pages returned {} rows for {} expected",
        seen.len(),
        expected.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole harness rests on the timestamps being identical rather than
    /// merely close. A fixture that drifted to per-row `Utc::now()` would make
    /// every tiebreaker test vacuous, so assert it directly.
    #[tokio::test]
    async fn tied_books_share_one_timestamp() {
        let db = setup_test_db().await;
        let library = seed_library(&db, "ties").await;
        let series = seed_series(&db, library.id, "Ties").await;

        let tied = seed_tied_books(&db, library.id, series.id, 6).await;

        assert_eq!(tied.inserted.len(), 6);
        for book in &tied.inserted {
            assert_eq!(book.created_at, tied.timestamp, "created_at must be shared");
            assert_eq!(book.updated_at, tied.timestamp, "updated_at must be shared");
        }
    }

    /// The fixture is only useful if the correct order differs from the order an
    /// untied query returns. Books are inserted in descending id order so the
    /// two are exact reverses and can never coincide.
    #[tokio::test]
    async fn tied_books_are_inserted_against_id_order() {
        let db = setup_test_db().await;
        let library = seed_library(&db, "ties").await;
        let series = seed_series(&db, library.id, "Ties").await;

        let tied = seed_tied_books(&db, library.id, series.id, 6).await;

        assert_eq!(tied.ids_as_inserted(), tied.ids_descending());
        assert_ne!(tied.ids_as_inserted(), tied.ids_ascending());
    }

    #[tokio::test]
    async fn tied_series_share_a_book_count_and_do_not_arrive_in_id_order() {
        let db = setup_test_db().await;
        let library = seed_library(&db, "ties").await;

        let tied = seed_tied_series_by_book_count(&db, library.id, 5, 2).await;

        assert_eq!(tied.inserted.len(), 5);
        assert_eq!(tied.books_each, 2);
        assert_ne!(
            tied.ids_as_inserted(),
            tied.ids_ascending(),
            "fixture cannot distinguish a tiebreaker from its absence"
        );
    }

    #[tokio::test]
    async fn tied_progress_rows_share_one_timestamp() {
        let db = setup_test_db().await;
        let library = seed_library(&db, "ties").await;
        let series = seed_series(&db, library.id, "Ties").await;
        let user = seed_user(&db, "reader").await;
        let books = seed_tied_books(&db, library.id, series.id, 4).await;

        let progress = seed_tied_progress(&db, user.id, &books.ids_as_inserted(), false).await;

        assert_eq!(progress.inserted.len(), 4);
        for row in &progress.inserted {
            assert_eq!(row.updated_at, progress.timestamp);
            assert!(!row.completed);
        }
        assert_eq!(progress.book_ids_ascending(), books.ids_ascending());
    }

    #[test]
    fn assert_exact_order_accepts_a_matching_sequence() {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        assert_exact_order(&ids, &ids, "identical sequences");
    }

    #[test]
    #[should_panic(expected = "order diverges at index 0")]
    fn assert_exact_order_rejects_a_reordered_sequence() {
        let mut ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        ids.sort();
        let reversed: Vec<Uuid> = ids.iter().rev().copied().collect();
        assert_exact_order(&reversed, &ids, "reversed sequence");
    }

    #[test]
    fn assert_pages_partition_accepts_a_clean_split() {
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let pages = vec![ids[..2].to_vec(), ids[2..].to_vec()];
        assert_pages_partition(&pages, &ids, "clean split");
    }

    #[test]
    #[should_panic(expected = "never returned by any page")]
    fn assert_pages_partition_catches_a_dropped_row() {
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        // The costly half of unstable pagination on its own: a row sorted into
        // page 1 for one query and out of it for the next, so neither page
        // returns it. Kept free of duplicates so this asserts the drop alone.
        let pages = vec![ids[..2].to_vec(), vec![ids[3]]];
        assert_pages_partition(&pages, &ids, "dropped row");
    }

    #[test]
    #[should_panic(expected = "returned on more than one page")]
    fn assert_pages_partition_catches_a_repeated_row() {
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let pages = vec![ids[..2].to_vec(), vec![ids[1], ids[2], ids[3]]];
        assert_pages_partition(&pages, &ids, "repeated row");
    }
}
