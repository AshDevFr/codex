---
---

# Configuration

Codex uses YAML configuration files with environment variable overrides. This guide covers all configuration options and best practices.

## Configuration File

Codex looks for configuration in the following order:

1. Path specified via `--config` flag
2. `codex.yaml` in the current directory
3. Default values

```bash
codex serve --config /path/to/codex.yaml
```

## Configuration Priority

Settings can come from multiple sources, with this priority (highest to lowest):

1. **Environment variables** (override everything)
2. **Configuration file** (YAML)
3. **Database settings** (runtime-configurable options)
4. **Hardcoded defaults** (fallback)

## Database Configuration

Codex supports both SQLite and PostgreSQL databases.

:::tip
For detailed database setup instructions including installation, user creation, and troubleshooting, see the [Database Setup guide](./deployment/database).
:::

### SQLite (Recommended for Simple Setups)

Best for single-user or small deployments with fewer than 10,000 books.

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db
    pragmas:
      journal_mode: WAL
      synchronous: NORMAL
```

#### SQLite Pragmas

| Pragma | Options | Description |
|--------|---------|-------------|
| `journal_mode` | `WAL` (recommended), `DELETE`, `TRUNCATE` | WAL provides better concurrency |
| `synchronous` | `NORMAL` (recommended), `FULL`, `OFF` | Trade-off between safety and speed |
| `foreign_keys` | Always `ON` | Cannot be disabled (data integrity) |

:::tip WAL Mode
**Write-Ahead Logging (WAL)** is strongly recommended for SQLite. It provides:
- Better read/write concurrency
- Faster writes for most workloads
- Crash recovery improvements
:::

### SQLite Connection Pool

SQLite connection pool settings can be tuned for your workload:

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db
    # Connection pool settings
    max_connections: 16        # Maximum pool size (default: 16)
    min_connections: 2         # Minimum warm connections (default: 2)
    acquire_timeout_seconds: 30  # Wait time for connection (default: 30)
    idle_timeout_seconds: 300    # Idle connection timeout (default: 300 = 5 min)
    max_lifetime_seconds: 1800   # Max connection lifetime (default: 1800 = 30 min)
```

| Setting | Default | Description |
|---------|---------|-------------|
| `max_connections` | `16` | Maximum connections in pool |
| `min_connections` | `2` | Minimum warm connections |
| `acquire_timeout_seconds` | `30` | How long to wait for a connection |
| `idle_timeout_seconds` | `300` | Idle connection timeout (5 min) |
| `max_lifetime_seconds` | `1800` | Maximum connection lifetime (30 min) |

:::tip SQLite Pool Sizing
SQLite with WAL mode handles concurrent reads well, but writes are serialized. The default of 16 connections works well for most workloads. Increase if you see "connection pool timeout" errors during heavy load.
:::

### PostgreSQL (Recommended for Production)

Best for multi-user environments, large libraries, or horizontal scaling.

```yaml
database:
  db_type: postgres
  postgres:
    host: localhost
    port: 5432
    username: codex
    password: codex
    database_name: codex
    ssl_mode: prefer
    # Connection pool settings
    max_connections: 100       # Maximum pool size (default: 100)
    min_connections: 5         # Minimum warm connections (default: 5)
    acquire_timeout_seconds: 30  # Wait time for connection (default: 30)
    idle_timeout_seconds: 600    # Idle connection timeout (default: 600 = 10 min)
    max_lifetime_seconds: 3600   # Max connection lifetime (default: 3600 = 1 hour)
```

#### PostgreSQL SSL Modes

| Mode | Description |
|------|-------------|
| `disable` | No SSL (not recommended for production) |
| `allow` | Try without SSL, use SSL if available |
| `prefer` | Try SSL first, fallback to no SSL (default) |
| `require` | SSL required, but don't verify certificate |
| `verify-ca` | SSL required, verify server certificate |
| `verify-full` | SSL required, verify certificate and hostname |

:::caution Production Security
For production deployments, use `verify-ca` or `verify-full` SSL mode with proper certificates.
:::

## Application Configuration

```yaml
application:
  name: Codex           # Server name (displayed in UI)
  host: 0.0.0.0         # Bind address (0.0.0.0 for all interfaces)
  port: 8080            # Server port
```

