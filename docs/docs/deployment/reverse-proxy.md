---
sidebar_position: 5
---

# Reverse Proxy Setup

Configure a reverse proxy for SSL termination and production deployments.

## SSE Requirements

Server-Sent Events (SSE) require special proxy configuration:
- **Disable buffering** - Events must stream immediately
- **Long timeouts** - SSE connections stay open indefinitely
- **No caching** - Event streams cannot be cached

## Nginx

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

### SSL with Let's Encrypt

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d codex.example.com

# Auto-renewal is configured automatically
```

## Caddy

Caddy automatically handles SSL certificates.

```
codex.example.com {
    reverse_proxy localhost:8080 {
        # Disable buffering for SSE
        flush_interval -1
    }
}
```

### With Basic Auth (optional)

```
codex.example.com {
    basicauth /admin/* {
        admin $2a$14$... # bcrypt hash
    }
    reverse_proxy localhost:8080 {
        flush_interval -1
    }
}
```

## Traefik

### Docker Labels

```yaml
services:
  codex:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.codex.rule=Host(`codex.example.com`)"
      - "traefik.http.routers.codex.tls=true"
      - "traefik.http.routers.codex.tls.certresolver=letsencrypt"
      - "traefik.http.services.codex.loadbalancer.server.port=8080"
      # SSE support
      - "traefik.http.services.codex.loadbalancer.responseforwarding.flushinterval=-1"
```

### File Configuration

```yaml
# traefik-dynamic.yml
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
        responseForwarding:
          flushInterval: "-1"
```

## Apache

```apache
<VirtualHost *:443>
    ServerName codex.example.com

    SSLEngine on
    SSLCertificateFile /etc/letsencrypt/live/codex.example.com/fullchain.pem
    SSLCertificateKeyFile /etc/letsencrypt/live/codex.example.com/privkey.pem

    ProxyPreserveHost On
    ProxyPass / http://127.0.0.1:8080/
    ProxyPassReverse / http://127.0.0.1:8080/

    # SSE support
    ProxyTimeout 86400
    SetEnv proxy-nokeepalive 1
    SetEnv force-proxy-request-1.0 1
    SetEnv proxy-initial-not-pooled 1

    # Disable buffering for SSE endpoints
    <LocationMatch "^/api/v1/(events|tasks|scans)/stream">
        SetOutputFilter NONE
        SetEnv no-gzip 1
    </LocationMatch>
</VirtualHost>
```

## HAProxy

```haproxy
frontend https
    bind *:443 ssl crt /etc/haproxy/certs/codex.pem
    default_backend codex

backend codex
    server codex 127.0.0.1:8080 check

    # SSE support
    timeout server 86400s
    timeout tunnel 86400s
    no option httpclose
    option http-server-close
```

## Common Issues

### SSE Not Working

1. **Check proxy buffering** - Must be disabled
2. **Check timeouts** - Must be long (24h+)
3. **Check connection headers** - Don't close connections prematurely

Test SSE directly:
```bash
curl -N -H "Authorization: Bearer $TOKEN" \
  https://codex.example.com/api/v1/events/stream
```

### 502 Bad Gateway

1. Check Codex is running: `curl http://localhost:8080/health`
2. Check firewall rules
3. Check proxy upstream configuration

### Slow Response

1. Enable keepalive connections
2. Check proxy_buffering setting
3. Review timeout values
