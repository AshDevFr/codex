---
sidebar_position: 7
---

# Security & Operations

Security best practices, monitoring, and backup strategies for production deployments.

## Security Best Practices

### Network Security

```bash
# Expose only necessary ports
# HTTP/HTTPS to internet, database internal only
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw deny 5432/tcp  # Block direct PostgreSQL access
```

### SSL/TLS

Always use HTTPS in production:

```bash
# Using certbot with Nginx
sudo certbot --nginx -d codex.example.com

# Caddy handles SSL automatically
```

### Secrets Management

Never commit secrets to version control:

1. **Environment variables** - Simple, use for Docker/containers
2. **Kubernetes Secrets** - For K8s deployments
3. **HashiCorp Vault** - Enterprise secret management
4. **AWS Secrets Manager** / **Azure Key Vault** - Cloud deployments

### Container Security

```yaml
# Run as non-root
securityContext:
  runAsNonRoot: true
  runAsUser: 1000
  readOnlyRootFilesystem: true
```

### JWT Secret

Generate a strong JWT secret:

```bash
openssl rand -base64 32
```

Store securely and never expose in logs or version control.

## Monitoring

### Health Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /health` | Basic health check | `{"status":"ok"}` |
| `GET /api/v1/metrics` | Server metrics | JSON metrics data |

### Health Check Script

```bash
#!/bin/bash
response=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health)
if [ "$response" != "200" ]; then
    echo "Codex health check failed: $response"
    exit 1
fi
```

### Logging

Configure structured logging:

```yaml
logging:
  level: info
  file: /var/log/codex/codex.log
```

Log levels:
- `error` - Only errors
- `warn` - Warnings and errors
- `info` - Normal operation (recommended)
- `debug` - Detailed debugging
- `trace` - Very verbose

### Prometheus Metrics

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'codex'
    static_configs:
      - targets: ['codex:8080']
    metrics_path: '/api/v1/metrics'
```

### Alerting Example

```yaml
# alertmanager rules
groups:
  - name: codex
    rules:
      - alert: CodexDown
        expr: up{job="codex"} == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Codex is down"
```

## Backup Strategy

### What to Backup

| Data | Location | Priority | Recovery |
|------|----------|----------|----------|
| Database | PostgreSQL/SQLite | Critical | Required for operation |
| Configuration | `codex.yaml` | Critical | Required for operation |
| Thumbnails | `data/thumbnails` | Low | Can regenerate via scan |
| Uploads | `data/uploads` | Medium | User-uploaded covers |
| Media files | Library paths | External | Managed outside Codex |

### PostgreSQL Backups

#### Manual Backup

```bash
# Plain SQL backup
pg_dump -U codex codex > backup_$(date +%Y%m%d).sql

# Compressed backup (recommended)
pg_dump -U codex codex | gzip > backup_$(date +%Y%m%d).sql.gz

# Custom format (allows parallel restore)
pg_dump -U codex -Fc codex > backup_$(date +%Y%m%d).dump
```

#### Restore from Backup

```bash
# From plain SQL
psql -U codex codex < backup_20240101.sql

# From compressed SQL
gunzip -c backup_20240101.sql.gz | psql -U codex codex

# From custom format (parallel restore)
pg_restore -U codex -d codex -j 4 backup_20240101.dump
```

#### Docker Backup

```bash
# Backup from container
docker exec codex-postgres pg_dump -U codex codex | gzip > backup_$(date +%Y%m%d).sql.gz

# Restore to container
gunzip -c backup_20240101.sql.gz | docker exec -i codex-postgres psql -U codex codex
```

### SQLite Backups

#### Safe Backup Methods

:::warning SQLite Backup Safety
Never copy a SQLite database while Codex is writing to it. Use one of these safe methods:
:::

**Method 1: Stop Codex (safest)**
```bash
# Stop Codex
sudo systemctl stop codex

# Copy database and WAL files
cp /opt/codex/data/codex.db /backup/codex_$(date +%Y%m%d).db
cp /opt/codex/data/codex.db-wal /backup/codex_$(date +%Y%m%d).db-wal 2>/dev/null || true
cp /opt/codex/data/codex.db-shm /backup/codex_$(date +%Y%m%d).db-shm 2>/dev/null || true