| Setting | Default | Description |
|---------|---------|-------------|
| `name` | `Codex` | Server display name |
| `host` | `127.0.0.1` | Bind address |
| `port` | `8080` | HTTP port |

## Authentication Configuration

```yaml
auth:
  jwt_secret: "CHANGE_ME_IN_PRODUCTION"
  jwt_expiry_hours: 24
  refresh_token_enabled: false
  email_confirmation_required: false
  argon2_memory_cost: 19456
  argon2_time_cost: 2
  argon2_parallelism: 1
```

| Setting | Default | Description |
|---------|---------|-------------|
| `jwt_secret` | Required | Secret key for JWT signing |
| `jwt_expiry_hours` | `24` | Token validity period |
| `refresh_token_enabled` | `false` | Enable refresh tokens |
| `email_confirmation_required` | `false` | Require email verification |
| `argon2_memory_cost` | `19456` | Argon2 memory cost (KiB) |
| `argon2_time_cost` | `2` | Argon2 iterations |
| `argon2_parallelism` | `1` | Argon2 parallelism |

:::danger JWT Secret
**Always change the JWT secret in production!** Generate a secure random string:

```bash
openssl rand -base64 32
```
:::

## API Configuration

```yaml
api:
  enable_api_docs: false
  api_docs_path: "/docs"
  cors_enabled: true
  max_page_size: 100
```

| Setting | Default | Description |
|---------|---------|-------------|
| `enable_api_docs` | `false` | Enable API documentation (Scalar) |
| `api_docs_path` | `/docs` | API documentation URL path |
| `cors_enabled` | `true` | Enable CORS |
| `max_page_size` | `100` | Maximum items per page |

## Logging Configuration

```yaml
logging:
  level: info
  # file: ./logs/codex.log  # Uncomment to enable file logging
```

| Setting | Default | Description |
|---------|---------|-------------|
| `level` | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `file` | None | Optional log file path |

## Task Worker Configuration

These settings require a restart to take effect.

```yaml
task:
  worker_count: 4
```

| Setting | Default | Description |
|---------|---------|-------------|
| `worker_count` | `4` | Number of parallel background workers |

## Scanner Configuration

These settings require a restart to take effect.

```yaml
scanner:
  max_concurrent_scans: 2
```

| Setting | Default | Description |
|---------|---------|-------------|
| `max_concurrent_scans` | `2` | Maximum concurrent library scans |

## Files Configuration

Configuration for file storage directories (thumbnails and uploads).

```yaml
files:
  thumbnail_dir: data/thumbnails
  uploads_dir: data/uploads
```

| Setting | Default | Description |
|---------|---------|-------------|
| `thumbnail_dir` | `data/thumbnails` | Directory for thumbnail cache |
| `uploads_dir` | `data/uploads` | Directory for user-uploaded files (covers, etc.) |

Additional thumbnail settings are stored in the database and can be changed via the Settings API without restart:
- `thumbnail_max_dimension` - Maximum width/height (default: 400px)
- `thumbnail_jpeg_quality` - JPEG quality (default: 85%)

## Email Configuration (Optional)

For email verification and notifications:

```yaml
email:
  smtp_host: localhost
  smtp_port: 587
  smtp_username: ""
  smtp_password: ""
  smtp_from_email: noreply@example.com
  smtp_from_name: Codex
  verification_token_expiry_hours: 24
  verification_url_base: http://localhost:8080
```

## Environment Variables

All configuration options can be overridden with environment variables using the `CODEX_` prefix.

### Naming Convention

Configuration paths are converted to environment variables:
- Use uppercase
- Replace dots with underscores
- Prefix with `CODEX_`

| Config Path | Environment Variable |
|-------------|---------------------|
| `database.db_type` | `CODEX_DATABASE_DB_TYPE` |
| `database.postgres.host` | `CODEX_DATABASE_POSTGRES_HOST` |
| `auth.jwt_secret` | `CODEX_AUTH_JWT_SECRET` |
| `logging.level` | `CODEX_LOGGING_LEVEL` |

### Common Environment Variables

