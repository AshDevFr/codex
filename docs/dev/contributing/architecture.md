---
---

# Architecture

This document describes the architecture and design decisions behind Codex.

## Overview

Codex is built with Rust for performance and safety. It follows a modular architecture that separates concerns and enables horizontal scaling.

## Core Principles

### Stateless Design

Codex is designed to be stateless:

- No server-side sessions
- All state stored in database
- JWT tokens for authentication
- Perfect for horizontal scaling

### Database Agnostic

Supports multiple database backends:

- **SQLite** - Simple, embedded, perfect for small deployments
- **PostgreSQL** - Production-grade, supports complex queries and scaling

### Modular Parser System

Format parsers are pluggable and isolated:

- Each format has its own parser module
- Easy to add new formats
- Failures in one parser don't affect others

## System Architecture

```
┌─────────────────────────────────────────┐
│           HTTP API Layer                │
│        (Axum Web Framework)             │
└───────────────────┬─────────────────────┘
                    │
┌───────────────────▼─────────────────────┐
│          Request Handlers               │
│   (Libraries, Books, Series, Users)     │
└───────────────────┬─────────────────────┘
                    │
┌───────────────────▼─────────────────────┐
│          Repository Layer               │
│        (Database Abstraction)           │
└───────────────────┬─────────────────────┘
                    │
┌───────────────────▼─────────────────────┐
│           Database Layer                │
│     (SeaORM - SQLite/PostgreSQL)        │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│           Scanner System                │
│      (File Detection & Analysis)        │
└───────────────────┬─────────────────────┘
                    │
┌───────────────────▼─────────────────────┐
│           Parser System                 │
│        (CBZ, CBR, EPUB, PDF)            │
└─────────────────────────────────────────┘
```

## Component Overview

### API Layer

Built with **Axum**, a modern Rust web framework:

- **Type-safe routing** - Compile-time route validation
- **Middleware support** - Authentication, CORS, logging
- **Async/await** - Non-blocking I/O
- **OpenAPI/Scalar** - Auto-generated API docs

### Database Layer

Uses **SeaORM** for database operations:

- **Type-safe queries** - Compile-time SQL validation
- **Migration system** - Version-controlled schema changes
- **Multi-database support** - SQLite and PostgreSQL
- **Connection pooling** - Efficient resource usage

### Parser System

Modular parser architecture:

```
parsers/
├── mod.rs           # Parser registry
├── traits.rs        # Parser trait definition
├── cbz/            # CBZ parser
├── cbr/            # CBR parser (optional)
├── epub/           # EPUB parser
├── pdf/            # PDF parser
└── metadata.rs     # Metadata extraction
```

Each parser implements the `Parser` trait:

```rust
pub trait Parser: Send + Sync {
    fn detect(&self, path: &Path) -> Result<bool>;
    fn parse(&self, path: &Path) -> Result<ParsedBook>;
    fn extract_pages(&self, path: &Path) -> Result<Vec<Page>>;
}
```

### Scanner System

The scanner discovers and analyzes files:

1. **Detector** - Identifies file types
2. **Analyzer** - Extracts metadata and structure
3. **Repository** - Stores results in database

### Authentication System

JWT-based authentication:

- **Argon2** password hashing
- **JWT tokens** for stateless auth
- **Permission system** - Role-based access control
- **API keys** - For programmatic access

## Data Models

### Core Entities

```
Library
  ├── Series
  │     └── Book
  │           └── Page
  │
  └── User
        └── API Key
```

### Relationships

- **Library** → **Series** (one-to-many)
- **Series** → **Book** (one-to-many)
- **Book** → **Page** (one-to-many)
- **User** → **API Key** (one-to-many)

### Metadata Sources

Books can have metadata from multiple sources:

- **ComicInfo.xml** - Embedded metadata
- **Filename parsing** - Extracted from file names
- **Database** - User-provided metadata
- **External APIs** - Future: integration with metadata providers

## Scalability

### Horizontal Scaling

Codex is designed for horizontal scaling:

