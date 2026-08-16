---
sidebar_position: 6
---

# Database Setup

Codex supports PostgreSQL and SQLite databases.

:::tip
For quick configuration examples and all database-related settings, see the [Configuration guide](../configuration#database-configuration).
:::

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
    username: codex
    password: your-secure-password
    database_name: codex
```

Or via environment variables:
```bash
CODEX_DATABASE_DB_TYPE=postgres
CODEX_DATABASE_POSTGRES_HOST=localhost
CODEX_DATABASE_POSTGRES_PORT=5432
CODEX_DATABASE_POSTGRES_USERNAME=codex
CODEX_DATABASE_POSTGRES_PASSWORD=your-secure-password
CODEX_DATABASE_POSTGRES_DATABASE_NAME=codex
```

### TLS

Configure TLS under `database.postgres`:

```yaml
database:
  postgres:
    ssl_mode: verify-full
    ssl_root_cert: /etc/ssl/certs/postgres-ca.crt
```

or with `CODEX_DATABASE__POSTGRES__SSL_MODE` and
`CODEX_DATABASE__POSTGRES__SSL_ROOT_CERT`.

Leaving `ssl_mode` unset falls back to the driver's own resolution, which is
`prefer` unless `PGSSLMODE` says otherwise: it negotiates TLS when the server
offers it, accepts any certificate, and drops to an unencrypted connection when
the server offers nothing. Both the missing verification and the downgrade are
silent. Keep that only when Codex and PostgreSQL sit on a network you trust.

Use `verify-full` for a managed or remote database. `verify-ca` skips the
hostname check and is the fallback when the certificate's subject does not
match the host you connect to. `require` encrypts without verifying anything,
which stops passive capture but not an active attacker.

For mutual TLS, add `ssl_client_cert` and `ssl_client_key`.

:::note
The libpq variables (`PGSSLMODE`, `PGSSLROOTCERT`, `PGSSLCERT`, `PGSSLKEY`) are
still read when the matching Codex setting is unset, so a deployment configured
that way keeps working; the Codex setting wins when both are present.

A `?sslmode=` query parameter on a `codex copy` URL is **not** honored: the URL
is decomposed into host, port, user, password and database name, and query
parameters are discarded. Configure TLS on the config file for that side, or
use `PGSSLMODE`.
:::

### Connection Pooling

For high-traffic deployments, configure connection pooling:

```yaml
database:
  postgres:
    max_connections: 25
    min_connections: 2
    connect_timeout: 30
    idle_timeout: 600
```

`max_connections` is a per-process ceiling, not a server-wide one. When several Codex processes share a database (web replicas, workers, the migration Job, the backup CronJob), the sum of their pools has to stay under the server's own `max_connections` less `superuser_reserved_connections`. See [Performance tuning](./performance.md) for how to size it.

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
