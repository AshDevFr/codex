//! Connection-pool contention regression tests.
//!
//! These guard the per-request query fan-out bound (the `db_batch` helper).
//! A list request enriches its rows from many related tables; without a bound
//! each query grabs its own pool connection, so a handful of concurrent
//! requests exhaust a small pool and `acquire()` blocks for seconds. With the
//! bound, a single request holds at most `batch_fan_out` connections, so many
//! concurrent requests complete promptly instead of timing out.
//!
//! The SQLite test runs against a deliberately small pool with a short acquire
//! timeout: if the fan-out bound regressed (back to unbounded `tokio::join!`),
//! the cross-request demand on the tiny pool would push `acquire()` past the
//! timeout and the requests would fail with 500s rather than 200s.

#[path = "../common/mod.rs"]
mod common;

use std::collections::HashMap;
use std::time::Duration;

use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use tempfile::TempDir;

/// Number of concurrent `GET /api/v1/series` requests to fire.
const CONCURRENT_REQUESTS: usize = 24;
/// Series to seed so the list endpoint does real per-row enrichment work.
///
/// Sized large enough that a regression to the old per-series N+1 fan-out
/// (`join_all` over the singular `series_to_dto`, ~6 queries per series, all
/// unbounded) would demand far more than the 4-connection pool can supply and
/// blow the acquire timeout — whereas the batched converter issues a fixed
/// handful of queries regardless of page size.
const SEEDED_SERIES: usize = 60;

/// Build a SQLite database with a small connection pool and a short acquire
/// timeout, so connection starvation surfaces as a fast failure rather than a
/// 30s hang. Returns the connection plus the TempDir (kept alive by the caller).
async fn small_pool_sqlite() -> (sea_orm::DatabaseConnection, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("contention.db");

    let mut pragmas = HashMap::new();
    pragmas.insert("foreign_keys".to_string(), "ON".to_string());
    pragmas.insert("journal_mode".to_string(), "WAL".to_string());

    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: db_path.to_str().unwrap().to_string(),
            pragmas: Some(pragmas),
            // Smaller than the full series DTO fan-out (14) so an unbounded
            // regression would saturate the pool under concurrency.
            max_connections: 4,
            min_connections: 1,
            // Fail fast instead of hanging 30s if the pool is genuinely starved.
            acquire_timeout_seconds: 5,
            ..SQLiteConfig::default()
        }),
    };

    let database = Database::new(&config).await.unwrap();
    database.run_migrations().await.unwrap();
    (database.sea_orm_connection().clone(), temp_dir)
}

/// Seed a library with `SEEDED_SERIES` series and return an admin JWT for the
/// given auth state.
async fn seed_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let library =
        LibraryRepository::create(db, "Library", "/lib", codex::db::ScanningStrategy::Default)
            .await
            .unwrap();
    for i in 0..SEEDED_SERIES {
        SeriesRepository::create(db, library.id, &format!("Series {i}"), None)
            .await
            .unwrap();
    }

    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

/// Fire `CONCURRENT_REQUESTS` simultaneous list requests against a 4-connection
/// SQLite pool. With the fan-out bound in place they all succeed; an unbounded
/// regression would starve the pool and return 500s (acquire timeout).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_series_list_does_not_starve_small_pool() {
    let (db, _temp_dir) = small_pool_sqlite().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = seed_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let mut handles = Vec::with_capacity(CONCURRENT_REQUESTS);
    for _ in 0..CONCURRENT_REQUESTS {
        let app = app.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            let request = get_request_with_auth("/api/v1/series", &token);
            make_request(app, request).await.0
        }));
    }

    for handle in handles {
        // The hard timeout is a deadlock guard, not a perf assertion: the
        // acquire timeout (5s) would already turn starvation into a 500.
        let status = tokio::time::timeout(Duration::from_secs(30), handle)
            .await
            .expect("a request hung well past the acquire timeout")
            .expect("request task panicked");
        assert_eq!(
            status,
            StatusCode::OK,
            "a concurrent list request failed — the connection pool was likely starved"
        );
    }
}

/// PostgreSQL parity: the same concurrent-list workload must not regress on
/// Postgres. Ignored by default; runs when a test PostgreSQL server is
/// available (see `setup_test_db_postgres`). Postgres uses a larger default
/// pool and true parallel execution, so this is a no-regression smoke rather
/// than a tight small-pool contention test.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore] // Requires PostgreSQL server
async fn concurrent_series_list_postgres_parity() {
    let Some(db) = setup_test_db_postgres().await else {
        return; // PostgreSQL not available; skip.
    };

    let library =
        LibraryRepository::create(&db, "Library", "/lib", codex::db::ScanningStrategy::Default)
            .await
            .unwrap();
    for i in 0..SEEDED_SERIES {
        SeriesRepository::create(&db, library.id, &format!("Series {i}"), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    let app = create_test_router(state).await;

    let mut handles = Vec::with_capacity(CONCURRENT_REQUESTS);
    for _ in 0..CONCURRENT_REQUESTS {
        let app = app.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            let request = get_request_with_auth("/api/v1/series", &token);
            make_request(app, request).await.0
        }));
    }

    for handle in handles {
        let status = tokio::time::timeout(Duration::from_secs(30), handle)
            .await
            .expect("a request hung")
            .expect("request task panicked");
        assert_eq!(status, StatusCode::OK);
    }
}
