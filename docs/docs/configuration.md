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

## PDF Rendering Configuration

Codex can render PDF pages server-side using the PDFium library. This enables:
- Thumbnails and covers for all PDF types (text-only, vector graphics, scanned)
- Server-side page rendering for the streaming reader mode

```yaml
pdf:
  # pdfium_library_path: /path/to/libpdfium.so  # Optional, auto-detected if not set
  render_dpi: 150              # Render DPI (72-300, higher = better quality, larger files)
  jpeg_quality: 85             # JPEG compression quality (1-100)
  cache_rendered_pages: true   # Cache rendered pages to disk
  cache_dir: data/cache        # Cache directory for rendered PDF pages
```

| Setting | Default | Description |
|---------|---------|-------------|
| `pdfium_library_path` | Auto-detect | Path to PDFium shared library. Usually not needed - Codex automatically searches the executable directory and system library paths |
| `render_dpi` | `150` | Render resolution in DPI. Higher values produce sharper images but larger files |
| `jpeg_quality` | `85` | JPEG compression quality (1-100). Higher values = better quality, larger files |
| `cache_rendered_pages` | `true` | Enable disk caching of rendered PDF pages |
| `cache_dir` | `data/cache` | Directory for PDF page cache (stored in `{cache_dir}/pdf_pages/`) |

### PDFium Library Installation

#### Docker (Recommended)

PDFium is bundled in the official Docker image. No additional setup required.

#### Binary Installation (Linux)

For standalone binary deployments, install PDFium separately:

```bash
# Download pre-built PDFium library (Debian/Ubuntu with glibc)
wget -O- https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-x64.tgz \
  | sudo tar -xz -C /usr/local
sudo ldconfig

# Or for Alpine/musl-based systems
wget -O- https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-musl-x64.tgz \
  | sudo tar -xz -C /usr/local
```

#### macOS

```bash
# Download PDFium for macOS
wget -O- https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-mac-x64.tgz \
  | sudo tar -xz -C /usr/local

# Or for Apple Silicon (arm64)
wget -O- https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-mac-arm64.tgz \
  | sudo tar -xz -C /usr/local
```

#### Windows

