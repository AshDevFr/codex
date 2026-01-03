# Codex

A next-generation digital library server for comics, manga, and ebooks built in Rust. Designed to scale horizontally while remaining simple for homelab deployments.

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

See [codex-sample.yaml](codex-sample.yaml) for configuration options.

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

## Project Status

Currently in Phase 1 (MVP Core). See [implementation docs](tmp/impl/overview.md) for the roadmap.

## License

MIT
