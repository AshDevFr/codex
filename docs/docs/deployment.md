---
sidebar_position: 6
---

# Deployment

Codex can be deployed in various ways depending on your needs. This guide covers the most common deployment scenarios.

## Docker Deployment

Docker is the easiest way to deploy Codex in production.

### Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/codex.git
cd codex

# Start with Docker Compose
docker compose up -d

# View logs
docker compose logs -f codex
```

### Docker Compose Configuration

The included `docker-compose.yml` provides:

- **PostgreSQL database** (port 5432)
- **Codex application** (port 8080)
- **Persistent volumes** for data
- **Health checks** and auto-restart

### Custom Configuration

Mount your config file:

```yaml
services:
  codex:
    volumes:
      - ./config/codex.yaml:/app/config/config.yaml:ro
      - ./data:/app/data
```

### Building Custom Images

```bash
# Production build
docker build -t codex:latest .

# Development build with hot reload
docker build -f Dockerfile.dev -t codex:dev .
```

### Multi-platform Builds

```bash
docker buildx build --platform linux/amd64,linux/arm64 -t codex:latest .
```

## Kubernetes Deployment

Codex is designed for horizontal scaling in Kubernetes.

### Basic Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codex
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
          value: "postgres-service"
        volumeMounts:
        - name: config
          mountPath: /app/config
      volumes:
      - name: config
        configMap:
          name: codex-config
```

### Stateless Architecture

Codex is stateless by design:
- No local file storage required
- All state in database
- Session tokens in JWT (no server-side sessions)
- Perfect for horizontal scaling

### Service Configuration

```yaml
apiVersion: v1
kind: Service
metadata:
  name: codex-service
spec:
  selector:
    app: codex
  ports:
  - port: 80
    targetPort: 8080
  type: LoadBalancer
```

### Ingress Example

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: codex-ingress
spec:
  rules:
  - host: codex.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: codex-service
            port:
              number: 80
```

## Systemd Service (Linux)

For traditional Linux deployments:

### Service File

Create `/etc/systemd/system/codex.service`:

```ini
[Unit]
Description=Codex Digital Library Server
After=network.target postgresql.service

[Service]
Type=simple
User=codex
WorkingDirectory=/opt/codex
ExecStart=/usr/local/bin/codex serve --config /opt/codex/codex.yaml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### Enable and Start

```bash
sudo systemctl daemon-reload
sudo systemctl enable codex
sudo systemctl start codex
sudo systemctl status codex
```

## Reverse Proxy Setup

### Nginx Configuration

```nginx
server {
    listen 80;
    server_name codex.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Caddy Configuration

```
codex.example.com {
    reverse_proxy localhost:8080
}
```

## Database Setup

### PostgreSQL

For production, use PostgreSQL:

```bash
# Create database
createdb codex

# Create user
createuser -P codex

# Run migrations (Codex handles this automatically on startup)
```

### SQLite

SQLite works for small deployments:

```bash
# Ensure directory exists
mkdir -p /var/lib/codex
chown codex:codex /var/lib/codex
```

## Security Considerations

### Firewall Rules

Only expose necessary ports:

```bash
# Allow HTTP/HTTPS
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# Block direct database access from internet
sudo ufw deny 5432/tcp
```

### SSL/TLS

Use a reverse proxy (Nginx, Caddy) with Let's Encrypt for SSL:

```bash
# Caddy automatically handles SSL
# Nginx with certbot:
sudo certbot --nginx -d codex.example.com
```

### Secrets Management

Never commit secrets to version control:

- Use environment variables
- Use Kubernetes secrets
- Use Docker secrets
- Use external secret managers (HashiCorp Vault, AWS Secrets Manager)

## Monitoring

### Health Checks

Codex exposes health endpoints:

```bash
# Basic health check
curl http://localhost:8080/health

# Readiness check
curl http://localhost:8080/ready
```

### Logging

Configure structured logging:

```yaml
logging:
  level: info
  file: /var/log/codex/codex.log
```

Use log aggregation tools:
- **Loki** for log aggregation
- **Prometheus** for metrics
- **Grafana** for visualization

## Backup Strategy

### Database Backups

**PostgreSQL:**
```bash
# Daily backup
pg_dump -U codex codex > backup_$(date +%Y%m%d).sql

# Restore
psql -U codex codex < backup_20240101.sql
```

**SQLite:**
```bash
# Simple copy
cp codex.db codex.db.backup
```

### Automated Backups

Set up cron jobs or Kubernetes CronJobs for automated backups.

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker logs codex

# Check configuration
docker exec codex cat /app/config/config.yaml
```

### Database Connection Issues

```bash
# Test PostgreSQL connection
psql -h localhost -U codex -d codex

# Check network connectivity
docker exec codex ping postgres
```

### Performance Issues

- Check database connection pool settings
- Monitor database query performance
- Review application logs for errors
- Consider horizontal scaling

## Next Steps

- Set up [authentication](./api#authentication)
- Configure [libraries](./getting-started)
- Explore the [API documentation](./api)

