---
---

# Deployment

This guide covers deploying Codex in various environments, from simple homelab setups to production Kubernetes clusters. Each deployment option has specific considerations and limitations you should understand.

## Deployment Overview

| Deployment Type | Database | Best For | Scaling |
|-----------------|----------|----------|---------|
| Docker Compose | PostgreSQL | Production, Multi-user | Single host |
| Binary + SQLite | SQLite | Homelab, Single-user | None |
| Kubernetes | PostgreSQL | Enterprise, High Availability | Horizontal |
| Systemd Service | Either | Dedicated server | None |

## Understanding Deployment Limitations

Before choosing a deployment strategy, understand these important architectural considerations:

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

## Docker Deployment

Docker is the recommended deployment method for most users.

### Quick Start

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

### Docker Compose Configuration

The provided `docker-compose.yml` includes multiple profiles:

#### Production Profile

```bash
docker compose --profile prod up -d
```

Services started:
- **postgres** - PostgreSQL 16 database (port 5432)
- **codex** - Codex server with embedded workers (port 8080)
- **mailhog** - Email testing interface (ports 1025, 8025)

#### Development Profile

```bash
docker compose --profile dev up -d
```

Services started:
- **postgres** - PostgreSQL database
- **codex-dev** - Backend with hot reload (port 8080)
- **codex-dev-worker** - Dedicated worker container
- **frontend-dev** - Vite dev server (port 5173)
- **mailhog** - Email testing

### Custom Docker Configuration

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

### Volume Considerations

| Volume | Purpose | Permissions |
|--------|---------|-------------|
| `/app/config` | Configuration files | Read-only |
| `/app/data` | Database (SQLite), thumbnails | Read-write |
| `/library` | Media files | Read-only (recommended) |
| `/app/data/thumbnails` | Thumbnail cache | Read-write |

:::tip Library Permissions
Mount your media library as read-only (`:ro`) to prevent accidental modifications. Codex only needs read access to your files.
:::

### Health Checks

Docker health checks are configured automatically:

```yaml
healthcheck:
  test: ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"]
  interval: 10s
  timeout: 5s
  retries: 5
```

## Kubernetes Deployment

Codex is designed for horizontal scaling in Kubernetes.

### Architecture

```
                    ┌───────────────────────┐
                    │    Load Balancer      │
                    └───────────┬───────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
        ▼                       ▼                       ▼
┌───────────────┐       ┌───────────────┐       ┌───────────────┐
│  Codex Pod 1  │       │  Codex Pod 2  │       │  Codex Pod N  │
│  + Workers    │       │  + Workers    │       │  + Workers    │
└───────┬───────┘       └───────┬───────┘       └───────┬───────┘
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │      PostgreSQL       │
                    │   (Single Instance)   │
                    └───────────────────────┘
```

### Deployment Manifest

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codex
  labels:
    app: codex
spec:
  replicas: 3
  selector:
    matchLabels:
      app: codex
  template:
    metadata:
      labels:
        app: codex
    spec:
      containers:
        - name: codex
          image: codex:latest
          ports:
            - containerPort: 8080
          env:
            - name: CODEX_DATABASE_DB_TYPE
              value: "postgres"
            - name: CODEX_DATABASE_POSTGRES_HOST
              valueFrom:
                configMapKeyRef:
                  name: codex-config
                  key: postgres-host
            - name: CODEX_DATABASE_POSTGRES_PASSWORD
              valueFrom:
                secretKeyRef:
                  name: codex-secrets
                  key: postgres-password
            - name: CODEX_AUTH_JWT_SECRET
              valueFrom:
                secretKeyRef:
                  name: codex-secrets
                  key: jwt-secret
          volumeMounts:
            - name: library
              mountPath: /library
              readOnly: true
            - name: thumbnails
              mountPath: /app/data/thumbnails
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 5
          resources:
            requests:
              memory: "256Mi"
              cpu: "250m"
            limits:
              memory: "1Gi"
              cpu: "1000m"
      volumes:
        - name: library
          persistentVolumeClaim:
            claimName: library-pvc
        - name: thumbnails
          persistentVolumeClaim:
            claimName: thumbnails-pvc
```

### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: codex
spec:
  selector:
    app: codex
  ports:
    - port: 80
      targetPort: 8080
  type: ClusterIP
```

### Ingress

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: codex
  annotations:
    # For SSE support - disable buffering
    nginx.ingress.kubernetes.io/proxy-buffering: "off"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
spec:
  rules:
    - host: codex.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: codex
                port:
                  number: 80
  tls:
    - hosts:
        - codex.example.com
      secretName: codex-tls
