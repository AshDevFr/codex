---
---

# Database Migration Strategy

Codex provides two commands for managing database migrations in multi-container environments:

- `migrate` - Runs database migrations and exits
- `wait-for-migrations` - Waits for migrations to complete before proceeding

## Problem

When multiple containers start simultaneously, they may all attempt to run migrations concurrently, causing conflicts and errors. This is especially problematic in Kubernetes deployments where multiple pods can start at the same time.

## Solution

Use a dedicated migration job/container that runs migrations before application containers start. Application containers can then skip migrations and wait for them to complete.

## Commands

### `migrate`

Runs database migrations and exits with an appropriate exit code.

```bash
codex migrate --config /path/to/config.yaml
```

**Usage:**
- Kubernetes Job: Run migrations as a separate job before deploying application pods
- Docker Compose: Use as a one-time service that runs before the main application

**Exit Codes:**
- `0` - Migrations completed successfully
- Non-zero - Migration failed

### `wait-for-migrations`

Waits for migrations to complete before proceeding. Useful as an init container in Kubernetes.

```bash
codex wait-for-migrations --config /path/to/config.yaml [--timeout 300] [--interval 2]
```

**Options:**
- `--timeout` - Maximum time to wait in seconds (default: 300)
- `--interval` - Check interval in seconds (default: 2)

**Usage:**
- Kubernetes Init Container: Wait for migrations before starting the main container
- Docker Compose: Use as a dependency service

## Migration Settings

### `database.run_migrations`

Set to `false` to stop a process applying migrations on startup. It then:
1. Skips running migrations automatically
2. Waits for the schema to become current
3. Fails to start if migrations are not complete within the wait timeout

In YAML:

```yaml
database:
  run_migrations: false
```

Or as an environment variable:

```bash
CODEX_DATABASE__RUN_MIGRATIONS=false codex serve --config /path/to/config.yaml
```

:::warning Renamed in Codex 2.0
This used to be `CODEX_SKIP_MIGRATIONS`, and the sense is **inverted**:
`SKIP_MIGRATIONS=true` becomes `RUN_MIGRATIONS=false`. The old name is no
longer read and Codex refuses to start when it sees it. See the
[2.0 upgrade guide](/docs/migration/v2-config).
:::

The wait is bounded by `database.migration_wait_timeout_secs` (default `300`)
and polled every `database.migration_wait_interval_secs` (default `2`).

`codex migrate` applies migrations unconditionally and ignores this setting, so
a migration Job can share the same environment as the deployments it runs
ahead of.

## Docker Compose Strategy

The `docker-compose.yml` includes a migration service that runs migrations before the application starts:

```yaml
services:
  # Migration job (runs once)
  codex-migrate:
    build:
      context: .
      dockerfile: Dockerfile
    depends_on:
      postgres:
        condition: service_healthy
    command: ["codex", "migrate", "--config", "/app/config/config.docker.yaml"]
    restart: "no"  # Run once and exit

  # Application (skips migrations)
  codex:
    depends_on:
      postgres:
        condition: service_healthy
    environment:
      CODEX_DATABASE__RUN_MIGRATIONS: "false"
    command: ["codex", "serve", "--config", "/app/config/config.docker.yaml"]
```

**Usage:**
```bash
# Start migration job
docker-compose up codex-migrate

# Start application (migrations already done)
docker-compose up codex
```

## Kubernetes Strategy

### Option 1: Init Container (Recommended)

Use `wait-for-migrations` as an init container:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codex
spec:
  template:
    spec:
      initContainers:
      - name: wait-for-migrations
        image: codex:latest
        command: ["codex", "wait-for-migrations", "--config", "/app/config/config.yaml"]
        env:
        - name: CODEX_DATABASE__POSTGRES__HOST
          value: "postgres"
        # ... other database config
      containers:
      - name: codex
        image: codex:latest
        env:
        - name: CODEX_DATABASE__RUN_MIGRATIONS
          value: "false"
        # ... other config
```

### Option 2: Migration Job

Run migrations as a separate Kubernetes Job:

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: codex-migrate
spec:
  template:
    spec:
      containers:
      - name: migrate
        image: codex:latest
        command: ["codex", "migrate", "--config", "/app/config/config.yaml"]
        env:
        - name: CODEX_DATABASE__POSTGRES__HOST
          value: "postgres"
        # ... other database config
      restartPolicy: Never
  backoffLimit: 3
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codex
spec:
  template:
    spec:
      containers:
      - name: codex
        image: codex:latest
        env:
        - name: CODEX_DATABASE__RUN_MIGRATIONS
          value: "false"
        # ... other config
```

**Deployment Order:**
1. Deploy migration job
2. Wait for job completion
3. Deploy application deployment

## Best Practices

1. **Always run migrations before application starts** - Use a job or init container
2. **Set `CODEX_DATABASE__RUN_MIGRATIONS=false`** in application containers to prevent conflicts
3. **Use health checks** - Ensure database is ready before running migrations
4. **Monitor migration jobs** - Set appropriate timeouts and retry limits
5. **Test migrations** - Test migration strategy in staging before production

## Troubleshooting

### Migrations fail with "already applied" errors

This is normal when multiple containers try to run migrations. Use the migration job/init container strategy.

### Application fails to start with "migrations not complete"

- Ensure migration job/init container completed successfully
- Check database connectivity
- Verify `CODEX_DATABASE__RUN_MIGRATIONS` is set correctly

### Migration job hangs

- Check database connectivity
- Verify database credentials
- Check migration logs for errors
- Increase timeout if migrations are slow

