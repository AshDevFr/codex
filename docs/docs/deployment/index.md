---
sidebar_position: 1
---

# Deployment Overview

This guide covers deploying Codex in various environments, from simple homelab setups to production Kubernetes clusters.

## Deployment Options

| Deployment Type | Database | Best For | Scaling |
|-----------------|----------|----------|---------|
| [Docker Compose](./docker) | PostgreSQL | Production, Multi-user | Single host |
| [Kubernetes](./kubernetes) | PostgreSQL | Enterprise, High Availability | Horizontal |
| [Systemd Service](./systemd) | Either | Dedicated server | None |
| Binary + SQLite | SQLite | Homelab, Single-user | None |

## Understanding Limitations

Before choosing a deployment strategy, understand these architectural considerations.

### SQLite Limitations

SQLite is excellent for simple setups but has constraints:

| Limitation | Impact | Recommendation |
|------------|--------|----------------|
| Single writer | Only one write operation at a time | Use PostgreSQL for multi-user |
| No horizontal scaling | Cannot run multiple Codex instances | Use PostgreSQL for scaling |
| Limited concurrency | Performance degrades with many users | Limit to ~5-10 concurrent users |
| No distributed workers | Workers must run in same process | Use combined `serve` mode |

:::warning SQLite Worker Limitation
With SQLite, you **cannot** run separate worker processes. The background task workers must run within the same process as the web server. Use `codex serve` which includes both the API server and workers.

Running `codex worker` separately with SQLite will cause database locking issues and is not supported.
:::

### PostgreSQL Advantages

PostgreSQL enables:
- Multiple concurrent users
- Horizontal scaling (multiple Codex instances)
- Separate worker processes
- Better performance under load
- LISTEN/NOTIFY for real-time cross-process events

### Server-Sent Events (SSE) Considerations

SSE streams require special handling in certain deployments:

| Deployment | SSE Behavior | Notes |
|------------|--------------|-------|
| Single instance | Works perfectly | All events delivered instantly |
| Multiple instances | Works with PostgreSQL | Events replayed via LISTEN/NOTIFY |
| Behind proxy | Requires configuration | Disable buffering, set timeouts |
| SQLite + Workers | Not supported | Cannot separate workers |

## Quick Start

### Docker (Recommended)

```bash
git clone https://github.com/AshDevFr/codex.git
cd codex
docker compose --profile prod up -d
docker compose exec codex codex seed --config /app/config/config.docker.yaml
```

### Binary

```bash
# Download and extract
curl -LO https://github.com/AshDevFr/codex/releases/latest/download/codex-linux-amd64.tar.gz
tar xzf codex-linux-amd64.tar.gz

# Configure and run
cp codex.example.yaml codex.yaml
./codex serve --config codex.yaml
```

## In This Section

- [Docker Deployment](./docker) - Docker Compose setup for development and production
- [Kubernetes Deployment](./kubernetes) - Horizontal scaling with K8s
- [Systemd Service](./systemd) - Traditional Linux server deployment
- [Reverse Proxy](./reverse-proxy) - Nginx, Caddy, and Traefik configuration
- [Database Setup](./database) - PostgreSQL and SQLite configuration
- [Security & Operations](./operations) - Security, monitoring, and backups
