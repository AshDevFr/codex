---
---

# Configuration

Codex uses YAML configuration files with environment variable overrides. This guide covers all configuration options and best practices.

## Configuration File

Every subcommand reads `config/codex.yaml` unless `--config` points somewhere
else:

```bash
codex serve                              # config/codex.yaml
codex serve --config /etc/codex/codex.yaml
```

The file may be YAML or TOML; the format is chosen from the extension, and
anything that is not `.toml` is read as YAML.

A missing file is not an error. Defaults plus environment variables are a
complete configuration on their own, which is what a container with no mounted
file relies on.

Codex does **not** write a config file at startup. To get a commented starter:

```bash
codex config init                        # writes config/codex.yaml
codex config init -c /etc/codex/codex.yaml
codex config init --force                # replace an existing file
```

### Local overlay

A sibling `<stem>.local.<ext>` file is merged on top of the base file when it
exists: `codex.yaml` picks up `codex.local.yaml`, `codex.toml` picks up
`codex.local.toml`. Use it to pin secrets and per-host tweaks without editing
the file you commit.

The merge is key by key, so the overlay only needs the keys it changes. One
exception: a list in the overlay **replaces** the base list rather than
extending it.

```yaml
# config/codex.local.yaml
auth:
  jwt_secret: "the-real-secret"
database:
  postgres:
    password: "the-real-password"
```

