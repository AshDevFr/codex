---
sidebar_position: 5
---

# Configuration

Codex uses YAML configuration files to control server behavior, database connections, and feature settings.

## Configuration File Location

By default, Codex looks for `codex.yaml` in the current directory. You can specify a custom path:

```bash
codex serve --config /path/to/codex.yaml
```

## Configuration Structure

### Database Configuration

Codex supports both SQLite and PostgreSQL databases.

#### SQLite (Simple Setup)

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db
    pragmas:
      journal_mode: WAL
      synchronous: NORMAL
```

**SQLite Pragmas:**

- `journal_mode`: WAL (Write-Ahead Logging) recommended for better concurrency
- `synchronous`: NORMAL for good balance, FULL for maximum safety
- `foreign_keys`: Always enabled (cannot be disabled)

#### PostgreSQL (Production)

```yaml
database:
  db_type: postgres
  postgres:
    host: localhost
    port: 5432
    user: codex
    password: codex
    database: codex
    ssl_mode: prefer
```

**PostgreSQL SSL Modes:**

- `disable`: No SSL
- `allow`: Try without SSL, use SSL if available
- `prefer`: Try SSL first, fallback to no SSL (default)
- `require`: SSL required
- `verify-ca`: SSL required, verify CA
- `verify-full`: SSL required, verify CA and hostname

### Application Configuration

```yaml
application:
  name: Codex
  host: 127.0.0.1 # Bind address
  port: 8080 # Server port
  debug: false # Enable debug mode
```

### Logging Configuration

```yaml
logging:
  level: info # error, warn, info, debug, trace
  file: ./logs/codex.log # Optional: enable file logging
```

### Authentication Configuration

```yaml
auth:
  jwt_secret: "CHANGE_ME_IN_PRODUCTION" # Override with CODEX_AUTH_JWT_SECRET env var
  jwt_expiry_hours: 24
  password_hash_cost: 19456 # Argon2 memory cost
  password_hash_iterations: 2
  password_hash_parallelism: 1
```

**Security Note:** Always use a strong, random JWT secret in production. Generate one with:

```bash
openssl rand -base64 32
```

### API Configuration

```yaml
api:
  enable_swagger: false # Enable Swagger UI at /docs
  swagger_path: "/docs" # Swagger UI path
  cors_enabled: true # Enable CORS
  max_page_size: 100 # Maximum items per page
```

## Environment Variables

You can override configuration values using environment variables:

### Database Overrides

```bash
CODEX_DATABASE_DB_TYPE=postgres
CODEX_DATABASE_POSTGRES_HOST=localhost
CODEX_DATABASE_POSTGRES_PORT=5432
CODEX_DATABASE_POSTGRES_USER=codex
CODEX_DATABASE_POSTGRES_PASSWORD=secret
CODEX_DATABASE_POSTGRES_DATABASE=codex
```

### Application Overrides

```bash
CODEX_APPLICATION_NAME=MyCodex
CODEX_APPLICATION_HOST=0.0.0.0
CODEX_APPLICATION_PORT=8080
CODEX_APPLICATION_DEBUG=true
```

### Authentication Overrides

```bash
CODEX_AUTH_JWT_SECRET=your-secret-key-here
CODEX_AUTH_JWT_EXPIRY_HOURS=48
```

### Logging Overrides

```bash
CODEX_LOGGING_LEVEL=debug
CODEX_LOGGING_FILE=./logs/codex.log
```

## Example Configuration Files

### Minimal SQLite Config

```yaml
database:
  db_type: sqlite
  sqlite:
    path: ./data/codex.db

application:
  host: 127.0.0.1
  port: 8080
```

### Production PostgreSQL Config

```yaml
database:
  db_type: postgres
  postgres:
    host: db.example.com
    port: 5432
    user: codex
    password: ${DB_PASSWORD} # Use env var
    database: codex
    ssl_mode: verify-full

application:
  name: Codex Production
  host: 0.0.0.0
  port: 8080
  debug: false

logging:
  level: info
  file: /var/log/codex/codex.log

auth:
  jwt_secret: ${JWT_SECRET} # Use env var

api:
  enable_swagger: false
  cors_enabled: true
```

## Configuration Validation

Codex validates configuration on startup. Common errors:

- **Invalid database type**: Must be `sqlite` or `postgres`
- **Missing database path**: SQLite requires a path
- **Database connection failed**: Check credentials and network
- **Invalid port**: Must be between 1-65535
- **File permissions**: Ensure Codex can write to log files and database

## Next Steps

- Learn about [deployment options](./deployment)
- Set up your first [library](./getting-started)