```

### Kubernetes Considerations

#### Shared Storage

All pods need access to:
- **Media library**: ReadOnlyMany (ROX) or ReadWriteMany (RWX) PVC
- **Thumbnails**: ReadWriteMany (RWX) PVC for shared cache

Storage options:
- NFS
- CephFS
- Cloud storage (EFS, Azure Files, etc.)

#### Database Requirements

- PostgreSQL is **required** for multi-replica deployments
- Use a managed PostgreSQL service or StatefulSet
- Ensure connection pooling for many replicas

#### Session Handling

- JWT tokens are stateless (no sticky sessions needed)
- Any pod can handle any request
- Load balancing works without session affinity

### Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: codex-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: codex
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

## Systemd Service (Linux)

For traditional Linux server deployments.

### Service File

Create `/etc/systemd/system/codex.service`:

```ini
[Unit]
Description=Codex Digital Library Server
After=network.target postgresql.service

[Service]
Type=simple
User=codex
Group=codex
WorkingDirectory=/opt/codex
ExecStart=/usr/local/bin/codex serve --config /opt/codex/codex.yaml
Restart=always
RestartSec=10

# Graceful shutdown timeout
TimeoutStopSec=30

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/codex/data /var/log/codex

[Install]
WantedBy=multi-user.target
```

### Enable and Start

```bash
# Create user
sudo useradd -r -s /bin/false codex

# Create directories
sudo mkdir -p /opt/codex/data /var/log/codex
sudo chown -R codex:codex /opt/codex /var/log/codex

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable codex
sudo systemctl start codex

# Check status
sudo systemctl status codex
journalctl -u codex -f
```

## Reverse Proxy Setup

### Nginx

```nginx
upstream codex {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 80;
    server_name codex.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name codex.example.com;

    ssl_certificate /etc/letsencrypt/live/codex.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/codex.example.com/privkey.pem;

    # SSE support - critical for real-time updates
    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 86400s;
    proxy_send_timeout 86400s;

    location / {
        proxy_pass http://codex;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Connection "";
    }

    # SSE endpoints - ensure no buffering
    location ~ ^/api/v1/(events|tasks|scans)/stream {
        proxy_pass http://codex;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Connection "";

        # Critical for SSE
        proxy_buffering off;
        proxy_cache off;
        chunked_transfer_encoding off;
    }

    # Increase body size for file uploads
    client_max_body_size 100M;
}
```

### Caddy

```
codex.example.com {
    reverse_proxy localhost:8080 {
        # Disable buffering for SSE
        flush_interval -1
    }
}
```

### Traefik

```yaml
# Dynamic configuration
http:
  routers:
    codex:
      rule: "Host(`codex.example.com`)"
      service: codex
      tls:
        certResolver: letsencrypt

  services:
    codex:
      loadBalancer:
        servers:
          - url: "http://codex:8080"
        # Important for SSE
        passHostHeader: true
```

## Database Setup

### PostgreSQL

```bash
# Create database and user
sudo -u postgres psql

CREATE DATABASE codex;
CREATE USER codex WITH ENCRYPTED PASSWORD 'your-secure-password';
GRANT ALL PRIVILEGES ON DATABASE codex TO codex;
\q
```

Codex runs migrations automatically on startup.

### SQLite

SQLite databases are created automatically. Ensure the data directory is writable:

```bash
mkdir -p /opt/codex/data
chown codex:codex /opt/codex/data
```

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

## Monitoring

### Health Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /health` | Basic health check | `{"status":"ok"}` |
| `GET /api/v1/metrics` | Server metrics | JSON metrics data |

### Logging

Configure structured logging:

```yaml
logging:
  level: info
  file: /var/log/codex/codex.log
```

### Prometheus Metrics

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'codex'
    static_configs:
      - targets: ['codex:8080']
    metrics_path: '/api/v1/metrics'
```

## Backup Strategy

### PostgreSQL

```bash
# Manual backup
pg_dump -U codex codex > backup_$(date +%Y%m%d).sql

# Restore
psql -U codex codex < backup_20240101.sql
```

### SQLite

```bash
# Simple copy (ensure Codex is stopped or use WAL mode)
cp /opt/codex/data/codex.db /backup/codex_$(date +%Y%m%d).db
```

### Automated Backups

```bash
# Cron job for daily backups
0 2 * * * pg_dump -U codex codex | gzip > /backup/codex_$(date +\%Y\%m\%d).sql.gz
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

### SSE Not Working

1. Check proxy buffering is disabled
2. Verify timeout settings
3. Check authentication token is valid
4. Review browser console for connection errors

See the [Troubleshooting Guide](./troubleshooting) for more detailed solutions.

## Next Steps

- [Configure your server](./configuration)
- [Set up libraries](./libraries)
- [Explore the API](./api)
