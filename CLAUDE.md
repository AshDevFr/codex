# Claude Development Guide for Codex

This document provides guidelines for working with Claude on the Codex project. Codex is a next-generation digital library server for comics, manga, and ebooks built in Rust.

## Project Overview

**Codex** is a Rust-based digital library server with:

- **Backend**: Rust (Axum web framework, SeaORM, Tokio)
- **Frontend**: TypeScript/React (Vite, Mantine UI, TanStack Query)
- **Databases**: SQLite (development, testing and production) and PostgreSQL (scaling and production)
- **Formats**: CBZ, CBR, EPUB, PDF support
- **Architecture**: Stateless, horizontally scalable

## Task Workflow

The maintainer keeps planning documents in `.specs/`, a separate private repository that this one
ignores. It is optional: if you do not have it, skip this section, and never reference its
files, phase numbers, or task IDs from committed code, comments, or commit messages.

- **Spec Repo**: `.specs/`
- **PRD**: `.specs/docs/PRD.md`
- **Phase Specs**: `.specs/docs/specs/phase-N_<title>.md`
- **Task Directory**: `.specs/docs/tasks/phase-N/`
- **Task File Pattern**: `N.NN-descriptive-name.md`

## Core Principles

### 1. All Implementations Must Have Tests

**MANDATORY**: Every new feature, bug fix, or refactoring must include appropriate tests:

- **Unit Tests**: Test individual functions, methods, and modules in isolation
- **Integration Tests**: Test interactions between components (when applicable)
- **API Tests**: Test HTTP endpoints with proper request/response validation
- **Frontend Tests**: Test React components, hooks, and utilities

#### Test Coverage Requirements

- **Backend (Rust)**:
  - Unit tests in the same module or `tests/` directory
  - Integration tests in `tests/` directory organized by feature area
  - Use `#[tokio::test]` for async tests
  - Use `#[test]` for synchronous tests
  - Test both success and error paths
  - Test edge cases and boundary conditions

- **Frontend (TypeScript/React)**:
  - Component tests using Vitest and React Testing Library
  - Hook tests for custom hooks
  - API client tests for request/response handling
  - Test user interactions, not implementation details

#### Test Organization

```
tests/
├── api/              # API endpoint integration tests
├── db/               # Database repository tests
├── parsers/          # File parser tests
├── scanner/          # Library scanner tests
└── common/           # Shared test utilities

web/src/
├── **/*.test.ts      # Unit tests alongside source
└── **/*.test.tsx     # Component tests
```

#### Example Test Patterns

**Rust Unit Test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = "test";

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }
}
```

**Rust Integration Test:**

```rust
#[path = "../common/mod.rs"]
mod common;

use common::*;

#[tokio::test]
async fn test_api_endpoint() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/endpoint");
    let (status, response) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Additional assertions...
}
```

**TypeScript Component Test:**

```typescript
import { renderWithProviders, screen, userEvent } from "@/test/utils";

