---
---

# Installation

Codex can be installed using Docker (recommended), a pre-built binary, or built from source. Choose the method that best fits your environment.

## System Requirements

### Minimum Requirements

- **CPU**: 1 core (2+ recommended for scanning)
- **RAM**: 512 MB (1 GB+ recommended)
- **Storage**: Depends on your library size and thumbnail cache
- **OS**: Linux, macOS, or Windows

### Recommended for Production

- **CPU**: 2+ cores
- **RAM**: 2 GB+
- **Storage**: SSD for database and thumbnails
- **Database**: PostgreSQL 12+ (for multi-user environments)

## Option 1: Docker (Recommended)

Docker is the easiest way to run Codex, especially with PostgreSQL.

### Quick Start with Docker Compose

```bash
# Clone the repository
git clone https://github.com/AshDevFr/codex.git
cd codex

# Start Codex with PostgreSQL
docker compose --profile prod up -d

# View logs
docker compose logs -f codex

# Create your admin user
docker compose exec codex codex seed --config /app/config/config.docker.yaml
```

Access Codex at `http://localhost:8080`

### Docker Compose Services

The included `docker-compose.yml` provides several profiles:

| Profile | Services | Use Case |
|---------|----------|----------|
| `prod` | PostgreSQL, Codex, Mailhog | Production deployment |
| `dev` | PostgreSQL, Codex (hot reload), Worker, Frontend, Mailhog | Development |
| `test` | PostgreSQL (test instance) | Testing |

### Custom Docker Setup

Run Codex with your own configuration:

```bash
docker run -d \
  --name codex \
  -p 8080:8080 \
  -v /path/to/your/config:/app/config:ro \
  -v /path/to/your/data:/app/data \
  -v /path/to/your/library:/library:ro \
  codex:latest \
  codex serve --config /app/config/codex.yaml
```

### Volume Mounts

| Container Path | Purpose |
|---------------|---------|
| `/app/config` | Configuration files (read-only recommended) |
| `/app/data` | Database (SQLite), thumbnails, and cache |
| `/library` | Your media files (read-only recommended) |

### Environment Variables

Override configuration via environment variables:

```bash
docker run -d \
  -e CODEX_DATABASE_DB_TYPE=postgres \
  -e CODEX_DATABASE_POSTGRES_HOST=db.example.com \
  -e CODEX_DATABASE_POSTGRES_PASSWORD=secret \
  -e CODEX_AUTH_JWT_SECRET=your-secure-secret \
  codex:latest
```

See [Configuration](./configuration#environment-variables) for all available variables.

## Option 2: Pre-built Binary

Download a pre-built binary for your platform.

### Download

Download the latest release from the [releases page](https://github.com/AshDevFr/codex/releases).

Available platforms:
- Linux x86_64
- Linux ARM64 (Raspberry Pi 4+)
- macOS x86_64 (Intel)
- macOS ARM64 (Apple Silicon)
- Windows x86_64

### Linux/macOS Installation

```bash
# Download and extract
tar -xzf codex-x.x.x-linux-x86_64.tar.gz

# Make executable
chmod +x codex

# Move to PATH (optional)
sudo mv codex /usr/local/bin/

# Verify installation
codex --version
```

### Windows Installation

1. Download the `.exe` file from releases
2. Extract to a folder (e.g., `C:\Program Files\Codex\`)
3. Add the folder to your system PATH
4. Open a new terminal and verify: `codex --version`

### Create Configuration

Create a minimal `codex.yaml` file:

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db

application:
  host: 127.0.0.1
  port: 8080

auth:
  jwt_secret: "change-this-to-a-secure-random-string"
```

### Run Codex

```bash
# Start the server
codex serve --config codex.yaml

# In another terminal, create admin user
codex seed --config codex.yaml
```

## Option 3: Build from Source

Build Codex from source for custom configurations or development.

### Prerequisites

- **Rust**: 1.75+ (install via [rustup](https://rustup.rs/))
- **Node.js**: 18+ (for frontend)
- **PostgreSQL**: 12+ (optional, for PostgreSQL builds)

### Clone and Build

```bash
# Clone repository
git clone https://github.com/AshDevFr/codex.git
cd codex

# Build release binary
cargo build --release

# Binary location: target/release/codex
```

### Build Options

```bash
# Build without CBR support (no proprietary dependencies)
cargo build --release --no-default-features

# Build with all features
cargo build --release --all-features
```

### Build Frontend

```bash
cd web
npm install
npm run build
```

The frontend build will be embedded in the binary or served from `web/dist`.

## Post-Installation Setup

### 1. Create Admin User

After starting Codex, create your admin account:

```bash
codex seed --config codex.yaml
```

Follow the interactive prompts to set:
- Username
- Email
- Password

### 2. Access the Web Interface

Open your browser and navigate to:
- **Default**: `http://localhost:8080`
- **Docker**: `http://localhost:8080`

### 3. Create Your First Library

1. Log in with your admin credentials
2. Navigate to Settings > Libraries
3. Click "Add Library"
4. Enter the library name and path
5. Configure scanning options
6. Start the initial scan

## Running as a Service

### Systemd (Linux)

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

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/codex/data /var/log/codex

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable codex
sudo systemctl start codex
sudo systemctl status codex
```

### Docker with Auto-Restart

```bash
docker run -d \
  --name codex \
  --restart unless-stopped \
  -p 8080:8080 \
  -v codex_data:/app/data \
  codex:latest
```

## Upgrading

### Docker

```bash
# Pull latest image
docker compose pull

# Restart with new image
docker compose --profile prod up -d

# Migrations run automatically on startup
```

### Binary

1. Download the new release
2. Stop the running Codex service
3. Replace the binary
4. Start Codex (migrations run automatically)

```bash
sudo systemctl stop codex
sudo cp codex-new /usr/local/bin/codex
sudo systemctl start codex
```

## Verifying Installation

### Health Check

```bash
curl http://localhost:8080/health
```

Expected response:
```json
{"status":"ok"}
```

### API Access

```bash
# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"your-password"}'
```

## Next Steps

- [Configure your server](./configuration)
- [Set up your first library](./getting-started)
- [Deploy to production](./deployment)
