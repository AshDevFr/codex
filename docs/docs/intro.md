---
sidebar_position: 1
slug: /
---

# Introduction

**Codex** is a next-generation digital library server for comics, manga, and ebooks built in Rust. Designed to scale horizontally while remaining simple for homelab deployments.

## Features

### Dual Database Support

- **SQLite** for simple setups and development
- **PostgreSQL** for production deployments

### Horizontal Scaling

- Stateless architecture perfect for Kubernetes deployments
- Designed to scale across multiple instances

### Multiple Format Support

- **CBZ** (Comic Book ZIP) - Standard comic archive format
- **CBR** (Comic Book RAR) - RAR-based comic archives
- **EPUB** - Ebook format
- **PDF** - Portable Document Format

### Fast Metadata Extraction

- ComicInfo.xml parsing for rich metadata
- Automatic page information extraction
- File hashing for deduplication

### Flexible Organization

- Per-library scanning strategies
- Customizable metadata sources
- Series and book organization

## Quick Start

Get started with Codex in minutes:

1. **Install** Codex using a [pre-built binary](./installation#option-1-pre-built-binary) or [Docker](./installation#option-2-docker)
2. **Configure** your server with a [configuration file](./configuration)
3. **Start** the server and create your first [library](./getting-started#your-first-library)

See the [Getting Started Guide](./getting-started) for detailed instructions.

## CBR Support and Licensing

CBR (Comic Book RAR) archive support requires the UnRAR library, which uses a **proprietary license** (not standard open source). The UnRAR license allows free use for extraction but prohibits creating RAR compression software.

Pre-built binaries include CBR support by default. If you're building from source and want to avoid proprietary dependencies, see the [Development Guide](./development#build-without-cbr-support).

## Project Status

Currently in Phase 1 (MVP Core). The project is actively under development.

## License

MIT