it("should render and handle interactions", async () => {
  const user = userEvent.setup();
  renderWithProviders(<MyComponent />);

  expect(screen.getByText("Expected Text")).toBeInTheDocument();

  await user.click(screen.getByRole("button"));
  // Assertions...
});
```

### 2. Testing Best Practices

#### When to Write Unit Tests

- Pure functions and utilities
- Business logic and calculations
- Data transformations
- Error handling
- Validation logic

#### When to Write Integration Tests

- API endpoints (HTTP handlers)
- Database operations (repositories)
- File system operations
- External service interactions
- Multi-component workflows

#### Test Database Strategy

- **SQLite**: Default for fast unit/integration tests (a fresh database file in a temp directory per test)
- **PostgreSQL**: Optional integration tests for database-specific features
- Use `setup_test_db()` for SQLite tests
- Use `setup_test_db_postgres()` for PostgreSQL tests (skips if unavailable)

#### Test Fixtures

- Use `tests/common/fixtures.rs` for creating test data
- Use `tests/fixtures/` for test files (CBZ, CBR, etc.)
- Keep fixtures minimal and focused
- Clean up resources (temp directories, database connections)

### 3. Code Quality Standards

#### Rust Code Style

- Follow Rust standard formatting: `cargo fmt`
- Run clippy: `cargo clippy -- -D warnings`
- Use meaningful variable and function names
- Document public APIs with doc comments
- Handle errors explicitly (avoid `unwrap()` in production code)
- Use `anyhow::Result` for application errors
- Use `thiserror` for structured error types

#### TypeScript/React Code Style

- Use TypeScript strictly (no `any` types)
- Follow React best practices (hooks, functional components)
- Use meaningful component and function names
- Extract reusable logic into custom hooks
- Use proper error boundaries
- Handle loading and error states
- **Use `es-toolkit`** for utility functions (arrays, objects, strings, etc.) instead of writing custom implementations. Check `es-toolkit` first before creating new utility functions

#### Error Handling

- Return `Result<T, E>` types in Rust
- Use proper HTTP status codes in API responses
- Provide meaningful error messages
- Log errors with appropriate levels (error, warn, info, debug)

### 4. Development Workflow

#### Before Making Changes

1. Understand the existing codebase structure
2. Check existing tests for similar functionality
3. Review related documentation
4. Check for similar implementations to maintain consistency

#### When Implementing Features or Fixing Bugs

1. **Use TDD when possible** - Write a failing test first that captures the expected behavior or reproduces the bug, then implement the fix/feature until the test passes
2. Implement the feature or fix
3. **Run only localized tests** - Run the specific test(s) related to your changes (e.g., `cargo test test_name`), not the full suite, while iterating
4. Run `cargo fmt`, `cargo clippy -- -D warnings`, and `make check` to ensure code quality
5. Update documentation if needed
6. **Before opening a PR** - Run the full test suite (`make test-fast`) and `cargo clippy -- -D warnings` to catch any regressions or warnings across the whole app

#### Testing Commands

```bash
# Backend tests (SQLite)
cargo test

# Fast parallel tests (requires cargo-nextest)
make test-fast

# Backend tests with PostgreSQL
make test-postgres

# Fast PostgreSQL tests (requires cargo-nextest)
make test-fast-postgres

# All tests (backend + frontend)
make test-all

# Fast All tests (backend + frontend) (requires cargo-nextest)
make test-fast-all

# Frontend tests only
cd web && npm run test:run

# Run specific test
cargo test test_function_name

# Run tests with output
cargo test -- --nocapture
```

#### Code Quality Checks

```bash
# Format code
cargo fmt
cd web && npm run format

# Lint code
cargo clippy -- -D warnings
cd web && npm run lint

# Run all checks
make check
```

### 5. Project Structure

#### Backend (Rust)

Codex is a Cargo workspace. The root `codex` crate produces the binary and contains only `main.rs` plus per-subcommand orchestration. Every subsystem lives in its own sibling crate under `crates/`. Editing a crate only recompiles that crate and its downstream consumers, so warm rebuilds stay tight.

```
src/                          # codex (binary) crate
├── main.rs                   # CLI entry point
└── commands/                 # Subcommands: scan, serve, worker, seed,
                              # migrate, wait_for_migrations, openapi, tasks

crates/
├── codex-api/                # HTTP API (axum routes, OPDS, OPDS2, Komga,
│   ├── routes/               #   KOReader, web static assets, observability)
│   │   ├── v1/{handlers,dto,routes}/   # Native /api/v1 API
│   │   ├── opds/             # OPDS 1.2 (Atom XML)
│   │   ├── opds2/            # OPDS 2.0 (JSON)
│   │   ├── komga/            # Optional Komga-compatible API
│   │   └── koreader/         # KOReader sync endpoints
│   ├── extractors/           # Axum extractors (auth, AppState)
│   ├── middleware/           # Auth, rate limit, http_metrics, tracing
│   ├── observability/        # OTel providers, HTTP layers, inventory poller
│   ├── error.rs              # ApiError → HTTP response mapping
│   ├── permissions.rs        # Re-export of codex_models::permissions
│   ├── docs.rs               # utoipa OpenAPI aggregator
│   └── web.rs                # Embedded frontend (rust-embed)
├── codex-config/             # Config loader, env overrides, defaults
├── codex-db/                 # SeaORM entities + repositories + Database
├── codex-events/             # In-process EventBroadcaster
├── codex-models/             # Cross-layer DTOs (permissions, sort, filter, …)
├── codex-parsers/            # CBZ/CBR (rar feature)/EPUB/PDF
├── codex-scanner/            # Library scan workflow
├── codex-scheduler/          # Cron/interval scheduler
├── codex-search/             # In-memory fuzzy index
├── codex-services/           # Business logic + metrics
├── codex-tasks/              # Task worker + handlers
└── codex-utils/              # JWT, password, hashing, CredentialEncryption,
                              #   error types, file/zip helpers

