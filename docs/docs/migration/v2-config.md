---
sidebar_position: 1
---

# Upgrading to Codex 2.0

Codex 2.0 renames every configuration environment variable. Nothing else about
your setup has to change: config files keep the same shape, and there is no
database migration beyond the usual automatic one.

**Codex will not start with the old names.** That is deliberate. The
alternative was to ignore them, which means a server running with default rate
limits, the wrong port, or workers in a pod meant to serve web traffic, and no
indication anything is wrong. A refusal at startup, listing exactly what to
change, is the safer failure.

## Before you upgrade

On 1.44 or later, ask the running version what will break:

```bash
codex config check
```

It prints every variable you set that changes, with its replacement. Prepare
the edit, then apply it at the same time you bump the image tag.

:::warning Do not rename before upgrading
1.x does not read the new spelling. A variable renamed early is silently
ignored, which is the failure mode this release exists to remove.
:::

## What changed, and why

The old scheme used a single `_` for two different jobs: separating nesting
levels, and separating words inside a key. Nothing in
`CODEX_RATE_LIMIT_ANONYMOUS_RPS` says whether the section is `rate_limit` or
`rate`, so every variable needed a hand-written rule, and adding a setting
meant remembering to add one. Several documented variables never had one and
did nothing at all.

Codex 2.0 separates nesting levels with `__` and keeps `_` for words within a
key:

```
rate_limit.anonymous_rps       ->  CODEX_RATE_LIMIT__ANONYMOUS_RPS
database.postgres.max_connections  ->  CODEX_DATABASE__POSTGRES__MAX_CONNECTIONS
pdf_handle_cache.capacity      ->  CODEX_PDF_HANDLE_CACHE__CAPACITY
```

## Settings that moved

These are not simple renames. Two invert their meaning, so read them carefully:

| Before | After | Note |
| ------ | ----- | ---- |
| `CODEX_COOKIE_SECURE` | `CODEX_AUTH__COOKIE_SECURE` | same meaning |
| `CODEX_DISABLE_WORKERS` | `CODEX_TASK__RUN_IN_PROCESS` | **inverted**: `DISABLE_WORKERS=true` becomes `RUN_IN_PROCESS=false` |
| `CODEX_IMAGE_DECODE_CONCURRENCY` | `CODEX_IMAGES__DECODE_CONCURRENCY` | same meaning |
| `CODEX_MIGRATION_WAIT_INTERVAL` | `CODEX_DATABASE__MIGRATION_WAIT_INTERVAL_SECS` | same meaning |
| `CODEX_MIGRATION_WAIT_TIMEOUT` | `CODEX_DATABASE__MIGRATION_WAIT_TIMEOUT_SECS` | same meaning |
| `CODEX_PLUGIN_ALLOWED_COMMANDS` | `CODEX_PLUGINS__ALLOWED_COMMANDS` | same meaning |
| `CODEX_SKIP_MIGRATIONS` | `CODEX_DATABASE__RUN_MIGRATIONS` | **inverted**: `SKIP_MIGRATIONS=true` becomes `RUN_MIGRATIONS=false` |

All seven are now ordinary config keys, so they can live in `codex.yaml`
instead of the environment.

## Settings that stay environment-only

`CODEX_ENCRYPTION_KEY`, `CODEX_SOURCE_DATABASE_URL` and
`CODEX_TARGET_DATABASE_URL` are unchanged.

## Other behaviour changes

**A bad value now stops the server.** The old override layer discarded values
it could not parse, so `CODEX_KOMGA_API_ENABLED=ture` quietly meant `false`.
Unrecognized input is now an error naming the variable.

**Startup no longer writes a config file.** Codex used to serialize its
defaults to disk when the file was missing, which produced an uncommented dump
and, because those defaults were read from the environment, could capture a
database password in plaintext. Run `codex config init` for a commented
starter instead.

**Values keep the shapes you already use.** Booleans still accept
`true`/`false`, `1`/`0`, `yes`/`no` and `on`/`off`. Lists still accept a
comma-separated string. `CODEX_OBSERVABILITY__OTLP__HEADERS` still accepts
`k1=v1,k2=v2`. An empty value still means "unset".

## New in this release

**Local overlay.** A `codex.local.yaml` beside your config is merged on top of
it, so secrets and per-host tweaks no longer mean editing the committed file.
It merges key by key; a list in the overlay replaces the base list rather than
extending it.

**TOML.** Config files may be `.toml` as well as `.yaml`, chosen by extension.

