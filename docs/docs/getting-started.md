---
---

# Getting Started

This guide walks you through setting up Codex and creating your first library.

## Prerequisites

Before starting, ensure you have:

- **Docker** (recommended) or a pre-built Codex binary
- A folder containing your digital comics, manga, or ebooks
- 10 minutes for initial setup

## Quick Start with Docker

The fastest way to get started:

```bash
# Clone the repository
git clone https://github.com/AshDevFr/codex.git
cd codex

# Start Codex with PostgreSQL
docker compose --profile prod up -d

# Wait for services to be healthy
docker compose logs -f codex
# Press Ctrl+C when you see "Listening on 0.0.0.0:8080"

# Create your admin account
docker compose exec codex codex seed --config /app/config/config.docker.yaml
```

Follow the prompts to create your admin username and password.

**Access Codex**: Open `http://localhost:8080` in your browser.

## Quick Start with Binary

If you prefer running without Docker:

### 1. Download Codex

Download the latest release for your platform from [GitHub Releases](https://github.com/AshDevFr/codex/releases).

### 2. Create Configuration

Create a `codex.yaml` file:

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db
    pragmas:
      journal_mode: WAL
      synchronous: NORMAL

application:
  host: 127.0.0.1
  port: 8080

auth:
  jwt_secret: "change-this-to-a-random-string"
```

:::tip Generate a Secure Secret
```bash
openssl rand -base64 32
```
:::

### 3. Start Codex

```bash
# Start the server
./codex serve --config codex.yaml
```

### 4. Create Admin User

In another terminal:

```bash
./codex seed --config codex.yaml
```

**Access Codex**: Open `http://localhost:8080` in your browser.

## First Login

1. Open Codex in your browser
2. Enter your admin username and password
3. You'll see the empty library dashboard

## Creating Your First Library

### Via Web Interface

1. Click **Settings** (gear icon) in the sidebar
2. Select **Libraries**
3. Click **Add Library**
4. Fill in the details:
   - **Name**: A descriptive name (e.g., "My Comics")
   - **Path**: The folder path containing your files
     - Docker: Use the container path (e.g., `/library`)
     - Binary: Use the local path (e.g., `/home/user/comics`)
5. Configure scanning options:
   - **Enable automatic scanning**: Recommended
   - **Scan on startup**: Initial scan when Codex starts
   - **Schedule**: Cron expression for periodic scans
6. Click **Save**

### Via API

```bash
# Get your token
TOKEN=$(curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"your-password"}' | jq -r '.access_token')

# Create library
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Comics",
    "path": "/library",
    "scanning_config": {
      "enabled": true,
      "scan_on_start": true
    }
  }'
```

## Library Path Configuration

### Docker Setup

When using Docker, mount your media folder as a volume:

```yaml
# docker-compose.yml
services:
  codex:
    volumes:
      - /path/to/your/comics:/library:ro
```

Then use `/library` as the path when creating the library.

### Multiple Libraries

You can mount multiple folders:

```yaml
volumes:
  - /media/comics:/library/comics:ro
  - /media/manga:/library/manga:ro
  - /media/ebooks:/library/ebooks:ro
```

Create separate libraries for each:
- Comics: `/library/comics`
- Manga: `/library/manga`
- Ebooks: `/library/ebooks`

## Running Your First Scan

### Automatic Scan

If you enabled "Scan on startup", Codex will automatically scan when the library is created.

### Manual Scan

1. Go to your library in the sidebar
2. Click the **Scan** button
3. Choose **Normal** for incremental scan or **Deep** for full re-scan
4. Watch the progress in real-time

### Monitor Progress

- Progress bar shows scan status
- Files discovered and processed
- New series and books found
- Any errors encountered

## Browsing Your Library

Once scanning completes:

### By Series

1. Click on a library in the sidebar
2. Browse series as cards or list
3. Click a series to see its books

### By Recent

- **Recently Added**: Latest scanned books
- **Continue Reading**: Books you've started

### Reading a Book

1. Click on a book cover
2. The reader opens with the first page
3. Navigate with:
   - Arrow keys or swipe
   - Click left/right edges
   - Slider at bottom
4. Progress is saved automatically

## Next Steps

Now that you're up and running:

### Configure Your Server

- [Full Configuration Guide](./configuration)
- Enable [API docs](./api#interactive-api-documentation) for API exploration
- Set up [HTTPS](./deployment#reverse-proxy-setup) for security

### Set Up Mobile Access

- Configure [OPDS](./opds) for e-reader apps
- Set up remote access via reverse proxy

### Manage Users

- Create additional [user accounts](./users)
- Configure [permissions](./users#permission-system)
- Set up [API keys](./users#api-keys) for automation

### Explore Features

- [Library scanning options](./libraries)
- [Supported file formats](./formats)
- [API documentation](./api)

## Common Issues

### Library Not Found

**Docker**: Ensure the volume is mounted correctly and the path matches.

```bash
# Check mounts
docker compose exec codex ls -la /library
```

**Binary**: Verify the path exists and Codex has read permissions.

### Scan Not Starting

1. Check library path is accessible
2. Verify no other scan is running
3. Check server logs for errors

### Books Not Appearing

1. Verify file format is supported (CBZ, CBR, EPUB, PDF)
2. Check files aren't corrupted
3. Run a deep scan to re-process all files

### Login Issues

1. Verify credentials are correct
2. Check JWT secret is set in configuration
3. Clear browser cookies and try again

For more troubleshooting, see the [Troubleshooting Guide](./troubleshooting).