1. **Stateless servers** - Any instance can handle any request
2. **Shared database** - All instances share the same database
3. **Load balancing** - Use any load balancer (Nginx, HAProxy, etc.)
4. **Kubernetes ready** - Deploy multiple replicas

### Database Scaling

**SQLite:**

- Single-file database
- Good for small to medium deployments
- Limited concurrent writes

**PostgreSQL:**

- Supports read replicas
- Connection pooling
- Can handle high concurrency
- Supports sharding (future)

### Caching Strategy

Current:

- Database query caching (via SeaORM)
- File system caching for extracted pages

Planned:

- Redis for distributed caching
- CDN integration for media files
- In-memory metadata cache

## Security

### Authentication

- **Password hashing** - Argon2 with configurable cost
- **JWT tokens** - Stateless, signed tokens
- **Token expiration** - Configurable expiry
- **API keys** - For programmatic access

### Authorization

- **Permission system** - Fine-grained permissions
- **Role-based access** - Admin, user, read-only roles
- **Resource-level permissions** - Per-library access control

### Input Validation

- **Type-safe parsing** - Rust's type system
- **Request validation** - Validate all inputs
- **SQL injection protection** - Parameterized queries via SeaORM
- **Path traversal protection** - Validate file paths

## Error Handling

### Error Types

Codex uses a hierarchical error system:

```rust
pub enum CodexError {
    Database(DatabaseError),
    Parser(ParserError),
    Authentication(AuthError),
    Validation(ValidationError),
    // ...
}
```

### Error Responses

All API errors follow a consistent format:

```json
{
  "error": "ErrorType",
  "message": "Human-readable message",
  "details": {}
}
```

## Logging

Structured logging with **tracing**:

- **Log levels** - error, warn, info, debug, trace
- **Structured fields** - JSON-formatted logs
- **File and console** - Configurable outputs
- **Request tracing** - Track requests across services

## Testing

### Test Structure

```
tests/
├── api/              # API integration tests
├── db/               # Database tests
├── parsers/          # Parser tests
└── scanner/          # Scanner tests
```

### Test Databases

- **SQLite** - Fast, in-memory for unit tests
- **PostgreSQL** - Integration tests with real database

## Performance Optimizations

### Database

- **Connection pooling** - Reuse database connections
- **Prepared statements** - Via SeaORM
- **Indexes** - Optimized database indexes
- **Query optimization** - Efficient queries

### File Processing

- **Parallel processing** - Process multiple files concurrently
- **Lazy extraction** - Extract pages on-demand
- **Caching** - Cache extracted metadata
- **Streaming** - Stream large files

### Memory Management

- **Zero-copy where possible** - Rust's ownership system
- **Efficient data structures** - Minimal allocations
- **Resource cleanup** - Proper cleanup of file handles

## Future Enhancements

### Planned Features

- **Full-text search** - Search book content
- **Thumbnail generation** - Automatic thumbnails
- **Webhook support** - Event notifications
- **Plugin system** - Extensible architecture
- **Metadata providers** - Integration with external APIs
- **Multi-language support** - i18n for UI

### Performance Improvements

- **Async file I/O** - Non-blocking file operations
- **Distributed caching** - Redis integration
- **CDN support** - Serve media from CDN
- **Background jobs** - Async task processing

## Technology Stack

### Core

- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **Axum** - Web framework
- **SeaORM** - ORM framework

### Database

- **SQLite** - Embedded database
- **PostgreSQL** - Production database
- **SeaORM** - Database abstraction

### Parsing

- **zip** - ZIP archive support
- **unrar** - RAR archive support (optional)
- **lopdf** - PDF parsing
- **quick-xml** - XML/EPUB parsing
- **image** - Image processing

### Security

- **argon2** - Password hashing
- **jsonwebtoken** - JWT tokens
- **sha2** - File hashing

## Next Steps

- Review [deployment options](/docs/deployment)
- Learn about [API usage](/docs/api)
- Explore [configuration options](/docs/configuration)
