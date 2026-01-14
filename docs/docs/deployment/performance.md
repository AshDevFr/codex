---
sidebar_position: 8
---

# Performance Tuning

Optimize Codex for large libraries, high traffic, or resource-constrained environments.

## Library Size Guidelines

| Library Size | Recommended Setup | Database | Workers |
|--------------|-------------------|----------|---------|
| < 1,000 books | Single instance | SQLite | 2-4 |
| 1,000 - 10,000 books | Single instance | SQLite or PostgreSQL | 4 |
| 10,000 - 50,000 books | Single instance | PostgreSQL | 4-8 |
| 50,000+ books | Multiple instances | PostgreSQL | 8+ per instance |

## Database Optimization

### SQLite Performance

SQLite works well for small to medium libraries with few concurrent users.

#### Enable WAL Mode

Write-Ahead Logging significantly improves concurrent read/write performance:

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db
    pragmas:
      journal_mode: WAL
      synchronous: NORMAL
```

:::tip
WAL mode allows readers while a write is in progress, dramatically improving responsiveness during library scans.
:::

#### SQLite Limitations

- **Single writer**: Only one process can write at a time
- **No horizontal scaling**: Cannot run multiple Codex instances
- **Concurrent users**: Best for 5-10 simultaneous users
- **File locking**: Network filesystems (NFS, SMB) may cause issues

### PostgreSQL Performance

PostgreSQL is recommended for larger libraries and multiple concurrent users.

#### Connection Pooling

Configure connection limits based on your workload:

```yaml
database:
  db_type: postgres
  postgres:
    host: localhost
    port: 5432
    username: codex
    password: your-password
    database_name: codex
    max_connections: 100
    min_connections: 5
    connect_timeout: 30
    idle_timeout: 600
```

**Guidelines:**
- `max_connections`: 10 × expected concurrent users + workers
- `min_connections`: Number of background workers
- Keep total connections under PostgreSQL's `max_connections` setting

#### PostgreSQL Tuning

For large libraries, tune PostgreSQL itself:

```ini
# postgresql.conf

# Memory (adjust based on available RAM)
shared_buffers = 256MB          # 25% of RAM, up to 1GB
effective_cache_size = 768MB    # 75% of RAM
work_mem = 16MB                 # Per-operation memory

# Write Performance
wal_buffers = 16MB
checkpoint_completion_target = 0.9

# Query Planning
random_page_cost = 1.1          # For SSDs
effective_io_concurrency = 200  # For SSDs
```

#### Index Maintenance

Periodically optimize indexes:

```bash
# Vacuum and analyze (run weekly)
psql -U codex codex -c "VACUUM ANALYZE"

# Reindex if queries slow down (run monthly)
psql -U codex codex -c "REINDEX DATABASE codex"
```

## Worker Configuration

Background workers handle scanning, thumbnail generation, and analysis tasks.

### Worker Count

```yaml
task:
  worker_count: 4
```

**Guidelines:**
- **Minimum**: 2 workers (one for scanning, one for other tasks)
- **Recommended**: CPU cores - 1 (leave headroom for web server)
- **Maximum**: 2 × CPU cores (diminishing returns beyond this)

:::caution
More workers isn't always better. Too many workers can cause:
- Memory exhaustion (each worker loads files into memory)
- Disk I/O saturation
- Database connection pool exhaustion
:::

### Scanner Concurrency

Limit simultaneous library scans:

```yaml
scanner:
  max_concurrent_scans: 2
```

Scanning is I/O intensive. Running too many concurrent scans can:
- Saturate disk bandwidth
- Cause memory pressure from parallel file processing
- Slow down the web interface

## Memory Optimization

### Thumbnail Configuration

Thumbnails are the largest memory consumers. Configure via the Settings API:

```bash
# Reduce thumbnail size for memory-constrained systems
curl -X PATCH http://localhost:8080/api/v1/admin/settings \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "thumbnail_max_dimension": 300,
    "thumbnail_jpeg_quality": 75
  }'
