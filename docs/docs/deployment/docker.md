---
sidebar_position: 2
---

# Docker Deployment

Docker is the recommended deployment method for most users.

## Quick Start

```bash
# Clone repository
git clone https://github.com/AshDevFr/codex.git
cd codex

# Start with PostgreSQL (production profile)
docker compose --profile prod up -d

# Create admin user
docker compose exec codex codex seed --config /app/config/config.docker.yaml

# View logs
docker compose logs -f codex
```

## Docker Compose Configuration

The provided `docker-compose.yml` includes multiple profiles:

### Production Profile

```bash
docker compose --profile prod up -d
```

Services started:
- **postgres** - PostgreSQL 16 database (port 5432)
- **codex** - Codex server with embedded workers (port 8080)
- **mailhog** - Email testing interface (ports 1025, 8025)

### Development Profile

```bash
docker compose --profile dev up -d
```

Services started:
- **postgres** - PostgreSQL database
- **codex-dev** - Backend with hot reload (port 8080)
- **codex-dev-worker** - Dedicated worker container
- **frontend-dev** - Vite dev server (port 5173)
- **mailhog** - Email testing

## Custom Docker Configuration

```yaml
services:
  codex:
    image: codex:latest
    ports:
      - "8080:8080"
    volumes:
      - ./config:/app/config:ro
      - ./data:/app/data
      - /path/to/library:/library:ro
    environment:
      CODEX_AUTH_JWT_SECRET: "your-secure-secret"
      CODEX_DATABASE_DB_TYPE: postgres
      CODEX_DATABASE_POSTGRES_HOST: postgres
      CODEX_DATABASE_POSTGRES_PASSWORD: secret
    depends_on:
      postgres:
        condition: service_healthy
```

## Volume Considerations

| Volume | Purpose | Permissions |
|--------|---------|-------------|
| `/app/config` | Configuration files | Read-only |
| `/app/data` | Database (SQLite), thumbnails | Read-write |
| `/library` | Media files | Read-only (recommended) |
| `/app/data/thumbnails` | Thumbnail cache | Read-write |

:::tip Library Permissions
Mount your media library as read-only (`:ro`) to prevent accidental modifications. Codex only needs read access to your files.
:::

## Health Checks

Docker health checks are configured automatically:

```yaml
healthcheck:
  test: ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"]
  interval: 10s
  timeout: 5s
  retries: 5
```

## Multiple Libraries

Mount multiple library paths:

```yaml
volumes:
  - /mnt/comics:/library/comics:ro
  - /mnt/manga:/library/manga:ro
  - /mnt/ebooks:/library/ebooks:ro
```

Then create libraries pointing to `/library/comics`, `/library/manga`, etc.

## Environment Variables

Common environment variables for Docker:

| Variable | Description | Example |
|----------|-------------|---------|
| `CODEX_AUTH_JWT_SECRET` | JWT signing secret | `your-secret-key` |
| `CODEX_DATABASE_DB_TYPE` | Database type | `postgres` or `sqlite` |
| `CODEX_DATABASE_POSTGRES_HOST` | PostgreSQL host | `postgres` |
| `CODEX_DATABASE_POSTGRES_PASSWORD` | PostgreSQL password | `secret` |
| `CODEX_APP_LOG_LEVEL` | Log level | `info`, `debug` |

See [Configuration](../configuration) for all options.

## Updating

```bash
# Pull latest image
docker compose pull

# Restart with new image
docker compose --profile prod up -d

# Check logs for migration status
docker compose logs codex
```

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker compose logs codex

# Verify configuration
docker compose exec codex cat /app/config/config.docker.yaml
```

### Database Connection Issues

```bash
# Test PostgreSQL connection
docker compose exec postgres psql -U codex -d codex -c "SELECT 1"

# Check network
docker compose exec codex ping postgres
```

### Permission Issues

```bash
# Check volume permissions
docker compose exec codex ls -la /app/data
docker compose exec codex ls -la /library
```
