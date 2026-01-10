---
---

# Development

This guide covers building Codex from source for development purposes.

## Prerequisites

- **Rust** 1.92 or later
- **Cargo** (comes with Rust)
- **OpenSSL** development libraries
- **UnRAR** library (optional, for CBR support)

### Installing Rust

If you don't have Rust installed, use [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Installing System Dependencies

#### macOS

```bash
brew install openssl
```

#### Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev
```

#### Fedora/RHEL

```bash
sudo dnf install gcc openssl-devel
```

## Building from Source

### Clone the Repository

```bash
git clone https://github.com/AshDevFr/codex.git
cd codex
```

### Standard Build (with CBR support)

By default, Codex includes CBR support:

```bash
cargo build --release
```

The binary will be located at `target/release/codex`.

### Build without CBR Support

If you want to avoid the proprietary UnRAR dependency:

```bash
cargo build --release --no-default-features
```

This disables CBR parsing but keeps all other formats (CBZ, EPUB, PDF) working.

### Development Build

For faster iteration during development:

```bash
cargo build
```

This creates an unoptimized binary at `target/debug/codex` that compiles faster.

## Running Tests

### All Tests

```bash
cargo test
```

### Specific Test Suites

```bash
# Parser tests
cargo test parsers

# Database tests
cargo test db

# API tests
cargo test api
```

### Tests without CBR Support

```bash
cargo test --no-default-features
```

### PostgreSQL Integration Tests

For running PostgreSQL tests, start the test container with `make test-up` and run `make test-postgres`.

## Development Workflow

### Running the Server

```bash
# Development mode (with debug logging)
RUST_LOG=debug cargo run -- serve --config config/config.sqlite.yaml
```

### Hot Reload (Docker)

Use Docker Compose with watch mode for automatic rebuilds:

```bash
docker compose -f docker-compose.yml -f compose.watch.yml --profile dev up --watch
```

### Frontend Development

For full-stack development with the React frontend:

```bash
# Start backend and frontend (recommended)
docker compose --profile dev up

# Access the app at http://localhost:5173
# The Vite dev server proxies /api and /opds requests to the backend
```

**Or run them separately:**

```bash
# Terminal 1 - Backend
cargo run -- serve --config config/config.sqlite.yaml

# Terminal 2 - Frontend
cd web
npm install
npm run dev

# Access at http://localhost:5173
```

**Important**: Always use `http://localhost:5173` (frontend) in development. The Vite dev server automatically proxies API requests to the backend at `http://localhost:8080`.

The frontend is a React/TypeScript application using Vite, Mantine UI, and TanStack Query.

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## Project Structure

```
codex/
├── src/
│   ├── api/          # HTTP API handlers
│   ├── commands/      # CLI commands
│   ├── config/        # Configuration management
│   ├── db/            # Database layer
│   ├── parsers/       # File format parsers
│   ├── scanner/       # File scanning logic
│   └── utils/         # Utility functions
├── migration/         # Database migrations
├── tests/             # Integration tests
└── docs/              # Documentation
```

## Database Migrations

### Running Migrations

Migrations run automatically on server startup. To run manually:

```bash
cd migration
cargo run
```

### Creating New Migrations

Use SeaORM's migration CLI:

```bash
# Install migration CLI
cargo install sea-orm-cli

# Create new migration
sea-orm-cli migrate generate <migration_name>
```

## Contributing

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Run `cargo clippy` before submitting PRs
- Write tests for new features

### Testing

- Add unit tests for new functions
- Add integration tests for API endpoints
- Ensure all tests pass before submitting

### Documentation

- Update relevant documentation when adding features
- Add code comments for complex logic
- Update API documentation if endpoints change

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug codex serve --config codex.yaml
```

### Database Debugging

For SQLite, you can inspect the database directly:

```bash
sqlite3 data/codex.db
```

For PostgreSQL:

```bash
psql -U codex -d codex
```

### Tracing

Codex uses the `tracing` crate for structured logging. Set log levels:

```bash
RUST_LOG=codex=debug,tower_http=info codex serve
```

## Performance Profiling

### Release Build

Always profile with release builds:

```bash
cargo build --release
```

### Profiling Tools

- **perf** (Linux): `perf record ./target/release/codex serve`
- **Instruments** (macOS): Use Xcode Instruments
- **flamegraph**: Generate flamegraphs for performance analysis

## Next Steps

- Review the [Architecture](./architecture) documentation
- Check the [API Documentation](../api) for endpoint details
- See [Configuration](../configuration) for available options
