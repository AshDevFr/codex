# PostgreSQL Testing Guide

## Overview

PostgreSQL tests are marked with `#[ignore]` because they require a running PostgreSQL instance. This prevents them from running during normal `cargo test` execution.

## Prerequisites

You need a PostgreSQL server with a test database configured.

### Using Docker (Recommended)

```bash
# Start PostgreSQL in Docker
docker run --name codex-postgres-test \
  -e POSTGRES_USER=codex_test \
  -e POSTGRES_PASSWORD=codex_test \
  -e POSTGRES_DB=codex_test \
  -p 5432:5432 \
  -d postgres:16

# Verify it's running
docker ps | grep codex-postgres-test
```

### Using Local PostgreSQL

Create a test user and database:

```sql
CREATE USER codex_test WITH PASSWORD 'codex_test';
CREATE DATABASE codex_test OWNER codex_test;
```

## Running PostgreSQL Tests

### Run all PostgreSQL tests

```bash
# Run ignored tests (requires PostgreSQL)
cargo test --test postgres_integration_tests -- --ignored

# Run unit tests in postgres.rs
cargo test db::postgres -- --ignored
```

### Run specific tests

```bash
# Run a specific test
cargo test test_postgres_library_insert_and_select -- --ignored

# Run connection tests
cargo test db::connection::test_database_new_postgres -- --ignored
```

### Environment Variables

You can customize the PostgreSQL connection using environment variables:

```bash
export POSTGRES_HOST=localhost
export POSTGRES_PORT=5432
export POSTGRES_USER=codex_test
export POSTGRES_PASSWORD=codex_test
export POSTGRES_DB=codex_test

cargo test --test postgres_integration_tests -- --ignored
```

## Available PostgreSQL Tests

### Integration Tests (5 tests)

1. `test_postgres_library_insert_and_select` - Basic CRUD operations
2. `test_postgres_series_book_relationship` - Multi-table relationships
3. `test_postgres_cascade_delete` - CASCADE DELETE verification
4. `test_postgres_health_check` - Connection health
5. `test_postgres_reconnect` - Reconnection handling

### Unit Tests (3 tests)

1. `test_new_postgres_connection` - Connection establishment
2. `test_postgres_connection_failure` - Error handling
3. `test_postgres_health_check` - Health check functionality

## Cleanup

### Docker

```bash
# Stop and remove the test container
docker stop codex-postgres-test
docker rm codex-postgres-test
```

### Local PostgreSQL

```sql
DROP DATABASE codex_test;
DROP USER codex_test;
```

## CI/CD Integration

For CI pipelines, use a PostgreSQL service:

### GitHub Actions

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_USER: codex_test
      POSTGRES_PASSWORD: codex_test
      POSTGRES_DB: codex_test
    ports:
      - 5432:5432
    options: >-
      --health-cmd pg_isready
      --health-interval 10s
      --health-timeout 5s
      --health-retries 5

steps:
  - name: Run PostgreSQL tests
    run: cargo test --test postgres_integration_tests -- --ignored
```

## Test Data

All PostgreSQL tests clean up after themselves by deleting created data. No manual cleanup is required between test runs.

## Troubleshooting

### Connection Refused

```
Error: Failed to connect to PostgreSQL database
```

**Solution:** Ensure PostgreSQL is running and accessible on port 5432.

### Authentication Failed

```
Error: password authentication failed for user "codex_test"
```

**Solution:** Check username and password are correct, or set environment variables.

### Database Does Not Exist

```
Error: database "codex_test" does not exist
```

**Solution:** Create the database first or ensure POSTGRES_DB is set correctly.

### Permission Denied

```
Error: permission denied to create table
```

**Solution:** Ensure the test user has CREATE privileges on the database.