```bash
# Database
CODEX_DATABASE_DB_TYPE=postgres
CODEX_DATABASE_POSTGRES_HOST=localhost
CODEX_DATABASE_POSTGRES_PORT=5432
CODEX_DATABASE_POSTGRES_USERNAME=codex
CODEX_DATABASE_POSTGRES_PASSWORD=secret
CODEX_DATABASE_POSTGRES_DATABASE_NAME=codex
CODEX_DATABASE_POSTGRES_SSL_MODE=require

# Application
CODEX_APPLICATION_HOST=0.0.0.0
CODEX_APPLICATION_PORT=8080

# Authentication
CODEX_AUTH_JWT_SECRET=your-secure-secret-key

# Logging
CODEX_LOGGING_LEVEL=debug
CODEX_LOGGING_FILE=/var/log/codex/codex.log

# API
CODEX_API_ENABLE_API_DOCS=true

# Task Workers
CODEX_TASK_WORKER_COUNT=4

# Scanner
CODEX_SCANNER_MAX_CONCURRENT_SCANS=2

# Files (thumbnails and uploads)
CODEX_FILES_THUMBNAIL_DIR=data/thumbnails
CODEX_FILES_UPLOADS_DIR=data/uploads
```

## Runtime vs Startup Settings

Some settings can be changed at runtime via the Settings API, while others require a restart.

### Runtime-Configurable (No Restart Required)

These settings are stored in the database and can be changed via `/api/v1/admin/settings`:

- Thumbnail max dimension
- Thumbnail JPEG quality
- Application name
- Logging level

### Startup-Time (Restart Required)

These settings are read from the config file at startup:

- Database connection settings
- Task worker count
- Scanner concurrent scan limit
- Thumbnail cache directory
- JWT secret
- Server host/port

## Example Configurations

### Minimal SQLite Configuration

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db

application:
  host: 127.0.0.1
  port: 8080

auth:
  jwt_secret: "your-secure-random-secret"
```

### Production PostgreSQL Configuration

```yaml
database:
  db_type: postgres
  postgres:
    host: db.example.com
    port: 5432
    username: codex
    password: ${DB_PASSWORD}
    database_name: codex
    ssl_mode: verify-full

application:
  name: My Library
  host: 0.0.0.0
  port: 8080

logging:
  level: info
  file: /var/log/codex/codex.log

auth:
  jwt_secret: ${JWT_SECRET}
  jwt_expiry_hours: 12

api:
  enable_api_docs: false
  cors_enabled: true

task:
  worker_count: 8

scanner:
  max_concurrent_scans: 4

files:
  thumbnail_dir: /var/lib/codex/thumbnails
  uploads_dir: /var/lib/codex/uploads
```

### Kubernetes Configuration

For Kubernetes deployments, use environment variables for all sensitive data:

```yaml
# Minimal config file - most settings come from environment
task:
  worker_count: 4

scanner:
  max_concurrent_scans: 2

files:
  thumbnail_dir: data/thumbnails
  uploads_dir: data/uploads
```

Set these via Kubernetes ConfigMaps and Secrets:

```bash
CODEX_DATABASE_DB_TYPE=postgres
CODEX_DATABASE_POSTGRES_HOST=postgres-service
CODEX_DATABASE_POSTGRES_PORT=5432
CODEX_DATABASE_POSTGRES_USERNAME=<from secret>
CODEX_DATABASE_POSTGRES_PASSWORD=<from secret>
CODEX_DATABASE_POSTGRES_DATABASE_NAME=codex
CODEX_AUTH_JWT_SECRET=<from secret>
```

## Configuration Validation

Codex validates configuration at startup. Common errors:

| Error | Cause | Solution |
|-------|-------|----------|
| Invalid database type | `db_type` not `sqlite` or `postgres` | Fix the db_type value |
| Missing database path | SQLite requires a path | Add `sqlite.path` |
| Database connection failed | Wrong credentials or host | Check connection settings |
| Invalid port | Port outside 1-65535 range | Use a valid port number |
| File permissions | Can't write to paths | Check directory permissions |

## Security Best Practices

1. **Use strong JWT secrets** - Generate with `openssl rand -base64 32`
2. **Never commit secrets** - Use environment variables or secret managers
3. **Use SSL for PostgreSQL** - Set `ssl_mode: verify-full` in production
4. **Restrict bind address** - Use `127.0.0.1` unless needed externally
5. **Disable API docs in production** - Set `enable_api_docs: false`

## Next Steps

- [Deploy Codex](./deployment)
- [Set up your first library](./getting-started)
- [Explore the API](./api)
