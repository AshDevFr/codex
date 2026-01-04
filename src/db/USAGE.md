# Database Usage Guide

## Quick Start

The database module provides SQLite support with automatic schema creation and migrations.

### Basic Setup

```rust
use codex::config::{DatabaseConfig, DatabaseType, SQLiteConfig};
use codex::db::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure database
    let config = DatabaseConfig {
        db_type: DatabaseType::SQLite,
        postgres: None,
        sqlite: Some(SQLiteConfig {
            path: "./codex.db".to_string(),
            pragmas: None,
        }),
    };

    // Create connection (auto-creates file and runs migrations if needed)
    let db = Database::new(&config).await?;

    // Use the database
    let pool = db.pool();

    // ... perform queries ...

    // Close when done
    db.close().await;

    Ok(())
}
```

## Features

### Automatic Database Creation

When you call `Database::new()` with a SQLite path that doesn't exist:

1. **Parent directories are created** if they don't exist
2. **Database file is created** automatically
3. **Migrations are run** to create all tables and indexes
4. **Connection pool is established** and ready to use

### Default Configuration

The SQLite connection is configured with:

- **WAL mode** - Write-Ahead Logging for better concurrency
- **Normal synchronous mode** - Balance between safety and performance
- **5 second busy timeout** - Prevents immediate lock failures
- **5 max connections** - Connection pool size

### Custom Pragmas

You can override SQLite pragmas:

```rust
use std::collections::HashMap;

let mut pragmas = HashMap::new();
pragmas.insert("foreign_keys".to_string(), "ON".to_string());
pragmas.insert("cache_size".to_string(), "-64000".to_string());

let config = SQLiteConfig {
    path: "./codex.db".to_string(),
    pragmas: Some(pragmas),
};
```

## Schema

The database includes these tables:

- **libraries** - Top-level content collections
- **series** - Collections of related books
- **books** - Individual files with metadata
- **book_metadata_records** - Extended metadata (ComicInfo.xml, etc.)
- **pages** - Individual pages within books
- **users** - User accounts
- **read_progress** - Reading progress tracking
- **metadata_sources** - External metadata source tracking

See [README.md](README.md) for detailed schema documentation.

## Running the Example

```bash
cargo run --example database_example
```

This will:
1. Create `example_codex.db` in the project root
2. Run migrations to create all tables
3. Perform a health check
4. Display the created tables

## Health Checks

Check database connectivity:

```rust
db.health_check().await?;
```

## Manual Migrations

If you need to run migrations manually:

```rust
db.run_migrations().await?;
```

## Testing

Run database tests:

```bash
cargo test db::connection
```

All tests use temporary directories and are cleaned up automatically.