```

| Setting | Default | Low Memory | High Quality |
|---------|---------|------------|--------------|
| `thumbnail_max_dimension` | 400 | 200-300 | 500-600 |
| `thumbnail_jpeg_quality` | 85 | 70-75 | 90-95 |

### Preload Settings

In large libraries, preloading can consume significant memory:

```yaml
# codex.yaml - no preload settings, these are client-side
```

Preload settings are per-user (stored in browser). For memory-constrained servers, advise users to reduce preload pages in Reader Settings.

## Scanning Performance

### Initial Scan Strategy

For large libraries, consider:

1. **Scan in batches**: Add libraries one at a time
2. **Use normal mode first**: Deep scans are more resource-intensive
3. **Schedule during off-hours**: Use cron scheduling

```yaml
# Example: Library with cron schedule for nightly deep scans
# Configure via UI when creating library
```

### File System Considerations

| Storage Type | Performance | Recommendation |
|--------------|-------------|----------------|
| Local SSD | Excellent | Best performance |
| Local HDD | Good | Acceptable for small libraries |
| Network (NFS) | Variable | Use PostgreSQL, may need tuning |
| Network (SMB) | Poor | Not recommended for large libraries |

:::warning Network Storage
Network-mounted libraries may experience:
- Slower scanning (metadata extraction over network)
- File locking issues with SQLite
- Timeout errors for large files

Use PostgreSQL and increase timeouts for network storage.
:::

### Duplicate Detection

Hash-based duplicate detection is CPU and I/O intensive:

- **Enable for**: Libraries with potential duplicates
- **Disable for**: Clean, well-organized libraries
- **Schedule**: Run duplicate scans during low-usage periods

## Horizontal Scaling

For high-traffic deployments, run multiple Codex instances behind a load balancer.

### Requirements

- **PostgreSQL**: Required (SQLite doesn't support multiple writers)
- **Shared storage**: All instances must access the same media files
- **Shared thumbnails**: Mount thumbnail directory on shared storage or use object storage

### Architecture

```
                    ┌─────────────────┐
                    │  Load Balancer  │
                    │  (Nginx/HAProxy)│
                    └────────┬────────┘
                             │
           ┌─────────────────┼─────────────────┐
           │                 │                 │
           ▼                 ▼                 ▼
    ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
    │   Codex 1   │   │   Codex 2   │   │   Codex 3   │
    │  (workers)  │   │  (workers)  │   │  (workers)  │
    └──────┬──────┘   └──────┬──────┘   └──────┬──────┘
           │                 │                 │
           └─────────────────┼─────────────────┘
                             │
                    ┌────────┴────────┐
                    │   PostgreSQL    │
                    └─────────────────┘
                             │
                    ┌────────┴────────┐
                    │  Shared Storage │
                    │ (Media + Thumbs)│
                    └─────────────────┘
```

### Configuration for Scaling

```yaml
# Each instance
database:
  db_type: postgres
  postgres:
    host: postgres-service
    # ... connection settings

# Reduce workers per instance when scaling horizontally
task:
  worker_count: 2

# Limit concurrent scans across cluster
scanner:
  max_concurrent_scans: 1
```

### Load Balancer Configuration

```nginx
# nginx.conf
upstream codex_cluster {
    least_conn;  # Route to least busy server
    server codex1:8080;
    server codex2:8080;
    server codex3:8080;
}

server {
    listen 80;
    server_name library.example.com;

    location / {
        proxy_pass http://codex_cluster;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    # SSE requires long timeouts
    location /api/v1/sse {
        proxy_pass http://codex_cluster;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_read_timeout 86400s;
    }
}
```

## Monitoring Performance

### Key Metrics

Monitor these metrics for performance issues:

| Metric | Healthy Range | Action if High |
|--------|---------------|----------------|
| Response time | < 200ms | Check database, add caching |
| Memory usage | < 80% | Reduce workers, thumbnail size |
| CPU usage | < 70% | Reduce workers, scan concurrency |
| Database connections | < 80% of max | Increase pool or reduce workers |
| Disk I/O wait | < 20% | Use faster storage, reduce concurrency |

### Health Endpoint

```bash
# Basic health check
curl http://localhost:8080/health

# Detailed metrics (requires authentication)
curl http://localhost:8080/api/v1/metrics \
  -H "Authorization: Bearer $TOKEN"
```

### Slow Query Detection

For PostgreSQL, enable slow query logging:

```ini
# postgresql.conf
log_min_duration_statement = 100  # Log queries > 100ms
```

## Troubleshooting Performance

### Slow Library Scans

1. Check disk I/O: `iostat -x 1`
2. Reduce concurrent scans
3. Use normal mode instead of deep scan
4. Check for network storage latency

### High Memory Usage

1. Reduce worker count
2. Lower thumbnail dimensions
3. Check for memory leaks (restart periodically)
4. Increase system swap as safety net

### Slow Page Loads

1. Check database query times
2. Verify thumbnail cache is working
3. Check network latency to clients
4. Enable response compression in reverse proxy

### Database Connection Exhaustion

```
Error: too many connections
```

1. Reduce `max_connections` in Codex config
2. Increase PostgreSQL `max_connections`
3. Reduce worker count
4. Check for connection leaks (long-running queries)
