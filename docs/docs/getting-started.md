---
sidebar_position: 3
---

# Getting Started

This guide will help you get Codex up and running quickly.

## Quick Start

### 1. Install Codex

See the [Installation Guide](./installation) for detailed instructions.

**Quick install with Docker:**

```bash
docker run -d -p 8080:8080 codex:latest
```

Or download a [pre-built binary](./installation#option-1-pre-built-binary) for your platform.

### 2. Create Configuration

Create a `codex.yaml` file:

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db

application:
  host: 127.0.0.1
  port: 8080
```

### 3. Initialize Database

Codex will automatically run migrations on first startup. Just start the server:

```bash
codex serve --config codex.yaml
```

### 4. Create Admin User

In another terminal, create your admin user:

```bash
codex seed --config codex.yaml
```

Follow the prompts to create your admin account.

### 5. Access the API

The API is now available at `http://localhost:8080/api/v1`.

Test it:

```bash
# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"your-password"}'
```

## Your First Library

### Create a Library via API

```bash
# Get your token (from login response)
TOKEN="your-jwt-token-here"

# Create a library
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Comics",
    "path": "/path/to/your/comics",
    "scan_strategy": "auto"
  }'
```

### Scan Your Library

Codex will automatically scan libraries with `scan_strategy: "auto"`. For manual scanning, use the scan command:

```bash
codex scan /path/to/your/comics --json
```

## Next Steps

- Learn about [configuration options](./configuration)
- Explore the [API documentation](./api)
- Read about [supported formats](./formats)
- Check [deployment options](./deployment) for production