**PostgreSQL TLS.** `database.postgres.ssl_mode` and friends are real settings.
See [Configuration](../configuration.md#postgresql-tls). The libpq variables
(`PGSSLMODE` and so on) still work when the Codex setting is unset.

## Full rename table

Generated from the configuration schema, so it is exhaustive.

### `api`

| Before | After |
| ------ | ----- |
| `CODEX_API_API_DOCS_PATH` | `CODEX_API__API_DOCS_PATH` |
| `CODEX_API_BASE_PATH` | `CODEX_API__BASE_PATH` |
| `CODEX_API_CORS_ENABLED` | `CODEX_API__CORS_ENABLED` |
| `CODEX_API_CORS_ORIGINS` | `CODEX_API__CORS_ORIGINS` |
| `CODEX_API_ENABLE_API_DOCS` | `CODEX_API__ENABLE_API_DOCS` |
| `CODEX_API_MAX_PAGE_SIZE` | `CODEX_API__MAX_PAGE_SIZE` |

### `application`

| Before | After |
| ------ | ----- |
| `CODEX_APPLICATION_BASE_URL` | `CODEX_APPLICATION__BASE_URL` |
| `CODEX_APPLICATION_HOST` | `CODEX_APPLICATION__HOST` |
| `CODEX_APPLICATION_PORT` | `CODEX_APPLICATION__PORT` |

### `auth`

| Before | After |
| ------ | ----- |
| `CODEX_AUTH_ARGON2_MEMORY_COST` | `CODEX_AUTH__ARGON2_MEMORY_COST` |
| `CODEX_AUTH_ARGON2_PARALLELISM` | `CODEX_AUTH__ARGON2_PARALLELISM` |
| `CODEX_AUTH_ARGON2_TIME_COST` | `CODEX_AUTH__ARGON2_TIME_COST` |
| `CODEX_AUTH_COOKIE_SECURE` | `CODEX_AUTH__COOKIE_SECURE` |
| `CODEX_AUTH_EMAIL_CONFIRMATION_REQUIRED` | `CODEX_AUTH__EMAIL_CONFIRMATION_REQUIRED` |
| `CODEX_AUTH_JWT_EXPIRY_HOURS` | `CODEX_AUTH__JWT_EXPIRY_HOURS` |
| `CODEX_AUTH_JWT_SECRET` | `CODEX_AUTH__JWT_SECRET` |
| `CODEX_AUTH_OIDC_ALLOWED_REDIRECT_URIS` | `CODEX_AUTH__OIDC__ALLOWED_REDIRECT_URIS` |
| `CODEX_AUTH_OIDC_AUTO_CREATE_USERS` | `CODEX_AUTH__OIDC__AUTO_CREATE_USERS` |
| `CODEX_AUTH_OIDC_DEFAULT_ROLE` | `CODEX_AUTH__OIDC__DEFAULT_ROLE` |
| `CODEX_AUTH_OIDC_ENABLED` | `CODEX_AUTH__OIDC__ENABLED` |
| `CODEX_AUTH_OIDC_PROVIDERS` | `CODEX_AUTH__OIDC__PROVIDERS` |
| `CODEX_AUTH_OIDC_PROVIDERS_*_ROLE_MAPPING` | `CODEX_AUTH__OIDC__PROVIDERS__*__ROLE_MAPPING` |
| `CODEX_AUTH_OIDC_REDIRECT_URI_BASE` | `CODEX_AUTH__OIDC__REDIRECT_URI_BASE` |
| `CODEX_AUTH_REFRESH_TOKEN_ENABLED` | `CODEX_AUTH__REFRESH_TOKEN_ENABLED` |
| `CODEX_AUTH_REFRESH_TOKEN_EXPIRY_DAYS` | `CODEX_AUTH__REFRESH_TOKEN_EXPIRY_DAYS` |

### `database`

| Before | After |
| ------ | ----- |
| `CODEX_DATABASE_DB_TYPE` | `CODEX_DATABASE__DB_TYPE` |
| `CODEX_DATABASE_MIGRATION_WAIT_INTERVAL_SECS` | `CODEX_DATABASE__MIGRATION_WAIT_INTERVAL_SECS` |
| `CODEX_DATABASE_MIGRATION_WAIT_TIMEOUT_SECS` | `CODEX_DATABASE__MIGRATION_WAIT_TIMEOUT_SECS` |
| `CODEX_DATABASE_POSTGRES_ACQUIRE_TIMEOUT_SECONDS` | `CODEX_DATABASE__POSTGRES__ACQUIRE_TIMEOUT_SECONDS` |
| `CODEX_DATABASE_POSTGRES_BACKGROUND_MAX_CONNECTIONS` | `CODEX_DATABASE__POSTGRES__BACKGROUND_MAX_CONNECTIONS` |
| `CODEX_DATABASE_POSTGRES_BATCH_FAN_OUT` | `CODEX_DATABASE__POSTGRES__BATCH_FAN_OUT` |
| `CODEX_DATABASE_POSTGRES_DATABASE_NAME` | `CODEX_DATABASE__POSTGRES__DATABASE_NAME` |
| `CODEX_DATABASE_POSTGRES_HOST` | `CODEX_DATABASE__POSTGRES__HOST` |
| `CODEX_DATABASE_POSTGRES_IDLE_TIMEOUT_SECONDS` | `CODEX_DATABASE__POSTGRES__IDLE_TIMEOUT_SECONDS` |
| `CODEX_DATABASE_POSTGRES_MAX_CONNECTIONS` | `CODEX_DATABASE__POSTGRES__MAX_CONNECTIONS` |
| `CODEX_DATABASE_POSTGRES_MAX_LIFETIME_SECONDS` | `CODEX_DATABASE__POSTGRES__MAX_LIFETIME_SECONDS` |
| `CODEX_DATABASE_POSTGRES_MIN_CONNECTIONS` | `CODEX_DATABASE__POSTGRES__MIN_CONNECTIONS` |
| `CODEX_DATABASE_POSTGRES_OPERATION_DEADLINE_SECONDS` | `CODEX_DATABASE__POSTGRES__OPERATION_DEADLINE_SECONDS` |
| `CODEX_DATABASE_POSTGRES_PASSWORD` | `CODEX_DATABASE__POSTGRES__PASSWORD` |
| `CODEX_DATABASE_POSTGRES_PORT` | `CODEX_DATABASE__POSTGRES__PORT` |
| `CODEX_DATABASE_POSTGRES_SSL_CLIENT_CERT` | `CODEX_DATABASE__POSTGRES__SSL_CLIENT_CERT` |
| `CODEX_DATABASE_POSTGRES_SSL_CLIENT_KEY` | `CODEX_DATABASE__POSTGRES__SSL_CLIENT_KEY` |
| `CODEX_DATABASE_POSTGRES_SSL_MODE` | `CODEX_DATABASE__POSTGRES__SSL_MODE` |
| `CODEX_DATABASE_POSTGRES_SSL_ROOT_CERT` | `CODEX_DATABASE__POSTGRES__SSL_ROOT_CERT` |
| `CODEX_DATABASE_POSTGRES_USERNAME` | `CODEX_DATABASE__POSTGRES__USERNAME` |
| `CODEX_DATABASE_RUN_MIGRATIONS` | `CODEX_DATABASE__RUN_MIGRATIONS` |
| `CODEX_DATABASE_SQLITE_ACQUIRE_TIMEOUT_SECONDS` | `CODEX_DATABASE__SQLITE__ACQUIRE_TIMEOUT_SECONDS` |
| `CODEX_DATABASE_SQLITE_BACKGROUND_MAX_CONNECTIONS` | `CODEX_DATABASE__SQLITE__BACKGROUND_MAX_CONNECTIONS` |
| `CODEX_DATABASE_SQLITE_BATCH_FAN_OUT` | `CODEX_DATABASE__SQLITE__BATCH_FAN_OUT` |
| `CODEX_DATABASE_SQLITE_IDLE_TIMEOUT_SECONDS` | `CODEX_DATABASE__SQLITE__IDLE_TIMEOUT_SECONDS` |
| `CODEX_DATABASE_SQLITE_MAX_CONNECTIONS` | `CODEX_DATABASE__SQLITE__MAX_CONNECTIONS` |
| `CODEX_DATABASE_SQLITE_MAX_LIFETIME_SECONDS` | `CODEX_DATABASE__SQLITE__MAX_LIFETIME_SECONDS` |
| `CODEX_DATABASE_SQLITE_MIN_CONNECTIONS` | `CODEX_DATABASE__SQLITE__MIN_CONNECTIONS` |
| `CODEX_DATABASE_SQLITE_OPERATION_DEADLINE_SECONDS` | `CODEX_DATABASE__SQLITE__OPERATION_DEADLINE_SECONDS` |
| `CODEX_DATABASE_SQLITE_PATH` | `CODEX_DATABASE__SQLITE__PATH` |
| `CODEX_DATABASE_SQLITE_PRAGMAS` | `CODEX_DATABASE__SQLITE__PRAGMAS` |

### `email`

| Before | After |
| ------ | ----- |
| `CODEX_EMAIL_SMTP_FROM_EMAIL` | `CODEX_EMAIL__SMTP_FROM_EMAIL` |
| `CODEX_EMAIL_SMTP_FROM_NAME` | `CODEX_EMAIL__SMTP_FROM_NAME` |
| `CODEX_EMAIL_SMTP_HOST` | `CODEX_EMAIL__SMTP_HOST` |
| `CODEX_EMAIL_SMTP_PASSWORD` | `CODEX_EMAIL__SMTP_PASSWORD` |
| `CODEX_EMAIL_SMTP_PORT` | `CODEX_EMAIL__SMTP_PORT` |
| `CODEX_EMAIL_SMTP_USERNAME` | `CODEX_EMAIL__SMTP_USERNAME` |
| `CODEX_EMAIL_VERIFICATION_TOKEN_EXPIRY_HOURS` | `CODEX_EMAIL__VERIFICATION_TOKEN_EXPIRY_HOURS` |
| `CODEX_EMAIL_VERIFICATION_URL_BASE` | `CODEX_EMAIL__VERIFICATION_URL_BASE` |

### `files`

| Before | After |
| ------ | ----- |
| `CODEX_FILES_PLUGINS_DIR` | `CODEX_FILES__PLUGINS_DIR` |
| `CODEX_FILES_THUMBNAIL_DIR` | `CODEX_FILES__THUMBNAIL_DIR` |
| `CODEX_FILES_UPLOADS_DIR` | `CODEX_FILES__UPLOADS_DIR` |

### `images`

| Before | After |
| ------ | ----- |
| `CODEX_IMAGES_DECODE_CONCURRENCY` | `CODEX_IMAGES__DECODE_CONCURRENCY` |

### `komga_api`

| Before | After |
| ------ | ----- |
| `CODEX_KOMGA_API_ENABLED` | `CODEX_KOMGA_API__ENABLED` |
| `CODEX_KOMGA_API_PREFIX` | `CODEX_KOMGA_API__PREFIX` |

### `koreader_api`

| Before | After |
| ------ | ----- |
| `CODEX_KOREADER_API_ENABLED` | `CODEX_KOREADER_API__ENABLED` |

### `logging`

| Before | After |
| ------ | ----- |
| `CODEX_LOGGING_CONSOLE` | `CODEX_LOGGING__CONSOLE` |
| `CODEX_LOGGING_FILE` | `CODEX_LOGGING__FILE` |
| `CODEX_LOGGING_LEVEL` | `CODEX_LOGGING__LEVEL` |

### `observability`

| Before | After |
| ------ | ----- |
| `CODEX_OBSERVABILITY_BROWSER_ENABLED` | `CODEX_OBSERVABILITY__BROWSER__ENABLED` |
| `CODEX_OBSERVABILITY_BROWSER_PROXY_PATH` | `CODEX_OBSERVABILITY__BROWSER__PROXY_PATH` |
| `CODEX_OBSERVABILITY_BROWSER_SAMPLE_RATIO` | `CODEX_OBSERVABILITY__BROWSER__SAMPLE_RATIO` |
| `CODEX_OBSERVABILITY_ENABLED` | `CODEX_OBSERVABILITY__ENABLED` |
| `CODEX_OBSERVABILITY_METRICS_ENABLED` | `CODEX_OBSERVABILITY__METRICS__ENABLED` |
| `CODEX_OBSERVABILITY_METRICS_EXPORT_INTERVAL_MS` | `CODEX_OBSERVABILITY__METRICS__EXPORT_INTERVAL_MS` |
| `CODEX_OBSERVABILITY_OTLP_ENDPOINT` | `CODEX_OBSERVABILITY__OTLP__ENDPOINT` |
| `CODEX_OBSERVABILITY_OTLP_HEADERS` | `CODEX_OBSERVABILITY__OTLP__HEADERS` |
| `CODEX_OBSERVABILITY_OTLP_PROTOCOL` | `CODEX_OBSERVABILITY__OTLP__PROTOCOL` |
| `CODEX_OBSERVABILITY_OTLP_PROXY_ENDPOINT` | `CODEX_OBSERVABILITY__OTLP__PROXY_ENDPOINT` |
| `CODEX_OBSERVABILITY_OTLP_TIMEOUT_MS` | `CODEX_OBSERVABILITY__OTLP__TIMEOUT_MS` |
| `CODEX_OBSERVABILITY_SERVICE_NAME` | `CODEX_OBSERVABILITY__SERVICE_NAME` |
| `CODEX_OBSERVABILITY_TRACES_ENABLED` | `CODEX_OBSERVABILITY__TRACES__ENABLED` |
| `CODEX_OBSERVABILITY_TRACES_SAMPLE_RATIO` | `CODEX_OBSERVABILITY__TRACES__SAMPLE_RATIO` |

### `pdf`

| Before | After |
| ------ | ----- |
| `CODEX_PDF_CACHE_DIR` | `CODEX_PDF__CACHE_DIR` |
| `CODEX_PDF_CACHE_RENDERED_PAGES` | `CODEX_PDF__CACHE_RENDERED_PAGES` |
| `CODEX_PDF_JPEG_QUALITY` | `CODEX_PDF__JPEG_QUALITY` |
| `CODEX_PDF_PDFIUM_LIBRARY_PATH` | `CODEX_PDF__PDFIUM_LIBRARY_PATH` |
| `CODEX_PDF_RENDER_DPI` | `CODEX_PDF__RENDER_DPI` |

### `pdf_handle_cache`

| Before | After |
| ------ | ----- |
| `CODEX_PDF_HANDLE_CACHE_CAPACITY` | `CODEX_PDF_HANDLE_CACHE__CAPACITY` |
| `CODEX_PDF_HANDLE_CACHE_ENABLED` | `CODEX_PDF_HANDLE_CACHE__ENABLED` |
| `CODEX_PDF_HANDLE_CACHE_IDLE_TTL_MINUTES` | `CODEX_PDF_HANDLE_CACHE__IDLE_TTL_MINUTES` |
| `CODEX_PDF_HANDLE_CACHE_SWEEP_INTERVAL_SECONDS` | `CODEX_PDF_HANDLE_CACHE__SWEEP_INTERVAL_SECONDS` |

### `plugins`

| Before | After |
| ------ | ----- |
| `CODEX_PLUGINS_ALLOWED_COMMANDS` | `CODEX_PLUGINS__ALLOWED_COMMANDS` |

### `rate_limit`

| Before | After |
| ------ | ----- |
| `CODEX_RATE_LIMIT_ANONYMOUS_BURST` | `CODEX_RATE_LIMIT__ANONYMOUS_BURST` |
| `CODEX_RATE_LIMIT_ANONYMOUS_RPS` | `CODEX_RATE_LIMIT__ANONYMOUS_RPS` |
| `CODEX_RATE_LIMIT_AUTHENTICATED_BURST` | `CODEX_RATE_LIMIT__AUTHENTICATED_BURST` |
| `CODEX_RATE_LIMIT_AUTHENTICATED_RPS` | `CODEX_RATE_LIMIT__AUTHENTICATED_RPS` |
| `CODEX_RATE_LIMIT_BUCKET_TTL_SECS` | `CODEX_RATE_LIMIT__BUCKET_TTL_SECS` |
| `CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS` | `CODEX_RATE_LIMIT__CLEANUP_INTERVAL_SECS` |
| `CODEX_RATE_LIMIT_ENABLED` | `CODEX_RATE_LIMIT__ENABLED` |
| `CODEX_RATE_LIMIT_EXEMPT_PATHS` | `CODEX_RATE_LIMIT__EXEMPT_PATHS` |

### `scanner`

| Before | After |
| ------ | ----- |
| `CODEX_SCANNER_MAX_CONCURRENT_SCANS` | `CODEX_SCANNER__MAX_CONCURRENT_SCANS` |

### `scheduler`

| Before | After |
| ------ | ----- |
| `CODEX_SCHEDULER_TIMEZONE` | `CODEX_SCHEDULER__TIMEZONE` |

### `task`

| Before | After |
| ------ | ----- |
| `CODEX_TASK_RUN_IN_PROCESS` | `CODEX_TASK__RUN_IN_PROCESS` |
| `CODEX_TASK_WORKER_COUNT` | `CODEX_TASK__WORKER_COUNT` |

## Verifying

After upgrading:

```bash
codex config check
```

A clean run prints the resolved configuration with secrets redacted. As a
Kubernetes initContainer, `codex config check --strict --quiet` fails the pod
before the app container starts.