# Restart Codex
sudo systemctl start codex
```

**Method 2: SQLite Online Backup (no downtime)**
```bash
# Uses SQLite's backup API - safe during operation
sqlite3 /opt/codex/data/codex.db ".backup '/backup/codex_$(date +%Y%m%d).db'"
```

**Method 3: VACUUM INTO (creates standalone copy)**
```bash
# Creates a fresh, compacted backup
sqlite3 /opt/codex/data/codex.db "VACUUM INTO '/backup/codex_$(date +%Y%m%d).db'"
```

#### Restore SQLite

```bash
# Stop Codex
sudo systemctl stop codex

# Replace database
cp /backup/codex_20240101.db /opt/codex/data/codex.db

# Remove WAL files (will be recreated)
rm -f /opt/codex/data/codex.db-wal /opt/codex/data/codex.db-shm

# Start Codex
sudo systemctl start codex
```

### Configuration Backup

Store configuration in version control:

```bash
# Initialize backup repository
mkdir -p /backup/codex-config
cd /backup/codex-config
git init

# Copy and commit configuration
cp /opt/codex/codex.yaml .
git add codex.yaml
git commit -m "Backup $(date +%Y-%m-%d)"

# Push to remote (optional)
git remote add origin git@github.com:youruser/codex-config.git
git push -u origin main
```

:::danger Secrets in Configuration
Never commit secrets (JWT secret, database passwords) to version control. Use environment variables for sensitive values:

```yaml
# codex.yaml - reference environment variables
auth:
  jwt_secret: ${JWT_SECRET}
database:
  postgres:
    password: ${DB_PASSWORD}
```
:::

### Automated Backups

#### Cron-based Backup Script

Create `/opt/codex/backup.sh`:

```bash
#!/bin/bash
set -e

BACKUP_DIR="/backup/codex"
RETENTION_DAYS=30
DATE=$(date +%Y%m%d)

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Backup PostgreSQL
pg_dump -U codex codex | gzip > "$BACKUP_DIR/db_$DATE.sql.gz"

# Backup configuration
cp /opt/codex/codex.yaml "$BACKUP_DIR/config_$DATE.yaml"

# Backup uploads (user-uploaded covers)
tar -czf "$BACKUP_DIR/uploads_$DATE.tar.gz" -C /opt/codex/data uploads 2>/dev/null || true

# Remove old backups
find "$BACKUP_DIR" -name "db_*.sql.gz" -mtime +$RETENTION_DAYS -delete
find "$BACKUP_DIR" -name "config_*.yaml" -mtime +$RETENTION_DAYS -delete
find "$BACKUP_DIR" -name "uploads_*.tar.gz" -mtime +$RETENTION_DAYS -delete

echo "Backup completed: $DATE"
```

Schedule with cron:

```bash
# /etc/cron.d/codex-backup
# Run daily at 2 AM
0 2 * * * root /opt/codex/backup.sh >> /var/log/codex-backup.log 2>&1
```

#### Docker Compose Backup

Add a backup service to your `docker-compose.yml`:

```yaml
services:
  backup:
    image: postgres:16
    volumes:
      - ./backups:/backups
      - codex_data:/data:ro
    environment:
      PGPASSWORD: your-password
    entrypoint: /bin/sh
    command: >
      -c "pg_dump -h postgres -U codex codex | gzip > /backups/db_$$(date +%Y%m%d).sql.gz &&
          find /backups -name 'db_*.sql.gz' -mtime +30 -delete"
    depends_on:
      - postgres
    profiles:
      - backup
```

Run backup manually:
```bash
docker compose --profile backup run --rm backup
```

### Backup Verification

**Always verify backups can be restored!**

#### PostgreSQL Verification

```bash
# Create test database
createdb codex_backup_test

# Restore backup
gunzip -c backup_20240101.sql.gz | psql -U codex codex_backup_test

# Verify data integrity
psql -U codex codex_backup_test << 'EOF'
SELECT 'books' as table_name, COUNT(*) as count FROM books
UNION ALL SELECT 'series', COUNT(*) FROM series
UNION ALL SELECT 'libraries', COUNT(*) FROM libraries
UNION ALL SELECT 'users', COUNT(*) FROM users;
EOF

