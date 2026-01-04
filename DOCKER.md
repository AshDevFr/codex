# Docker Setup for Codex

This document describes how to run Codex with Docker and Docker Compose.

## Quick Start

### Production Mode (with PostgreSQL)

```bash
# Start services
docker compose up -d

# View logs
docker compose logs -f codex

# Stop services
docker compose down
```

### Development Mode (with hot reload)

```bash
# Start development environment
docker compose --profile dev up -d

# View logs
docker compose logs -f codex-dev

# Stop
docker compose --profile dev down
```

### Development with Watch Mode

Docker Compose watch mode automatically syncs code changes:

```bash
# Start with watch mode
docker compose -f docker-compose.yml -f compose.watch.yml --profile dev up --watch

# Make changes to src/ files - they sync automatically!
# cargo-watch will detect changes and rebuild
```

## Available Services

### Production Services (default profile)

- **postgres** - PostgreSQL 16 database
  - Port: 5432
  - User: codex
  - Password: codex
  - Database: codex

- **codex** - Production application
  - Port: 8080
  - Built with release optimizations
  - Auto-restarts on failure

### Development Services (dev profile)

- **codex-dev** - Development application
  - Port: 8080
  - Hot reload with cargo-watch
  - Debug logging enabled
  - Source code mounted as volume

### Test Services (test profile)

- **postgres-test** - Test database
  - Port: 5433 (different from main DB)
  - User: codex_test
  - Password: codex_test
  - Database: codex_test
  - Uses tmpfs (data cleared on restart)

## Running Tests with PostgreSQL

### Start test database

```bash
docker compose --profile test up -d postgres-test

# Wait for it to be ready
docker compose --profile test ps
```

### Run PostgreSQL tests

```bash
# Set environment variables
export POSTGRES_HOST=localhost
export POSTGRES_PORT=5433
export POSTGRES_USER=codex_test
export POSTGRES_PASSWORD=codex_test
export POSTGRES_DB=codex_test

# Run ignored tests
cargo test --test postgres_integration_tests -- --ignored
cargo test db::postgres -- --ignored
```

### Stop test database

```bash
docker compose --profile test down
```

## Configuration

### Using Different Configs

The application uses the config file at `/app/config/config.yaml` inside the container.

Mount your config:

```bash
docker run -v ./my-config.yaml:/app/config/config.yaml codex
```

Or use environment variables in docker-compose.yml.

### Example Configs

- `config/config.docker.yaml` - PostgreSQL setup for Docker
- `config/config.sqlite.yaml` - SQLite setup for local development

## Building

### Build production image

```bash
docker build -t codex:latest .
```

### Build development image

```bash
docker build -f Dockerfile.dev -t codex:dev .
```

### Multi-platform build

```bash
docker buildx build --platform linux/amd64,linux/arm64 -t codex:latest .
```

## Volumes

### Persistent Data

- `postgres_data` - PostgreSQL database files
- `codex_data` - Application data (if using file storage)

### Development Cache

- `cargo_cache` - Cargo registry cache (speeds up rebuilds)
- `target_cache` - Build artifacts cache

### Managing Volumes

```bash
# List volumes
docker volume ls | grep codex

# Remove all volumes (CAUTION: deletes data!)
docker compose down -v

# Backup database
docker exec codex-postgres pg_dump -U codex codex > backup.sql

# Restore database
cat backup.sql | docker exec -i codex-postgres psql -U codex codex
```

## Networking

All services use the `codex-network` bridge network.

Access between containers:
- Application → Database: `postgres:5432`
- External → Application: `localhost:8080`
- External → Database: `localhost:5432`

## Troubleshooting

### Database connection failed

```bash
# Check if postgres is healthy
docker compose ps

# View postgres logs
docker compose logs postgres

# Test connection manually
docker exec -it codex-postgres psql -U codex -d codex
```

### Application won't start

```bash
# View application logs
docker compose logs codex

# Check migrations
docker exec -it codex-app ls -la /app

# Connect to container
docker exec -it codex-app bash
```

### Hot reload not working (dev mode)

```bash
# Check cargo-watch logs
docker compose logs -f codex-dev

# Manually trigger rebuild
docker compose restart codex-dev

# Check file mounts
docker exec -it codex-dev ls -la /app/src
```

## Environment Variables

Available environment variables:

```bash
# Rust
RUST_LOG=debug           # Logging level (error, warn, info, debug, trace)
RUST_BACKTRACE=1         # Enable backtraces

# PostgreSQL (for tests)
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_USER=codex_test
POSTGRES_PASSWORD=codex_test
POSTGRES_DB=codex_test
```

## Clean Up

```bash
# Stop all services
docker compose --profile dev --profile test down

# Remove volumes (DELETES DATA!)
docker compose --profile dev --profile test down -v

# Remove images
docker rmi codex:latest codex:dev

# Full cleanup (removes everything)
docker system prune -a --volumes
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Test with PostgreSQL

jobs:
  test:
    runs-on: ubuntu-latest
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
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run PostgreSQL tests
        run: cargo test --test postgres_integration_tests -- --ignored
        env:
          POSTGRES_HOST: localhost
          POSTGRES_PORT: 5432
          POSTGRES_USER: codex_test
          POSTGRES_PASSWORD: codex_test
          POSTGRES_DB: codex_test
```

## Performance Tips

1. **Use bind mounts for source code** (faster on macOS/Windows)
2. **Use volumes for caches** (cargo_cache, target_cache)
3. **Use tmpfs for test database** (faster tests)
4. **Multi-stage builds** reduce final image size
5. **Cache dependencies layer** speeds up rebuilds