:::note No variable interpolation
Config files are read literally. `${JWT_SECRET}` in a YAML value is the
literal string `${JWT_SECRET}`, not the environment variable. To pull a
value from the environment, set the matching `CODEX_*` variable (see
[Environment Variables](#environment-variables)) or use a local overlay
rendered by your deployment tooling.
:::

## Configuration Priority

Settings are layered, with later sources overriding earlier ones:

1. **Built-in defaults**
2. **Configuration file** (`config/codex.yaml` or `--config`)
3. **Local overlay** (`config/codex.local.yaml`)
4. **Environment variables** (`CODEX_*`, highest priority)

A handful of settings live in the database instead and are changed through the
admin UI or the settings API without a restart. See [Runtime vs Startup
Settings](#runtime-vs-startup-settings).

## Database Configuration

Codex supports both SQLite and PostgreSQL databases.

### Migration control

```yaml
database:
  run_migrations: true              # Apply pending migrations at startup
  migration_wait_timeout_secs: 300  # When false, how long to wait for them
  migration_wait_interval_secs: 2   # How often to re-check
```

| Setting | Default | Description |
|---------|---------|-------------|
| `run_migrations` | `true` | Apply pending migrations when the process starts |
| `migration_wait_timeout_secs` | `300` | With `run_migrations: false`, how long to wait for the schema to become current before giving up |
| `migration_wait_interval_secs` | `2` | Poll interval while waiting |

A single-process deployment can leave these alone. Run more than one process
against the same database and they race on a version bump: one applies a
migration, the rest abort on the object it just created. Set
`run_migrations: false` on every `serve` and `worker` process, and apply the
schema once from a dedicated `codex migrate` job. `codex migrate` applies
migrations unconditionally and ignores this setting, so it can share the same
environment.

:::warning Renamed in Codex 2.0
This replaces `CODEX_SKIP_MIGRATIONS`, with the **opposite** sense:
`SKIP_MIGRATIONS=true` becomes `CODEX_DATABASE__RUN_MIGRATIONS=false`. The old
name is no longer read. See the [upgrade guide](./migration/v2-config.md).
:::

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
    max_connections: 64        # Maximum pool size (default: 64)
    min_connections: 2         # Minimum warm connections (default: 2)
    acquire_timeout_seconds: 30  # Wait time for connection (default: 30)
    idle_timeout_seconds: 300    # Idle connection timeout (default: 300 = 5 min)
    max_lifetime_seconds: 1800   # Max connection lifetime (default: 1800 = 30 min)
    batch_fan_out: 4           # Per-request query fan-out bound (default: 4)
    background_max_connections: 4  # Background-work pool size (default: 4)
```

| Setting | Default | Description |
|---------|---------|-------------|
| `max_connections` | `64` | Maximum connections in pool |
| `min_connections` | `2` | Minimum warm connections |
| `acquire_timeout_seconds` | `30` | How long to wait for a connection |
| `idle_timeout_seconds` | `300` | Idle connection timeout (5 min) |
| `max_lifetime_seconds` | `1800` | Maximum connection lifetime (30 min) |
| `batch_fan_out` | `4` | Max related-table queries one request runs at once |
| `background_max_connections` | `4` | Connections for the in-process background pool |
| `operation_deadline_seconds` | `30` | Maximum time one operation may hold a connection before timing out |

:::tip SQLite Pool Sizing
SQLite with WAL mode handles concurrent reads well, but writes are serialized. Connections are cheap file handles under WAL, so the default of 64 gives headroom for many concurrent readers (e.g. multiple browser tabs).

`batch_fan_out` caps how many related-table queries a single list/detail request runs concurrently, so a few simultaneous requests cannot each grab a connection per query and exhaust the pool. `background_max_connections` gives in-process task workers, the scheduler, and pollers a **separate** pool, so a scan or analysis burst cannot starve interactive API requests. Increase `max_connections` if you still see "connection pool timeout" errors under heavy load.
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
    # Connection pool settings
    max_connections: 25        # Maximum pool size (default: 25)
    min_connections: 2         # Minimum warm connections (default: 2)
    acquire_timeout_seconds: 30  # Wait time for connection (default: 30)
    idle_timeout_seconds: 600    # Idle connection timeout (default: 600 = 10 min)
    max_lifetime_seconds: 3600   # Max connection lifetime (default: 3600 = 1 hour)
    batch_fan_out: 8           # Per-request query fan-out bound (default: 8)
    background_max_connections: 16  # Background-work pool size (default: 16)
    operation_deadline_seconds: 30  # Max time one operation may hold a connection
```

:::warning PostgreSQL connection budget
These pool sizes are **per process**, while PostgreSQL's own `max_connections` (100 by default, a few of them reserved for the superuser) is a limit for the whole server. Every replica, worker, migration Job and backup CronJob opens its own pool, so the budget to check is:

```
(web replicas x max_connections) + (workers x max_connections) + jobs  <=  server max_connections - superuser_reserved_connections
```

`background_max_connections` is **additive** to `max_connections` whenever task workers run in the same process as the web server. Multi-pod deployments run the web server with `CODEX_TASK__RUN_IN_PROCESS=false`, so no background pool is created there.

Overrunning the budget does not degrade gracefully: the server refuses new connections with `FATAL: remaining connection slots are reserved`, and because a starting process needs a connection before it can do anything, **new pods fail to start while the running ones carry on looking healthy**. Codex logs a warning at startup when its pools do not fit, including when the connections already open on the server leave no room.
:::

#### PostgreSQL TLS

TLS is configured under `database.postgres`:

```yaml
database:
  postgres:
    ssl_mode: verify-full
    ssl_root_cert: /etc/ssl/certs/postgres-ca.crt
    # ssl_client_cert / ssl_client_key for mutual TLS
```

| Setting | Purpose |
|---------|---------|
| `ssl_mode` | Negotiation mode (see the table below). Unset leaves the driver default, `prefer`. |
| `ssl_root_cert` | CA certificate used to verify the server. |
| `ssl_client_cert`, `ssl_client_key` | Client certificate and key, for mutual TLS. |

| `ssl_mode` | Encrypted | Certificate verified | Hostname verified |
|-------------|-----------|----------------------|-------------------|
| `disable` | no | no | no |
| `allow` | only if the server requires it | no | no |
| `prefer` *(driver default)* | only if the server offers it | **no** | **no** |
| `require` | yes | no | no |
| `verify-ca` | yes | yes | no |
| `verify-full` | yes | yes | yes |

:::warning Leaving `ssl_mode` unset guards against eavesdropping, not interception
The driver default is `prefer`: it encrypts when the server offers TLS, but it
accepts **any** certificate and **silently falls back to an unencrypted
connection** when the server offers none. Neither the fallback nor a bogus
certificate is logged, so a downgrade looks exactly like a healthy start.

That is adequate on a private network you control, and not much else. Set
`ssl_mode: verify-full` for anything remote or managed.
:::

:::note The libpq variables still work
`PGSSLMODE`, `PGSSLROOTCERT`, `PGSSLCERT` and `PGSSLKEY` are read by the driver
when the corresponding Codex setting is unset, so a deployment configured that
way keeps working. The Codex setting wins when both are present. Being driver
variables they carry no `CODEX_` prefix and do not appear in `codex config
check`.
:::

## Data Directory

```yaml
data_dir: data
```

| Setting | Default | Description |
|---------|---------|-------------|
| `data_dir` | `data` | Base directory for Codex's own state |

`database.sqlite.path`, `files.thumbnail_dir`, `files.uploads_dir`,
`files.plugins_dir` and `pdf.cache_dir` default to subdirectories of
`data_dir`, so pointing `data_dir` at a mounted volume moves all of them at
once. Setting any of those keys yourself wins and is used exactly as written.

## Application Configuration

```yaml
application:
  host: 0.0.0.0         # Bind address (0.0.0.0 for all interfaces)
  port: 8080            # Server port
  base_url: https://codex.example.com  # Public-facing URL (optional)
```

| Setting | Default | Description |
|---------|---------|-------------|
| `host` | `0.0.0.0` | Bind address |
| `port` | `8080` | HTTP port |
| `base_url` | *(none)* | Public-facing URL (e.g., `https://codex.example.com`). Used as fallback for OIDC redirect URIs and email verification links. If not set, falls back to `http://{host}:{port}`. |

:::note The server display name is not a config key
The name shown in the UI is a database setting, changed through the admin
settings UI or `/api/v1/admin/settings`. An `application.name` key in the
config file is not a setting and is ignored.
:::

## Authentication Configuration

```yaml
auth:
  jwt_secret: "CHANGE_ME_IN_PRODUCTION"
  jwt_expiry_hours: 24
  refresh_token_enabled: true
  refresh_token_expiry_days: 30
  email_confirmation_required: false
  argon2_memory_cost: 19456
  argon2_time_cost: 2
  argon2_parallelism: 1
  # cookie_secure: true   # Send `Secure` on auth cookies (set this behind TLS)
```

| Setting | Default | Description |
|---------|---------|-------------|
| `jwt_secret` | Required | Secret key for JWT signing |
| `jwt_expiry_hours` | `24` | Access token validity period |
| `refresh_token_enabled` | `true` | Issue refresh tokens on login and accept them at `POST /api/v1/auth/refresh` |
| `refresh_token_expiry_days` | `30` | Refresh token lifetime, in days |
| `email_confirmation_required` | `false` | Require email verification |
| `argon2_memory_cost` | `19456` | Argon2 memory cost (KiB) |
| `argon2_time_cost` | `2` | Argon2 iterations |
| `argon2_parallelism` | `1` | Argon2 parallelism |
| `cookie_secure` | `false` | Send the `Secure` attribute on auth cookies. Off by default so plain-HTTP development works; **set it to `true` on any deployment that terminates TLS**, or the session cookie is also sent over a plaintext downgrade. |

:::danger JWT Secret
**Always change the JWT secret in production!** Generate a secure random string:

```bash
openssl rand -base64 32
```
:::

### Refresh Tokens

When `refresh_token_enabled` is `true` (default), both password login (`POST /api/v1/auth/login`) and OIDC single sign-on return a `refreshToken` alongside the access token. The frontend transparently exchanges an expired access token for a fresh pair on the next API call via `POST /api/v1/auth/refresh`, so users are not bounced to the login screen mid-session, regardless of how they signed in.

- **Rotation:** every refresh issues a new refresh token and revokes the old one atomically.
- **Theft detection:** replaying an already-rotated (revoked) refresh token revokes every refresh token in that login's family, forcing all sessions for that login to re-authenticate. This matches the OAuth 2.0 security recommendations (RFC 6819).
- **Storage at rest:** refresh tokens are stored as `sha256` hashes in the `refresh_tokens` table. Compromise of the database does not yield usable tokens.
- **Cleanup:** a daily background task prunes expired tokens and revoked tokens older than 30 days.
- **Logout:** `POST /api/v1/auth/logout` accepts `{ "refreshToken": "..." }` and revokes that specific token server-side.

Disable the feature by setting `refresh_token_enabled: false`. The login response then omits `refreshToken` and `/auth/refresh` returns `401`. Clients fall back to the legacy "log in again at access-token expiry" behavior.

:::note Storage on the client
The web client currently stores the refresh token in `localStorage` alongside the access token. The XSS posture is identical to today's access-token-only storage. Migration to an httpOnly cookie is tracked as a separate hardening ticket.
:::

### OIDC (Single Sign-On)

Enable OpenID Connect authentication to allow users to sign in via external identity providers:

```yaml
auth:
  oidc:
    enabled: true
    auto_create_users: true
    default_role: reader
    providers:
      authentik:
        display_name: "Authentik"
        issuer_url: "https://authentik.example.com/application/o/codex/"
        client_id: "codex-client-id"
        client_secret: "codex-client-secret"
        scopes:
          - email
          - profile
          - groups
        role_mapping:
          admin:
            - codex-admins
          maintainer:
            - codex-editors
          reader:
            - codex-users
        groups_claim: "groups"
```

| Setting | Default | Description |
|---------|---------|-------------|
| `oidc.enabled` | `false` | Enable OIDC authentication |
| `oidc.auto_create_users` | `true` | Create users on first OIDC login |
| `oidc.default_role` | `reader` | Default role when no groups match |
| `oidc.redirect_uri_base` | auto-detected | Override base URL for OAuth callbacks. Falls back to `application.base_url`. |
| `oidc.allowed_redirect_uris` | `[]` | Exact post-login redirect targets to accept (e.g. `codexreader://auth`), for native and desktop clients. Compared as whole strings, and an empty list permits none. |
| `oidc.providers` | `{}` | Provider blocks, keyed by the name used in the callback URL. |

Sign-in spans two requests: Codex builds an authorization URL, the identity
provider sends the browser back to the callback. The state that links them is kept
in the database, so the replica handling the callback need not be the one that
started the flow and no session affinity is required. Abandoned sign-ins are swept
every 15 minutes; they expire after 5 minutes either way.

See [OIDC / Single Sign-On](./users/oidc) for full setup instructions and provider guides.

## API Configuration

```yaml
api:
  base_path: /api/v1
  enable_api_docs: false
  api_docs_path: "/docs"
  cors_enabled: true
  cors_origins:
    - "*"
  max_page_size: 100
```

| Setting | Default | Description |
|---------|---------|-------------|
| `base_path` | `/api/v1` | Path prefix reported for the native API. Informational: the routes are mounted at `/api/v1` regardless. |
| `enable_api_docs` | `false` | Enable API documentation (Scalar) |
| `api_docs_path` | `/docs` | API documentation URL path |
| `cors_enabled` | `true` | Enable CORS |
| `cors_origins` | `["*"]` | Allowed origins when CORS is enabled. Narrow this in production. |
| `max_page_size` | `100` | Maximum items per page |

## Logging Configuration

```yaml
logging:
  level: info
  console: true
  # file: ./logs/codex.log  # Uncomment to enable file logging
```

| Setting | Default | Description |
|---------|---------|-------------|
| `level` | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `console` | `true` | Write logs to stdout. Set to `false` when only the log file matters. |
| `file` | None | Optional log file path |

## Task Worker Configuration

These settings require a restart to take effect.

```yaml
task:
  run_in_process: true
  worker_count: 2
```

| Setting | Default | Description |
|---------|---------|-------------|
| `run_in_process` | `true` | Run background task workers inside this process |
| `worker_count` | `2` | Number of parallel background workers |

:::tip Splitting web from workers
A multi-pod deployment runs the web server with `run_in_process: false` (or
`CODEX_TASK__RUN_IN_PROCESS=false`) and puts the workers in their own
`codex worker` pods, so a scan burst cannot slow down interactive requests.
The scheduler still runs in every `serve` process regardless of this setting.

This replaces the `CODEX_DISABLE_WORKERS` variable from 1.x, with the **opposite**
sense. See the [2.0 upgrade guide](./migration/v2-config.md).
:::

## Scanner Configuration

These settings require a restart to take effect.

```yaml
scanner:
  max_concurrent_scans: 2
```

| Setting | Default | Description |
|---------|---------|-------------|
| `max_concurrent_scans` | `2` | Maximum concurrent library scans |

## Image Decoding

```yaml
images:
  decode_concurrency: 3
```

| Setting | Default | Description |
|---------|---------|-------------|
| `decode_concurrency` | `3` | Maximum image decodes running at once |

Decoding is CPU- and memory-hungry, so this caps how many pages or covers are
decoded in parallel across the whole process. Raise it on a machine with cores
to spare; lower it if thumbnail generation is crowding out request handling.

## Scheduler Configuration

Controls the job scheduler that runs cron-based tasks (library scans, deduplication, thumbnail generation, etc.).

```yaml
scheduler:
  timezone: "America/Los_Angeles"
```

| Setting | Default | Env Override | Description |
|---------|---------|--------------|-------------|
| `timezone` | `UTC` | `CODEX_SCHEDULER__TIMEZONE` | Default IANA timezone for all cron schedules |

The timezone must be a valid IANA timezone name (e.g., `America/New_York`, `Europe/London`, `Asia/Tokyo`). Abbreviations like `PST` or offsets like `UTC+8` are **not** supported.

:::tip Per-Library Timezone Override
Each library can override the server-level timezone via the `cronTimezone` field in its scanning configuration. This is useful when different libraries should run scans at different local times.

Priority: **Library `cronTimezone`** > **Server `scheduler.timezone`** > **UTC**
:::

:::note Docker Users
The Docker `TZ` environment variable does **not** affect the cron scheduler. You must set `CODEX_SCHEDULER__TIMEZONE` (or configure `scheduler.timezone` in your YAML) for cron jobs to run in your local timezone.
:::

## Files Configuration

Configuration for file storage directories (thumbnails and uploads).

```yaml
files:
  thumbnail_dir: data/thumbnails
  uploads_dir: data/uploads
  plugins_dir: data/plugins
```

| Setting | Default | Description |
|---------|---------|-------------|
| `thumbnail_dir` | `{data_dir}/thumbnails` | Directory for thumbnail cache |
| `uploads_dir` | `{data_dir}/uploads` | Directory for user-uploaded files (covers, etc.) |
| `plugins_dir` | `{data_dir}/plugins` | Directory installed plugins are unpacked into |

Additional thumbnail settings are stored in the database and can be changed via the Settings API without restart:
- `thumbnail_max_dimension` - Maximum width/height (default: 400px)
- `thumbnail_jpeg_quality` - JPEG quality (default: 85%)

## Plugins Configuration

```yaml
plugins:
  allowed_commands:
    - deno
```

| Setting | Default | Description |
|---------|---------|-------------|
| `allowed_commands` | `[]` | Extra commands a plugin may be launched with, on top of the built-in allowlist |

Plugin processes are only started via an allowlisted command, so a compromised
admin account cannot turn plugin installation into arbitrary command execution.
`node`, `npx`, `python`, `python3`, `uv` and `uvx` are always allowed, as are
absolute paths under the plugins directory. `allowed_commands` adds to that
list; it does not replace it.

Set it in the config file, or as
`CODEX_PLUGINS__ALLOWED_COMMANDS='[deno, bun]'`. In 1.x this was
`CODEX_PLUGIN_ALLOWED_COMMANDS` (singular, comma-separated), which is no longer
read.

### Plugin log level

Plugins run with the log level the host is using. It comes from
[`logging.level`](#logging-configuration), and `trace` is delivered to plugins
as `debug` (the plugin SDK logger has no `trace` level).

The host sends the level to every plugin in the `initialize` message; the
plugin SDK applies it to its own logger and exposes it so each plugin can adopt
it for its own logging. Plugins honor it on a best-effort basis.

:::tip
Set `logging.level: debug` (or `CODEX_LOGGING__LEVEL=debug`) when debugging a
misbehaving plugin to surface its diagnostic logging, then revert to `info` to
keep logs quiet. Note that this makes the host verbose too.
:::

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
  # verification_url_base: https://codex.example.com  # Falls back to application.base_url
```

:::tip
If you've set `application.base_url`, you don't need to set `verification_url_base` separately — it will automatically use the application base URL for email verification links.
:::

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
3. Or set `CODEX_PDF__PDFIUM_LIBRARY_PATH` to the full path of `pdfium.dll`

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

## PDF Handle Cache

Separate from the on-disk page cache above, Codex keeps an in-memory cache of *open* PDF document handles. Opening a PDF (especially a large one) is significantly more expensive than rendering a single page from an already-open handle, so reusing handles across requests is the primary way the streaming reader stays responsive on big files.

```yaml
pdf_handle_cache:
  enabled: true              # Master switch (when false, every render re-opens the PDF)
  capacity: 256              # Max number of resident open handles
  idle_ttl_minutes: 15       # Drop a handle after this many minutes of inactivity
  sweep_interval_seconds: 60 # How often the background sweeper enforces idle_ttl_minutes
```

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `true` | Master switch. When `false`, every page render re-opens the PDF from disk |
| `capacity` | `256` | Hard cap on resident open handles. Memory footprint is roughly `capacity × per-PDF-open-cost` (typically 5–15 MB per book) |
| `idle_ttl_minutes` | `15` | An entry is dropped if it hasn't been accessed for this many minutes |
| `sweep_interval_seconds` | `60` | How often the background sweeper walks the cache to apply `idle_ttl_minutes`. Lower values catch idle handles sooner; higher values reduce wakeups |

The handle cache is independent from the disk page cache (`pdf.cache_*`): the disk cache stores rendered JPEGs, while the handle cache stores open `PdfDocument` objects. Cached handles are automatically evicted when a book is updated (rescanned, edited) or deleted, so stale handles are not served.

Admin endpoints under `/api/v1/admin/pdf-cache` expose handle-cache stats and a manual clear operation.

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
- **No oneshot detection**: The `oneshot` field is always omitted from responses

For more details, see the [Third-Party Apps documentation](./third-party-apps).

## KOReader Sync API (Optional)

Codex can serve the KOReader progress-sync endpoints, so a KOReader device
syncs reading position against Codex instead of a separate sync server.

```yaml
koreader_api:
  enabled: true
```

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Expose the KOReader sync endpoints |

See [Third-Party Apps](./third-party-apps) for device-side setup.

## Rate Limiting

Codex includes built-in API rate limiting to protect against abuse. Rate limiting is **enabled by default** and uses a token bucket algorithm with per-client tracking.

```yaml
rate_limit:
  enabled: true
  anonymous_rps: 10           # Requests per second for anonymous users
  anonymous_burst: 50         # Maximum burst size for anonymous users
  authenticated_rps: 50       # Requests per second for authenticated users
  authenticated_burst: 200    # Maximum burst size for authenticated users
  exempt_paths:               # Glob patterns for paths exempt from rate limiting
    - /health
    - /api/v1/events
    - /api/v1/events/**
    - /api/v1/books/*/thumbnail   # Exempt book thumbnails
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
| `exempt_paths` | `["/health", "/api/v1/events", "/api/v1/events/**"]` | Glob patterns for paths exempt from rate limiting |
| `cleanup_interval_secs` | `60` | How often to clean up stale client buckets |
| `bucket_ttl_secs` | `300` | Time in seconds before a bucket is considered stale |

Exempt paths support glob patterns:
- `*` matches a single path segment (e.g., `/api/v1/books/*/thumbnail` matches `/api/v1/books/123/thumbnail`)
- `**` matches zero or more path segments (e.g., `/api/v1/events/**` matches `/api/v1/events/stream`)
- Exact paths match only themselves (e.g., `/health` matches only `/health`, not `/health/check`)

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
CODEX_RATE_LIMIT__ENABLED=false
```

:::caution
Disabling rate limiting may expose your server to abuse. Only disable for trusted networks or development environments.
:::

## Plugin Credential Encryption

Codex encrypts sensitive plugin data at rest — including OAuth tokens, refresh tokens, and plugin credentials — using **AES-256-GCM** authenticated encryption. This section covers setting up and managing the encryption key.

### Setting Up the Encryption Key

The encryption key is provided via the `CODEX_ENCRYPTION_KEY` environment variable. It must be a **base64-encoded 32-byte (256-bit) key**.

Generate a key using OpenSSL:

```bash
openssl rand -base64 32
```

Then set it as an environment variable:

```bash
export CODEX_ENCRYPTION_KEY="your-generated-base64-key-here"
```

Or in a Docker Compose file:

```yaml
environment:
  CODEX_ENCRYPTION_KEY: "your-generated-base64-key-here"
```

:::danger Required for Plugins
The encryption key is **required** when using sync or recommendation plugins that store OAuth tokens. Without it, plugin connection attempts will fail with a "Service Unavailable" error. Metadata-only plugins (like Open Library) do not require an encryption key.
:::

### What the Key Protects

| Data | When Encrypted |
|------|----------------|
| OAuth access tokens | When a user connects a sync/recommendation plugin |
| OAuth refresh tokens | When the external service issues a refresh token |
| Plugin credentials | When a plugin stores API keys or secrets |

All encrypted values use a random 96-bit nonce, so encrypting the same token twice produces different ciphertext. Decryption requires the exact same key that was used for encryption.

### Key Requirements

- **Length**: Exactly 32 bytes (256 bits) before base64 encoding
- **Encoding**: Standard base64 (RFC 4648)
- **Persistence**: Must remain the same across Codex restarts — changing the key without re-encrypting data will make existing tokens undecryptable

### Key Rotation

Codex does not currently support automatic key rotation. If you need to rotate the encryption key, follow this manual procedure:

1. **Stop Codex** — ensure no requests are in flight
2. **Have all users disconnect their plugins** — go to **Settings > Integrations** and click **Disconnect** on each plugin connection. This deletes the encrypted tokens from the database
3. **Update the encryption key** — set `CODEX_ENCRYPTION_KEY` to the new key
4. **Start Codex**
5. **Have users reconnect their plugins** — each user re-authorizes via OAuth, and new tokens are encrypted with the new key

:::tip Simpler Alternative
Since disconnecting and reconnecting plugins re-issues fresh OAuth tokens encrypted with the current key, this is the simplest and safest rotation method. No data migration or scripting is required.
:::

:::caution Lost Key
If you lose the encryption key, all stored OAuth tokens become undecryptable. Users will need to disconnect and reconnect their plugins to issue new tokens. No plugin configuration or storage data is lost — only the encrypted credentials.
:::

### Future Enhancement

Automatic key rotation with key versioning (storing the key version alongside encrypted data for seamless re-encryption) is planned for a future release.

## Observability Configuration

Codex emits OpenTelemetry traces and metrics over OTLP, plus optional browser RUM proxied through the server. Everything is **disabled by default**; nothing is exported until an operator opts in.

For the full guide (architecture, sampling guidance, backend matrix, troubleshooting), see the [Observability page](./observability).

```yaml
observability:
  enabled: false                        # master switch; must be true for any export to happen
  service_name: codex                   # `service.name` resource attribute
  otlp:
    endpoint: ""                        # e.g. http://localhost:4317 (gRPC) or http://localhost:4318 (HTTP)
    protocol: grpc                      # grpc | http/protobuf | http/json
    headers: {}                         # auth/tenant headers (e.g. signoz-access-token: ...)
    timeout_ms: 5000
  traces:
    enabled: true                       # honored only when observability.enabled is also true
    sample_ratio: 1.0                   # parent-based sampler ratio in [0.0, 1.0]
  metrics:
    enabled: true
    export_interval_ms: 30000           # periodic reader interval
  browser:
    enabled: false                      # opt-in separately; enables the OTLP proxy + ships SDK config
    proxy_path: /api/v1/observability/otlp
    sample_ratio: 0.1                   # browsers are noisy; sample lower than backend by default
```

### Top-level settings

| Setting | Default | Env Override | Description |
|---------|---------|--------------|-------------|
| `enabled` | `false` | `CODEX_OBSERVABILITY__ENABLED` | Master switch. No providers are initialized when `false`. |
| `service_name` | `codex` | `CODEX_OBSERVABILITY__SERVICE_NAME` | Resource attribute that identifies this process in the backend UI. |

### OTLP exporter (`observability.otlp`)

| Setting | Default | Env Override | Description |
|---------|---------|--------------|-------------|
| `endpoint` | `""` | `CODEX_OBSERVABILITY__OTLP__ENDPOINT` | Collector URL. Required when `enabled: true`. |
| `protocol` | `grpc` | `CODEX_OBSERVABILITY__OTLP__PROTOCOL` | One of `grpc`, `http/protobuf`, `http/json`. |
| `headers` | `{}` | `CODEX_OBSERVABILITY__OTLP__HEADERS` | Map of arbitrary headers. Env format: `'{k1=v1, k2=v2}'`; quote any value containing a space or comma. |
| `timeout_ms` | `5000` | `CODEX_OBSERVABILITY__OTLP__TIMEOUT_MS` | Per-export request timeout. |

:::tip Endpoint format
For gRPC endpoints, include the scheme: `http://host:4317` (cleartext) or `https://host:4317` (TLS).
For HTTP endpoints, point at the base URL only: `http://collector:4318`. The SDK appends `/v1/traces` and `/v1/metrics` per signal.
:::

### Traces (`observability.traces`)

| Setting | Default | Env Override | Description |
|---------|---------|--------------|-------------|
| `enabled` | `true` | `CODEX_OBSERVABILITY__TRACES__ENABLED` | Per-signal switch. Honored only when the parent `enabled` is also true. |
| `sample_ratio` | `1.0` | `CODEX_OBSERVABILITY__TRACES__SAMPLE_RATIO` | Parent-based sampler ratio in `[0.0, 1.0]`. Out-of-range values are clamped. |

See the [sampling guidance table](./observability#sampling-guidance) for production-sized recommendations.

### Metrics (`observability.metrics`)

| Setting | Default | Env Override | Description |
|---------|---------|--------------|-------------|
| `enabled` | `true` | `CODEX_OBSERVABILITY__METRICS__ENABLED` | Per-signal switch. Honored only when the parent `enabled` is also true. |
| `export_interval_ms` | `30000` | `CODEX_OBSERVABILITY__METRICS__EXPORT_INTERVAL_MS` | Periodic reader export interval. Lower values increase load on the collector. |

### Browser RUM (`observability.browser`)

| Setting | Default | Env Override | Description |
|---------|---------|--------------|-------------|
| `enabled` | `false` | `CODEX_OBSERVABILITY__BROWSER__ENABLED` | Opt-in switch for the OTLP proxy and the SPA's SDK bootstrap. |
| `proxy_path` | `/api/v1/observability/otlp` | `CODEX_OBSERVABILITY__BROWSER__PROXY_PATH` | Path on the Codex server where the browser SDK POSTs OTLP batches. |
| `sample_ratio` | `0.1` | `CODEX_OBSERVABILITY__BROWSER__SAMPLE_RATIO` | Client-side sample ratio. |

:::note Two independent switches
`observability.browser.enabled` is intentionally independent from the backend `observability.enabled` flag. Some operators want server-side observability without shipping spans from every browser tab. The SDK additionally refuses to start if `observability.otlp.endpoint` is empty, so a misconfigured server cannot leak data via the browser.
:::

## Environment Variables

All configuration options can be overridden with environment variables using the `CODEX_` prefix.

### Naming Convention

Configuration paths are converted to environment variables:
- Use uppercase
- Separate nesting levels with `__` (a single `_` still separates words inside one key)
- Prefix with `CODEX_`

Values are typed. Booleans are `true`/`false`, lists are `[a, b]`, and maps are
`{key=value, key=value}`; quote any entry containing a space or a comma. An
empty value means "unset". Anything that does not parse stops the server rather
than being silently discarded.

:::info Upgrading from 1.x
These names changed in Codex 2.0, and the old flat spelling is no longer read.
Codex refuses to start when it sees one, listing each with its replacement. See
the [upgrade guide](./migration/v2-config.md).
:::

| Config Path | Environment Variable |
|-------------|---------------------|
| `database.db_type` | `CODEX_DATABASE__DB_TYPE` |
| `database.postgres.host` | `CODEX_DATABASE__POSTGRES__HOST` |
| `auth.jwt_secret` | `CODEX_AUTH__JWT_SECRET` |
| `logging.level` | `CODEX_LOGGING__LEVEL` |
| `scheduler.timezone` | `CODEX_SCHEDULER__TIMEZONE` |
| `pdf_handle_cache.capacity` | `CODEX_PDF_HANDLE_CACHE__CAPACITY` |
| `auth.oidc.providers.authentik.issuer_url` | `CODEX_AUTH__OIDC__PROVIDERS__AUTHENTIK__ISSUER_URL` |

Note the last two: `pdf_handle_cache` and `issuer_url` are single keys whose
names contain `_`, so only the level boundaries double up.

A variable that does not name a real setting is ignored, which is silent and
hard to spot. Run [`codex config check`](#checking-your-configuration) to have
every `CODEX_*` variable classified against the real key list.

### Common Environment Variables

```bash
# Database
CODEX_DATABASE__DB_TYPE=postgres
CODEX_DATABASE__POSTGRES__HOST=localhost
CODEX_DATABASE__POSTGRES__PORT=5432
CODEX_DATABASE__POSTGRES__USERNAME=codex
CODEX_DATABASE__POSTGRES__PASSWORD=secret
CODEX_DATABASE__POSTGRES__DATABASE_NAME=codex

CODEX_DATABASE__POSTGRES__SSL_MODE=verify-full
CODEX_DATABASE__POSTGRES__SSL_ROOT_CERT=/etc/ssl/certs/postgres-ca.crt

# Migrations (false = wait for a separate `codex migrate` job)
CODEX_DATABASE__RUN_MIGRATIONS=false
CODEX_DATABASE__MIGRATION_WAIT_TIMEOUT_SECS=300
CODEX_DATABASE__MIGRATION_WAIT_INTERVAL_SECS=2

# Data directory (thumbnails, uploads, plugins, caches default under it)
CODEX_DATA_DIR=/var/lib/codex

# Application
CODEX_APPLICATION__HOST=0.0.0.0
CODEX_APPLICATION__PORT=8080
CODEX_APPLICATION__BASE_URL=https://library.example.com

# Authentication
CODEX_AUTH__JWT_SECRET=your-secure-secret-key
CODEX_AUTH__COOKIE_SECURE=true

# Logging
CODEX_LOGGING__LEVEL=debug
CODEX_LOGGING__CONSOLE=true
CODEX_LOGGING__FILE=/var/log/codex/codex.log

# API
CODEX_API__ENABLE_API_DOCS=true
CODEX_API__CORS_ENABLED=true
CODEX_API__CORS_ORIGINS='[https://library.example.com]'

# Task Workers (RUN_IN_PROCESS=false for a web pod with separate workers)
CODEX_TASK__RUN_IN_PROCESS=true
CODEX_TASK__WORKER_COUNT=4

# Scanner
CODEX_SCANNER__MAX_CONCURRENT_SCANS=2

# Image decoding
CODEX_IMAGES__DECODE_CONCURRENCY=3

# Plugins
CODEX_PLUGINS__ALLOWED_COMMANDS='[deno, bun]'

# Scheduler
CODEX_SCHEDULER__TIMEZONE=America/Los_Angeles

# Files (thumbnails, uploads and plugins)
CODEX_FILES__THUMBNAIL_DIR=data/thumbnails
CODEX_FILES__UPLOADS_DIR=data/uploads
CODEX_FILES__PLUGINS_DIR=data/plugins

# PDF Rendering
# CODEX_PDF__PDFIUM_LIBRARY_PATH=/usr/local/lib/libpdfium.so  # Optional, auto-detected
CODEX_PDF__RENDER_DPI=150
CODEX_PDF__JPEG_QUALITY=85
CODEX_PDF__CACHE_RENDERED_PAGES=true
CODEX_PDF__CACHE_DIR=data/cache

# PDF Handle Cache (in-memory open-document cache)
CODEX_PDF_HANDLE_CACHE__ENABLED=true
CODEX_PDF_HANDLE_CACHE__CAPACITY=256
CODEX_PDF_HANDLE_CACHE__IDLE_TTL_MINUTES=15
CODEX_PDF_HANDLE_CACHE__SWEEP_INTERVAL_SECONDS=60

# Komga-Compatible API
CODEX_KOMGA_API__ENABLED=true
CODEX_KOMGA_API__PREFIX=komga

# KOReader Sync API
CODEX_KOREADER_API__ENABLED=true

# Plugin Credential Encryption
CODEX_ENCRYPTION_KEY=your-base64-encoded-32-byte-key

# Rate Limiting
CODEX_RATE_LIMIT__ENABLED=true
CODEX_RATE_LIMIT__ANONYMOUS_RPS=10
CODEX_RATE_LIMIT__ANONYMOUS_BURST=50
CODEX_RATE_LIMIT__AUTHENTICATED_RPS=50
CODEX_RATE_LIMIT__AUTHENTICATED_BURST=200
CODEX_RATE_LIMIT__EXEMPT_PATHS='[/health, /api/v1/events]'
CODEX_RATE_LIMIT__CLEANUP_INTERVAL_SECS=60
CODEX_RATE_LIMIT__BUCKET_TTL_SECS=300

# Observability (OpenTelemetry / OTLP)
CODEX_OBSERVABILITY__ENABLED=true
CODEX_OBSERVABILITY__SERVICE_NAME=codex
CODEX_OBSERVABILITY__OTLP__ENDPOINT=http://localhost:4317
CODEX_OBSERVABILITY__OTLP__PROTOCOL=grpc
CODEX_OBSERVABILITY__OTLP__HEADERS='{signoz-access-token=abc123, x-tenant=production}'
CODEX_OBSERVABILITY__OTLP__TIMEOUT_MS=5000
CODEX_OBSERVABILITY__TRACES__ENABLED=true
CODEX_OBSERVABILITY__TRACES__SAMPLE_RATIO=0.1
CODEX_OBSERVABILITY__METRICS__ENABLED=true
CODEX_OBSERVABILITY__METRICS__EXPORT_INTERVAL_MS=30000
CODEX_OBSERVABILITY__BROWSER__ENABLED=false
CODEX_OBSERVABILITY__BROWSER__PROXY_PATH=/api/v1/observability/otlp
CODEX_OBSERVABILITY__BROWSER__SAMPLE_RATIO=0.1
```

## Runtime vs Startup Settings

Some settings can be changed at runtime via the Settings API, while others require a restart.

### Runtime-Configurable (No Restart Required)

These settings are stored in the database and can be changed via `/api/v1/admin/settings`:

- Thumbnail max dimension
- Thumbnail JPEG quality
- Application name (the display name in the UI; not a config-file key)
- Logging level
- Fuzzy search rollout flag (`search.fuzzy.enabled`)

#### `search.fuzzy.enabled` (Boolean, default `false`)

Routes the global search bar through the in-memory fuzzy index built at startup,
replacing the legacy `LIKE %term%` substring match. The fuzzy path understands
gap-skipped subsequences and treats punctuation as a gap, so queries like
`"on ch"` or `"one punch"` match titles like `"One-Punch Man"`. When disabled,
search falls back to the substring path with no other behavioural change. Toggle
the flag through the admin settings UI or the settings API; no restart needed.
The index itself is always live, so flipping the flag does not trigger a
rebuild.

### Startup-Time (Restart Required)

Everything in the config file and the `CODEX_*` environment is read once, at
startup. That includes:

- Database connection and pool settings, and migration behaviour
- Task worker count and whether workers run in-process
- Scanner concurrent scan limit
- Data directory and the thumbnail / uploads / plugins directories
- JWT secret and cookie flags
- Server host/port and base URL
- OIDC providers
- PDF rendering settings (DPI, cache directory, PDFium library path)
- Rate limiting settings
- Observability settings
- Plugin allowlist and encryption key (`CODEX_ENCRYPTION_KEY`)

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
    database_name: codex
    ssl_mode: verify-full
    ssl_root_cert: /etc/ssl/certs/postgres-ca.crt

application:
  host: 0.0.0.0
  port: 8080
  base_url: https://library.example.com

logging:
  level: info
  file: /var/log/codex/codex.log

auth:
  jwt_expiry_hours: 12
  cookie_secure: true

api:
  enable_api_docs: false
  cors_enabled: true
  cors_origins:
    - https://library.example.com

task:
  worker_count: 8

scanner:
  max_concurrent_scans: 4

files:
  thumbnail_dir: /var/lib/codex/thumbnails
  uploads_dir: /var/lib/codex/uploads
```

The secrets are deliberately absent. Supply them from the environment:

```bash
CODEX_AUTH__JWT_SECRET=...
CODEX_DATABASE__POSTGRES__PASSWORD=...
CODEX_ENCRYPTION_KEY=...
```

or from a `codex.local.yaml` beside this file that your deployment tooling
renders and your version control ignores. Config files are **not** interpolated,
so `password: ${DB_PASSWORD}` stores that literal string as the password.

### Kubernetes Configuration

For Kubernetes deployments, use environment variables for all sensitive data:

Mount a config file only for the nested shapes that are awkward as environment
variables, such as OIDC providers and rate-limit exempt paths, and set
everything else from a ConfigMap and a Secret:

```yaml
# Minimal config file - most settings come from the environment
data_dir: /data

scanner:
  max_concurrent_scans: 2
```

From a ConfigMap:

```bash
CODEX_DATABASE__DB_TYPE=postgres
CODEX_DATABASE__POSTGRES__HOST=postgres-service
CODEX_DATABASE__POSTGRES__PORT=5432
CODEX_DATABASE__POSTGRES__DATABASE_NAME=codex
CODEX_APPLICATION__BASE_URL=https://library.example.com
CODEX_AUTH__COOKIE_SECURE=true
# Migrations belong to one `codex migrate` Job, not to every pod
CODEX_DATABASE__RUN_MIGRATIONS=false
# Web pods; the worker Deployment runs `codex worker` instead
CODEX_TASK__RUN_IN_PROCESS=false
```

From a Secret:

```bash
CODEX_DATABASE__POSTGRES__USERNAME=...
CODEX_DATABASE__POSTGRES__PASSWORD=...
CODEX_AUTH__JWT_SECRET=...
CODEX_ENCRYPTION_KEY=...
```

:::tip Fail the pod, not the request
Run `codex config check --strict --quiet` as an initContainer with the same
environment. It opens no database connection, so it catches a misspelled
variable before the app container starts. See [Kubernetes
deployment](./deployment/kubernetes.md).
:::

## Checking Your Configuration

`codex config check` resolves the configuration exactly as `serve` would, then
reports anything wrong with it. It opens no database connection and writes
nothing, so it is safe to run against a read-only config mount or before a
database is available.

```bash
codex config check                       # findings plus the resolved config
codex config check --quiet               # findings only
codex config check --strict              # exit 1 if anything was reported
codex config check -c /etc/codex.yaml    # a specific config file
```

It reports four things:

| Reported | Severity | Meaning |
|----------|----------|---------|
| A value could not be parsed | error | A variable holds a value of the wrong type. Parsing stops at the first one, so fix it and run again. |
| Environment variables that are no longer read | error | The old flat spelling from 1.x. Each is listed with its `__` replacement. |
| Environment variables that were replaced | error | Removed in favour of a config key that is not a re-spelling, such as `CODEX_SKIP_MIGRATIONS`. The note says whether the meaning inverts. |
| Unrecognized environment variables | warning | Not a Codex setting. Usually a typo; a suggestion is offered when there is a near match. Never fatal, since another tool may legitimately use the `CODEX_` prefix. |

The first three fail the check and stop the server. The last only fails under
`--strict`.

The resolved configuration is printed with every secret replaced by
`<redacted>` (set) or `<unset>` (empty), so the output is safe in logs.

:::tip Use it as a Kubernetes initContainer
Because it needs no database and exits non-zero under `--strict`, `config
check` fails a pod before the app container starts rather than after. See
[Kubernetes deployment](./deployment/kubernetes.md).
:::

## Configuration Validation

Codex validates configuration at startup. Common errors:

| Error | Cause | Solution |
|-------|-------|----------|
| Invalid database type | `db_type` not `sqlite` or `postgres` | Fix the db_type value |
| Missing database path | SQLite requires a path | Add `sqlite.path` |
| Database connection failed | Wrong credentials or host | Check connection settings |
| Invalid port | Port outside 1-65535 range | Use a valid port number |
| File permissions | Can't write to paths | Check directory permissions |

## Library & Plugin Advanced Settings

Libraries and plugins have advanced settings for metadata preprocessing and auto-match conditions. These are configured via the web UI or API, not the config file.

### Library Settings

Libraries support these optional settings for metadata processing:

| Setting | Type | Description |
|---------|------|-------------|
| `title_preprocessing_rules` | JSON | Regex rules to clean series titles during scan |
| `auto_match_conditions` | JSON | Conditions that must pass for auto-matching |

#### Title Preprocessing Rules

Clean up series directory names before they become display titles:

```json
[
  {
    "pattern": "\\s*\\(Digital\\)$",
    "replacement": "",
    "description": "Remove (Digital) suffix"
  }
]
```

Common patterns:
- Remove "(Digital)" suffix: `\\s*\\(Digital\\)$` → ``
- Remove "[Author]" prefix: `^\\[[^\\]]+\\]\\s*` → ``
- Remove year suffix: `\\s*\\(\\d{4}\\)$` → ``

#### Auto-Match Conditions

Control when auto-matching occurs for series in this library:

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "book_count",
      "operator": "gte",
      "value": 1
    }
  ]
}
```

### Plugin Settings

Plugins support these optional settings for search customization:

| Setting | Type | Description |
|---------|------|-------------|
| `search_query_template` | string | Handlebars template for search query |
| `search_preprocessing_rules` | JSON | Regex rules to clean search queries |
| `auto_match_conditions` | JSON | Conditions that must pass for this plugin |
| `use_existing_external_id` | boolean | Reuse existing external ID instead of searching |

#### Search Query Template

Customize the search query sent to the metadata provider:

```handlebars
{{metadata.title}}{{#if metadata.year}} ({{metadata.year}}){{/if}}
```

#### Use Existing External ID

When enabled, if a series already has an external ID for this plugin, Codex will call `plugin.get(external_id)` directly instead of searching.

For detailed configuration, see the [Preprocessing Rules Guide](./preprocessing-rules.md).

## Security Best Practices

1. **Use strong JWT secrets** - Generate with `openssl rand -base64 32`
2. **Set a plugin encryption key** - Required for sync/recommendation plugins; generate with `openssl rand -base64 32`
3. **Never commit secrets** - Use environment variables or secret managers
4. **Require verified TLS to PostgreSQL** - The default mode encrypts opportunistically but accepts any certificate and falls back to plaintext without warning. Set `database.postgres.ssl_mode: verify-full` and `database.postgres.ssl_root_cert: /path/to/ca.crt` (see [PostgreSQL TLS](#postgresql-tls)). The libpq variables `PGSSLMODE` and `PGSSLROOTCERT` still work when the Codex setting is unset.
5. **Restrict bind address** - The default is `0.0.0.0`; use `127.0.0.1` unless the server must be reachable from other hosts
6. **Disable API docs in production** - Set `enable_api_docs: false`
7. **Narrow CORS** - The default `api.cors_origins` is `["*"]`; list your real origins instead
8. **Set `auth.cookie_secure: true`** behind TLS, so the session cookie is never sent over a plaintext downgrade
9. **Validate before you deploy** - Run `codex config check --strict` against the config and environment a release will actually use

## Next Steps

- [Deploy Codex](./deployment)
- [Set up your first library](./getting-started)
- [Explore the API](./api)
- [Configure Preprocessing Rules](./preprocessing-rules)
