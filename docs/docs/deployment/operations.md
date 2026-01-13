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

| Data | Location | Priority |
|------|----------|----------|
| Database | PostgreSQL/SQLite | Critical |
| Configuration | `codex.yaml` | Critical |
| Thumbnails | `/app/data/thumbnails` | Low (can regenerate) |
| Media files | Library paths | External to Codex |

### PostgreSQL Backups

```bash
# Manual backup
pg_dump -U codex codex > backup_$(date +%Y%m%d).sql

# Compressed
pg_dump -U codex codex | gzip > backup_$(date +%Y%m%d).sql.gz

# Restore
psql -U codex codex < backup_20240101.sql
```

### SQLite Backups

```bash
# Simple copy (ensure Codex is stopped or use WAL mode)
cp /opt/codex/data/codex.db /backup/codex_$(date +%Y%m%d).db
```

### Automated Backups

```bash
# Cron job for daily backups
# /etc/cron.d/codex-backup
0 2 * * * postgres pg_dump -U codex codex | gzip > /backup/codex_$(date +\%Y\%m\%d).sql.gz

# Retention (keep 30 days)
0 3 * * * find /backup -name "codex_*.sql.gz" -mtime +30 -delete
```

### Backup Verification

Regularly test restores:

```bash
# Create test database
createdb codex_test

# Restore backup
psql -U codex codex_test < backup.sql

# Verify
psql -U codex codex_test -c "SELECT COUNT(*) FROM books"

# Cleanup
dropdb codex_test
```

## Disaster Recovery

### Recovery Steps

1. **Deploy fresh Codex instance**
2. **Restore database from backup**
3. **Verify configuration**
4. **Run health checks**
5. **Trigger library scan** (optional, to verify file access)

### Recovery Time Objective (RTO)

| Component | Recovery Method | Typical RTO |
|-----------|-----------------|-------------|
| Database | Restore from backup | 5-30 minutes |
| Configuration | Version control | Minutes |
| Thumbnails | Regenerate via scan | Hours |

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
