# Testing Guide

This directory contains integration and end-to-end tests for Codex.

## Test Organization

- `api/` - REST API endpoint tests
- `db/` - Database repository tests
- `parsers/` - File parser tests
- `scanner/` - Library scanner tests
- `common/` - Shared test utilities and fixtures

## Database Testing

Codex supports both SQLite and PostgreSQL. To ensure compatibility and catch database-specific bugs, we provide helpers for testing against both databases.

### SQLite Tests (Default)

Most tests use SQLite as it's fast and requires no external dependencies:

```rust
use common::setup_test_db;

#[tokio::test]
async fn test_something() {
    let (db, _temp_dir) = setup_test_db().await;
    // ... test code
}
```

### PostgreSQL Tests (Optional)

Some tests also run against PostgreSQL to catch database-specific issues. PostgreSQL is stricter about SQL syntax, especially:
- Ambiguous column names in JOINs
- Data type casting
- Transaction isolation
- NULL handling in aggregations

To run PostgreSQL tests:

1. Start the test database:
   ```bash
   docker-compose up -d postgres-test
   ```

2. Run tests normally:
   ```bash
   cargo test
   ```

PostgreSQL tests will automatically skip if the database is not available:

```rust
use common::setup_test_db_postgres;

#[tokio::test]
async fn test_something_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        // Skip test if PostgreSQL is not available
        return;
    };
    // ... test code
}
```

### Environment Variables

- `POSTGRES_TEST_URL` - Override the default PostgreSQL connection URL
  - Default: `postgres://codex:codex@localhost:54321/codex_test`

## When to Add PostgreSQL Tests

Add a PostgreSQL-specific test when:

1. **Using JOINs with aggregations** (SUM, COUNT, etc.)
   - PostgreSQL requires qualified column names when tables have overlapping column names
   - Example: `test_get_metrics_postgres` verifies the metrics endpoint works with JOINs

2. **Complex SQL queries**
   - Window functions, CTEs, or subqueries
   - Database-specific functions

3. **Transaction behavior**
   - Testing concurrent access or isolation levels

4. **A production bug was PostgreSQL-specific**
   - Add a regression test to prevent reoccurrence

## Common Patterns

### Creating Test Data

Use the helpers in `common/fixtures.rs`:

```rust
use common::*;

let book = create_test_book(
    series_id,
    "/path/to/book.cbz",
    "book.cbz",
    "hash123",
    "cbz",
    10, // page_count
);
```

### Testing API Endpoints

Use the HTTP helpers in `common/http.rs`:

```rust
use common::*;

let state = create_test_auth_state(db.clone());
let token = create_admin_and_token(&db, &state).await;
let app = create_test_router(state);

let request = get_request_with_auth("/api/v1/metrics", &token);
let (status, response): (StatusCode, Option<MetricsDto>) =
    make_json_request(app, request).await;

assert_eq!(status, StatusCode::OK);
```

## Running Tests

```bash
# Run all tests (SQLite only)
cargo test

# Run specific test file
cargo test --test api

# Run specific test
cargo test test_get_metrics_postgres

# Run with PostgreSQL tests
docker-compose up -d postgres-test
cargo test

# Show test output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4
```

## CI/CD Considerations

For CI pipelines, consider:

1. Running SQLite tests on every commit (fast feedback)
2. Running PostgreSQL tests on pull requests
3. Setting `POSTGRES_TEST_URL` in CI environment
4. Using docker-compose to start test database in CI

Example GitHub Actions:

```yaml
- name: Start PostgreSQL
  run: docker-compose up -d postgres-test

- name: Wait for PostgreSQL
  run: |
    timeout 30 bash -c 'until docker-compose exec -T postgres-test pg_isready; do sleep 1; done'

- name: Run tests
  run: cargo test
```

## Troubleshooting

### PostgreSQL tests are skipped

- Ensure the test database is running: `docker-compose up -d postgres-test`
- Check the connection URL matches your setup
- Verify the database is accessible: `psql postgres://codex:codex@localhost:54321/codex_test`

### Tests fail with "database locked" (SQLite)

- Reduce test parallelism: `cargo test -- --test-threads=1`
- Or use separate test databases per test

### Cleanup between tests

PostgreSQL tests automatically truncate tables before running. For SQLite, each test gets a fresh database via TempDir.
