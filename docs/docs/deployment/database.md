---
sidebar_position: 6
---

# Database Setup

Codex supports PostgreSQL and SQLite databases.

## Database Comparison

| Feature | PostgreSQL | SQLite |
|---------|------------|--------|
| Multi-user | Excellent | Limited |
| Horizontal scaling | Yes | No |
| Separate workers | Yes | No |
| Setup complexity | Moderate | Simple |
| Best for | Production | Homelab |

## PostgreSQL

### Installation

#### Docker

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_USER: codex
      POSTGRES_PASSWORD: your-secure-password
      POSTGRES_DB: codex
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U codex"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  postgres_data:
```

#### Linux Package

```bash
# Ubuntu/Debian
sudo apt install postgresql postgresql-contrib

# Fedora/RHEL
sudo dnf install postgresql-server postgresql-contrib
sudo postgresql-setup --initdb
sudo systemctl enable --now postgresql
```

### Create Database

```bash
# Connect as postgres user
sudo -u postgres psql

# Create database and user
CREATE DATABASE codex;
CREATE USER codex WITH ENCRYPTED PASSWORD 'your-secure-password';
GRANT ALL PRIVILEGES ON DATABASE codex TO codex;

# For PostgreSQL 15+, also grant schema permissions
\c codex
GRANT ALL ON SCHEMA public TO codex;

\q
```

### Configuration

```yaml
# codex.yaml
database:
  db_type: postgres
  postgres:
    host: localhost
    port: 5432
    user: codex
    password: your-secure-password
    database: codex
```

Or via environment variables:
```bash
CODEX_DATABASE_DB_TYPE=postgres
CODEX_DATABASE_POSTGRES_HOST=localhost
CODEX_DATABASE_POSTGRES_PORT=5432
CODEX_DATABASE_POSTGRES_USER=codex
CODEX_DATABASE_POSTGRES_PASSWORD=your-secure-password
CODEX_DATABASE_POSTGRES_DATABASE=codex
```

### Connection Pooling

For high-traffic deployments, configure connection pooling:

```yaml
database:
  postgres:
    max_connections: 100
    min_connections: 5
    connect_timeout: 30
    idle_timeout: 600
```

### Backups

```bash
# Manual backup
pg_dump -U codex codex > backup_$(date +%Y%m%d).sql

# Compressed backup
pg_dump -U codex codex | gzip > backup_$(date +%Y%m%d).sql.gz

# Restore
psql -U codex codex < backup_20240101.sql

# Or compressed
gunzip -c backup_20240101.sql.gz | psql -U codex codex
```

#### Automated Backups

```bash
# /etc/cron.d/codex-backup
0 2 * * * postgres pg_dump -U codex codex | gzip > /backup/codex_$(date +\%Y\%m\%d).sql.gz
```

## SQLite

### Setup

SQLite requires no setup. The database is created automatically:

```yaml
# codex.yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db
```

Ensure the directory exists and is writable:
```bash
mkdir -p ./data
```

### Limitations

:::warning SQLite Limitations
- **Single writer** - Only one process can write at a time
- **No horizontal scaling** - Cannot run multiple Codex instances
- **No separate workers** - Must use `codex serve` (combined mode)
- **Limited concurrency** - Best for 5-10 concurrent users
:::

### Backups

```bash
# Ensure Codex is stopped or using WAL mode
cp ./data/codex.db /backup/codex_$(date +%Y%m%d).db

# With WAL files (if using WAL mode)
cp ./data/codex.db ./data/codex.db-wal ./data/codex.db-shm /backup/
```

### WAL Mode

SQLite WAL mode improves concurrency:

```yaml
database:
  sqlite:
    path: ./data/codex.db
    journal_mode: wal
```

## Migrations

Codex runs migrations automatically on startup. No manual intervention is required.

Check migration status in logs:
```
INFO Running database migrations...
INFO Migrations completed successfully
```

## Troubleshooting

### PostgreSQL Connection Refused

```bash
# Check PostgreSQL is running
sudo systemctl status postgresql

# Check listening port
sudo ss -tlnp | grep 5432

# Check pg_hba.conf allows connections
sudo cat /etc/postgresql/16/main/pg_hba.conf
```

### SQLite Locked

```
Error: database is locked
```

This occurs when multiple processes try to write simultaneously:
- Ensure only one Codex instance is running
- Use `codex serve` instead of separate `codex worker`
- Consider switching to PostgreSQL

### Migration Failed

```bash
# Check logs for specific error
journalctl -u codex | grep -i migration

# If needed, restore from backup
psql -U codex codex < backup.sql
```
