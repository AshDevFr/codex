# Codex

A next-generation digital library server for comics, manga, and ebooks built in Rust. Designed to scale horizontally while remaining simple for homelab deployments.

> **Note:** Codex is under active development. Architecture and APIs may still change as the project matures. There is a sizable backlog of planned work, so responses to feature requests may be slow. Bug reports are always welcome.

## Features

- **Dual database support**: SQLite for simple setups, PostgreSQL for production
- **Horizontal scaling**: Stateless architecture for Kubernetes deployments
- **Multiple formats**: CBZ, CBR, EPUB, PDF support
- **Fast metadata extraction**: ComicInfo.xml parsing, page info, and file hashes
- **Flexible organization**: Per-library scanning strategies

## Quick Start

Build the project:

```bash
cargo build --release
```

Start the server:

```bash
codex serve --config codex.yaml
```

See [config/config.sqlite.yaml](config/config.sqlite.yaml) for configuration options.

## Development Setup

After cloning, install pre-commit hooks to ensure OpenAPI files stay in sync:

```bash
make setup-hooks
```

This requires Python and will install [pre-commit](https://pre-commit.com/) if not already installed.

## CBR Support and Licensing

CBR (Comic Book RAR) archive support requires the UnRAR library, which uses a **proprietary license** (not standard open source). The UnRAR license allows free use for extraction but prohibits creating RAR compression software.

### Building with CBR Support (Default)

By default, Codex includes CBR support:

```bash
cargo build --release
```

### Building without CBR Support

**If you don't want proprietary dependencies**, build without the `rar` feature:

```bash
cargo build --release --no-default-features
```

This removes the UnRAR dependency and disables CBR parsing. All other formats (CBZ, EPUB, PDF) will continue to work normally.

### Testing

```bash
# Test with CBR support (default)
cargo test

# Test without CBR support
cargo test --no-default-features

# Test CBR parser specifically (requires manual test files)
cargo test --features rar cbr_parser
```

For more details, see the [UnRAR license](https://www.rarlab.com/license.htm).

## Documentation

- [Getting Started](https://codex.4sh.dev/docs/getting-started)
- [Configuration](https://codex.4sh.dev/docs/configuration)
- [API Documentation](https://codex.4sh.dev/docs/api/codex-api)
- [Troubleshooting](https://codex.4sh.dev/docs/troubleshooting)
- [Changelog](CHANGELOG.md)

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE).

For commercial licensing options, contact [@AshDevFr](https://github.com/AshDevFr).
