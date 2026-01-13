---
sidebar_position: 4
---

# Systemd Service

For traditional Linux server deployments without containers.

## Prerequisites

- Linux server with systemd
- Codex binary downloaded
- PostgreSQL or SQLite configured

## Installation

### Download Binary

```bash
# Download latest release
curl -LO https://github.com/AshDevFr/codex/releases/latest/download/codex-linux-amd64.tar.gz

# Extract
tar xzf codex-linux-amd64.tar.gz

# Move to system location
sudo mv codex /usr/local/bin/
sudo chmod +x /usr/local/bin/codex
```

### Create User and Directories

```bash
# Create dedicated user
sudo useradd -r -s /bin/false codex

# Create directories
sudo mkdir -p /opt/codex/data /var/log/codex

# Set ownership
sudo chown -R codex:codex /opt/codex /var/log/codex
```

### Create Configuration

```bash
sudo cp codex.example.yaml /opt/codex/codex.yaml
sudo chown codex:codex /opt/codex/codex.yaml
sudo chmod 600 /opt/codex/codex.yaml
```

Edit `/opt/codex/codex.yaml` with your settings.

## Service File

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

## Enable and Start

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable on boot
sudo systemctl enable codex

# Start service
sudo systemctl start codex

# Check status
sudo systemctl status codex
```

## View Logs

```bash
# Follow logs
journalctl -u codex -f

# Last 100 lines
journalctl -u codex -n 100

# Since specific time
journalctl -u codex --since "1 hour ago"
```

## Managing the Service

```bash
# Stop
sudo systemctl stop codex

# Restart
sudo systemctl restart codex

# Reload (if supported)
sudo systemctl reload codex
```

## Updating

```bash
# Stop service
sudo systemctl stop codex

# Download and replace binary
curl -LO https://github.com/AshDevFr/codex/releases/latest/download/codex-linux-amd64.tar.gz
tar xzf codex-linux-amd64.tar.gz
sudo mv codex /usr/local/bin/

# Start service (migrations run automatically)
sudo systemctl start codex

# Check logs
journalctl -u codex -f
```

## Library Access

Ensure the codex user can read your media libraries:

```bash
# Option 1: Add codex to media group
sudo usermod -aG media codex

# Option 2: Set ACLs
sudo setfacl -R -m u:codex:rx /path/to/library
```

## Security Hardening

The service file includes basic hardening. Additional options:

```ini
[Service]
# ... existing options ...

# Additional security
PrivateTmp=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true
MemoryDenyWriteExecute=true

# Restrict network (if not needed)
# PrivateNetwork=true

# Restrict system calls
SystemCallFilter=@system-service
SystemCallErrorNumber=EPERM
```

## With PostgreSQL

If using PostgreSQL on the same server:

```ini
[Unit]
Description=Codex Digital Library Server
After=network.target postgresql.service
Requires=postgresql.service
```

## Troubleshooting

### Service Fails to Start

```bash
# Check detailed status
systemctl status codex

# Check logs
journalctl -u codex --no-pager

# Verify binary works
sudo -u codex /usr/local/bin/codex --version
```

### Permission Denied

```bash
# Check file ownership
ls -la /opt/codex/
ls -la /var/log/codex/

# Check library access
sudo -u codex ls /path/to/library
```

### Database Connection Issues

```bash
# Test PostgreSQL connection
sudo -u codex psql -h localhost -U codex -d codex -c "SELECT 1"
```