migration/                    # SeaORM migrations (own crate; codex-db → migration)
tests/                        # Integration tests against codex-api
```

Each crate builds in isolation (`cargo build -p codex-<crate>`). The root crate is gated by three features that fan out to the relevant subcrates: `default = ["rar", "observability"]`, plus `embed-frontend` for the bundled UI.

#### Frontend (TypeScript/React)

```
web/src/
├── api/              # API client and types
├── components/       # React components
├── hooks/            # Custom React hooks
├── pages/            # Page components
├── store/            # State management (Zustand)
├── test/             # Test utilities
└── utils/            # Utility functions
```

### 6. Database Considerations

#### Dual Database Support

- Codex supports both SQLite and PostgreSQL
- Use SeaORM for database operations (database-agnostic)
- Test with SQLite by default (fast, no external dependencies)
- Test with PostgreSQL for database-specific features:
  - JOINs with aggregations
  - Complex SQL queries
  - Transaction behavior
  - Production bug regressions

#### Migration Management

- Migrations in `migration/src/`
- Use SeaORM migrations
- Test migrations before applying

#### Sorting and Pagination

**IMPORTANT**: Always perform sorting at the database level, never in-memory after fetching data.

- **Always sort in the database query** - Use SeaORM's `.order_by()` before pagination
- **Never sort after pagination** - Sorting a paginated subset only orders that page, not the full dataset
- **Avoid in-memory sorting of query results** - This defeats the purpose of pagination and produces incorrect results

**Why this matters**: When you paginate first and sort second, you only sort the current page's items. The "first" items on page 1 may not actually be the first items overall - they're just the first items of an arbitrary subset.

```rust
// ✅ CORRECT: Sort in database, then paginate
let items = Entity::find()
    .order_by_asc(Column::Name)  // Sort first
    .paginate(db, page_size)      // Then paginate
    .fetch_page(page)
    .await?;

// ❌ WRONG: Paginate then sort in memory
let mut items = Entity::find()
    .paginate(db, page_size)
    .fetch_page(page)
    .await?;
items.sort_by(|a, b| a.name.cmp(&b.name));  // Only sorts this page!
```

### 7. API Development

#### API Structure

- **Native API** under `/api/v1/` - Primary Codex REST API
- **OPDS 1.2** under `/opds/` - Atom XML catalog for e-readers
- **OPDS 2.0** under `/opds/v2/` - JSON catalog for modern readers
- **Komga API** under `/{prefix}/api/v1/` - Compatibility layer for Komga apps (disabled by default)
- Use Axum for routing and handlers
- DTOs in `crates/codex-api/src/routes/v1/dto/` for request/response types
- Error handling via `crates/codex-api/src/error.rs`
- Authentication via JWT tokens, API keys, or Basic Auth
- Authorization via permission system

#### Komga-Compatible API

The Komga API is a compatibility layer that allows third-party apps designed for Komga (like Komic for iOS) to work with Codex.

- **Disabled by default** - Enable via `komga_api.enabled: true` in config
- **Configurable prefix** - Default: `komga`, configurable via `komga_api.prefix`
- **Same auth methods** - Supports JWT, API keys, and Basic Auth
- **Tests in `tests/api/komga.rs`** - Integration tests for all endpoints
- **Documentation** - See `docs/docs/third-party-apps.md` and `docs/docs/configuration.md`

#### API Testing

- Use `tests/common/http.rs` helpers
- Test authentication and authorization
- Test request validation
- Test error responses
- Test success responses with proper data

### 8. File Parsing

#### Supported Formats

- **CBZ**: ZIP-based comic archives
- **CBR**: RAR-based comic archives (optional, requires `rar` feature)
- **EPUB**: Ebook format
- **PDF**: Portable Document Format

#### Parser Testing

- Test format detection (`can_parse`)
- Test metadata extraction
- Test page information extraction
- Test ComicInfo.xml parsing (when applicable)
- Test error handling for invalid files
- Use fixtures from `tests/fixtures/`

### 9. Library Scanning

#### Scanner Features

- Normal and deep scan modes
- Progress tracking via SSE
- File deduplication via hashing
- Series and book organization
- Soft delete support
- Task queue integration

#### Scanner Testing

- Test scan modes (normal, deep)
- Test progress reporting
- Test file discovery
- Test duplicate detection
- Test soft delete/restore
- Test error recovery

### 10. Frontend Development

#### Tech Stack

- **Framework**: React 19
- **Build Tool**: Vite
- **UI Library**: Mantine
- **State Management**: Zustand
- **Data Fetching**: TanStack Query
- **Testing**: Vitest + React Testing Library

#### Frontend Testing

- Test component rendering
- Test user interactions
- Test API integration (mocked)
- Test state management
- Test routing
- Test error states

### 11. Documentation

#### Code Documentation

- Document public APIs with doc comments
- Use `///` for Rust documentation
- Use JSDoc for TypeScript functions
- Include examples in documentation
- Keep documentation up-to-date with code