1. Download `pdfium-win-x64.zip` from [bblanchon/pdfium-binaries releases](https://github.com/bblanchon/pdfium-binaries/releases)
2. Extract `pdfium.dll` to a directory in your `PATH`
3. Or set `CODEX_PDF_PDFIUM_LIBRARY_PATH` to the full path of `pdfium.dll`

### Without PDFium

If PDFium is not installed:
- **Scanned PDFs** (with embedded images): Work normally via embedded image extraction
- **Text-only PDFs**: Page extraction will fail, but the PDF can still be viewed in native mode

:::tip Native PDF Mode
Users can switch to native PDF mode in the reader settings, which downloads the full PDF and renders it client-side using pdf.js. This works without PDFium but uses more bandwidth.
:::

### Cache Management

Rendered PDF pages are cached to disk to improve performance. The cache structure is:

```
{cache_dir}/pdf_pages/{book_id_prefix}/{book_id}/page_{number}_{dpi}.jpg
```

Cache is automatically invalidated when:
- A book file is updated (detected by file hash change during scan)
- The book is deleted from the library

To manually clear the cache:
- Delete a specific book's cache: Remove `{cache_dir}/pdf_pages/{book_id}/`
- Clear all cached pages: Remove `{cache_dir}/pdf_pages/`

## Komga-Compatible API (Optional)

Codex can expose a Komga-compatible API, allowing you to use third-party apps designed for Komga (such as Komic for iOS) with your Codex server.

:::info
This feature is **disabled by default** and must be explicitly enabled in your configuration.
:::

```yaml
komga_api:
  enabled: true
  prefix: "komga"  # URL prefix (default: komga)
```

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Enable Komga-compatible API endpoints |
| `prefix` | `komga` | URL prefix for Komga API (results in `/{prefix}/api/v1/...`) |

When enabled, the Komga API will be available at:
```
http://your-server:8080/komga/api/v1/libraries
http://your-server:8080/komga/api/v1/series
http://your-server:8080/komga/api/v1/books
...
```

### Configuring Third-Party Apps

To connect apps like Komic to Codex:

1. **Server URL**: `http://your-server:8080/komga`
2. **Authentication**: Use your Codex username and password (Basic Auth)

:::tip Custom Prefix
You can change the URL prefix to avoid conflicts or for preference. For example, setting `prefix: "compat"` would make the API available at `/compat/api/v1/...`.
:::

### Supported Features

- Library browsing
- Series and book navigation
- Thumbnail display
- Page streaming for reading
- Reading progress sync
- Book file downloads
- Next/previous book navigation

### Limitations

- **Read-only**: Metadata editing through the Komga API is not supported
- **No collections/read lists**: These Komga features are not implemented
- **Basic search only**: Full Komga search syntax is not supported
- **No oneshot detection**: All items return `oneshot: false`

For more details, see the [Third-Party Apps documentation](./third-party-apps).

## Rate Limiting

Codex includes built-in API rate limiting to protect against abuse. Rate limiting is **enabled by default** and uses a token bucket algorithm with per-client tracking.

```yaml
rate_limit:
  enabled: true
  anonymous_rps: 10           # Requests per second for anonymous users
  anonymous_burst: 50         # Maximum burst size for anonymous users
  authenticated_rps: 50       # Requests per second for authenticated users
  authenticated_burst: 200    # Maximum burst size for authenticated users
  exempt_paths:               # Paths exempt from rate limiting
    - /health
    - /api/v1/events
  cleanup_interval_secs: 60   # How often to clean stale buckets
  bucket_ttl_secs: 300        # Time before a bucket is considered stale
```

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `true` | Enable/disable rate limiting |
| `anonymous_rps` | `10` | Requests per second for anonymous users |
| `anonymous_burst` | `50` | Maximum burst size for anonymous users |
| `authenticated_rps` | `50` | Requests per second for authenticated users |
| `authenticated_burst` | `200` | Maximum burst size for authenticated users |
| `exempt_paths` | `["/health", "/api/v1/events"]` | Paths exempt from rate limiting |
| `cleanup_interval_secs` | `60` | How often to clean up stale client buckets |
| `bucket_ttl_secs` | `300` | Time in seconds before a bucket is considered stale |

### How It Works

Rate limiting uses a **token bucket** algorithm:

1. Each client (identified by IP address or user ID) has a bucket of tokens
2. Tokens are added at the configured rate (e.g., 10/second for anonymous)
3. Each request consumes one token
4. If no tokens are available, the request is rejected with HTTP 429
5. The bucket can hold up to the burst limit, allowing temporary spikes

### Response Headers

All API responses include rate limit information:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests allowed |
| `X-RateLimit-Remaining` | Requests remaining in current window |
| `X-RateLimit-Reset` | Unix timestamp when limit resets |

### 429 Too Many Requests

When rate limited, the API returns:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 30
X-RateLimit-Limit: 50
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1706140800
Content-Type: application/json

{
  "error": "rate_limit_exceeded",
  "message": "Too many requests. Please retry after 30 seconds.",
  "retry_after": 30
}
```

### Disabling Rate Limiting

To disable rate limiting (not recommended for production):

```yaml
rate_limit:
  enabled: false
```

Or via environment variable:

```bash
CODEX_RATE_LIMIT_ENABLED=false
```

:::caution
Disabling rate limiting may expose your server to abuse. Only disable for trusted networks or development environments.
:::

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

# PDF Rendering
# CODEX_PDF_PDFIUM_LIBRARY_PATH=/usr/local/lib/libpdfium.so  # Optional, auto-detected
CODEX_PDF_RENDER_DPI=150
CODEX_PDF_JPEG_QUALITY=85
CODEX_PDF_CACHE_RENDERED_PAGES=true
CODEX_PDF_CACHE_DIR=data/cache

# Komga-Compatible API
CODEX_KOMGA_API_ENABLED=true
CODEX_KOMGA_API_PREFIX=komga

# Rate Limiting
CODEX_RATE_LIMIT_ENABLED=true
CODEX_RATE_LIMIT_ANONYMOUS_RPS=10
CODEX_RATE_LIMIT_ANONYMOUS_BURST=50
CODEX_RATE_LIMIT_AUTHENTICATED_RPS=50
CODEX_RATE_LIMIT_AUTHENTICATED_BURST=200
CODEX_RATE_LIMIT_EXEMPT_PATHS=/health,/api/v1/events
CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS=60
CODEX_RATE_LIMIT_BUCKET_TTL_SECS=300
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
- PDF rendering settings (DPI, cache directory, PDFium library path)
- Rate limiting settings

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