# Cleanup test database
dropdb codex_backup_test
```

#### SQLite Verification

```bash
# Check database integrity
sqlite3 /backup/codex_20240101.db "PRAGMA integrity_check"

# Verify row counts
sqlite3 /backup/codex_20240101.db << 'EOF'
SELECT 'books' as table_name, COUNT(*) as count FROM books
UNION ALL SELECT 'series', COUNT(*) FROM series
UNION ALL SELECT 'libraries', COUNT(*) FROM libraries;
EOF
```

### Off-site Backup

For disaster recovery, store backups off-site:

#### AWS S3

```bash
# Install AWS CLI
pip install awscli

# Upload backup
aws s3 cp /backup/codex/db_20240101.sql.gz s3://your-bucket/codex-backups/

# Sync backup directory
aws s3 sync /backup/codex s3://your-bucket/codex-backups/ --delete
```

#### Restic (encrypted backups)

```bash
# Initialize repository
restic init --repo s3:s3.amazonaws.com/your-bucket/codex-backups

# Backup
restic backup /backup/codex --repo s3:s3.amazonaws.com/your-bucket/codex-backups

# Prune old backups (keep 30 daily, 12 monthly)
restic forget --keep-daily 30 --keep-monthly 12 --prune
```

## Disaster Recovery

### Recovery Checklist

1. **Assess the situation**
   - What failed? (Server, storage, database)
   - What's the most recent backup?
   - Is media storage accessible?

2. **Deploy fresh Codex instance**
   ```bash
   # Docker
   docker compose up -d

   # Or systemd
   sudo systemctl start codex
   ```

3. **Restore database from backup**
   ```bash
   # PostgreSQL
   gunzip -c backup_latest.sql.gz | psql -U codex codex

   # SQLite
   cp backup_latest.db /opt/codex/data/codex.db
   ```

4. **Restore configuration**
   ```bash
   cp /backup/codex-config/codex.yaml /opt/codex/
   ```

5. **Restore uploads (if backed up)**
   ```bash
   tar -xzf uploads_latest.tar.gz -C /opt/codex/data/
   ```

6. **Verify health**
   ```bash
   curl http://localhost:8080/health
   ```

7. **Regenerate thumbnails (optional)**
   - Trigger a library scan via the UI
   - Or wait for scheduled scan

8. **Verify media access**
   - Check library paths are mounted
   - Test opening a book

### Recovery Time Objectives (RTO)

| Component | Recovery Method | Typical Time |
|-----------|-----------------|--------------|
| Application | Deploy container/binary | 5 minutes |
| Database (small) | Restore < 1GB backup | 5-10 minutes |
| Database (large) | Restore > 10GB backup | 30-60 minutes |
| Configuration | Copy from backup/VCS | 2 minutes |
| Thumbnails | Regenerate via scan | Hours (varies by library size) |
| Uploads | Restore from backup | 5-10 minutes |

### Recovery Point Objective (RPO)

Your RPO depends on backup frequency:

| Backup Schedule | Maximum Data Loss |
|-----------------|-------------------|
| Hourly | Up to 1 hour |
| Daily | Up to 24 hours |
| Weekly | Up to 7 days |

:::tip
For critical deployments, consider PostgreSQL streaming replication for near-zero RPO.
:::

## Maintenance

### Log Rotation

```bash
# /etc/logrotate.d/codex
/var/log/codex/*.log {
    daily
    rotate 14
    compress
    delaycompress
    missingok
    notifempty
    create 640 codex codex
}
```

### Database Maintenance

#### PostgreSQL

```bash
# Vacuum (reclaim space)
psql -U codex codex -c "VACUUM ANALYZE"

# Reindex (optimize queries)
psql -U codex codex -c "REINDEX DATABASE codex"
```

#### SQLite

```bash
# Vacuum (must stop Codex first)
sqlite3 /opt/codex/data/codex.db "VACUUM"
```

### Thumbnail Cleanup

Orphaned thumbnails can accumulate:

```bash
# Via API (requires admin)
curl -X POST http://localhost:8080/api/v1/maintenance/cleanup-thumbnails \
  -H "Authorization: Bearer $TOKEN"
```