#### Markdown Documentation

- User documentation in `docs/docs/`
- API documentation via OpenAPI/Scalar
- Architecture documentation
- Development guides
- Troubleshooting guides

### 12. Common Tasks

#### Adding a New API Endpoint

1. Define DTOs in `crates/codex-api/src/routes/v1/dto/`
2. Create handler in `crates/codex-api/src/routes/v1/handlers/`
3. Add route in `crates/codex-api/src/routes/v1/routes/`
4. Register the path and schemas in `crates/codex-api/src/docs.rs`
5. Write integration tests in `tests/api/` (declare the module in `tests/api/mod.rs`)
6. Regenerate the OpenAPI spec and frontend types: `make openapi-all`

#### Adding a New Parser

1. Implement the `FormatParser` trait (`crates/codex-parsers/src/traits.rs`) in a new module under `crates/codex-parsers/src/`
2. Export the module from `crates/codex-parsers/src/lib.rs`
3. Dispatch to it from format detection in `crates/codex-scanner/src/analyzer.rs`
4. Write tests in `tests/parsers/`
5. Add test fixtures under `tests/fixtures/` if needed

#### Adding a New Database Entity

1. Create entity in `crates/codex-db/src/entities/` (register it in `mod.rs`)
2. Create repository in `crates/codex-db/src/repositories/` (register it in `mod.rs`)
3. Create migration in `migration/src/` and add it to the `Migrator` list in `migration/src/lib.rs`
4. Write repository tests in `tests/db/`
5. Update related code

### 13. Makefile Commands

The Makefile is the source of truth; run `make help` for the full, current list. The ones you will reach for most:

```bash
make dev-up              # Docker dev environment (backend, worker, frontend)
make dev-seed            # Create the initial admin user in the dev environment
make run                 # Run locally without Docker
make frontend-mock       # Frontend against a mock API (no backend needed)
make test-fast           # Backend tests with nextest (SQLite)
make test-fast-postgres  # Backend tests against PostgreSQL (starts the test DB)
make test-frontend       # Frontend tests
make check               # Format, lint, and tests
make openapi-all         # Regenerate the OpenAPI spec and frontend types
make setup-hooks         # Install pre-commit hooks
```

> **⚠️ Do not touch `CHANGELOG.md` or run any `make changelog*` commands.** The changelog is generated by `git-cliff` when the maintainer cuts a release (`make release-prepare`). Editing it by hand or regenerating it ad hoc creates conflicts and overwrites the release pipeline's output. If you think changelog content is wrong or missing, flag it rather than modifying the file.

### 14. Important Notes

#### CBR Support

- CBR support requires the UnRAR library (proprietary license)
- Enabled by default via `rar` feature
- Build without CBR: `cargo build --no-default-features`
- Test without CBR: `cargo test --no-default-features`

#### Database Testing

- PostgreSQL tests are optional and skip if database unavailable
- Start PostgreSQL test container: `make test-up`
- PostgreSQL tests use `--ignored` flag by default

#### Frontend Testing

- Tests use Vitest with jsdom environment
- Mock API calls with MSW (handlers in `web/src/mocks/`)
- Use `renderWithProviders` for component tests
