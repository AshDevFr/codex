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

## Companion Projects

- **[Tsundoku](https://github.com/AshDevFr/tsundoku)** ([docs](https://tsundoku.4sh.dev)) — a standalone manga discovery service that finds series you *don't* own yet. Where Codex's release tracking watches series already in your library, Tsundoku polls discovery sources and keeps a browsable catalog of titles not yet in Codex, reading your library over the Codex API to skip the ones you already have. See the [Codex docs](https://codex.4sh.dev/docs/tsundoku) for how the two fit together.

## Project Status & Support

Codex is a solo side project. I built it for my own use and continue to develop it because I enjoy it. A few things that follow from that:

- **No SLA.** I read everything but respond when I have time.
- **Bug reports are welcome.** Use the issue template and include version, deployment method, and relevant logs.
- **Feature requests are welcome, but I will close ones that fall outside the scope below.**
- **PRs are welcome** for bugs and small features. For larger changes, please open an issue first so we can agree on direction before you write code.
- **I don't provide installation support.** The [docs](https://codex.4sh.dev/) cover deployment. If you can't get past the docs, this project may not be a good fit yet.

### Scope

What Codex is and isn't, so you can decide if it fits:

- **Manga is the polished path.** The reader, scanner defaults, and release tracking are tuned for manga first. Books and EPUB work but are less battle-tested.
- **Metadata is plugin-driven by design.** There are no built-in scrapers. Use one of the existing plugins (Open Library, MangaBaka) or write your own with the TypeScript SDK.
- **PostgreSQL is a first-class target.** Production deployment to Kubernetes is a primary use case, not an afterthought.
- **Komga API compatibility exists specifically so Komic keeps working.** Other Komga-compatible apps may work, but they are not actively tested.
- **No i18n yet.** English only.

### Sponsoring

If Codex is useful to you and you want to support its development, you can [buy me a coffee](https://buymeacoffee.com/4shdev). Optional, and never expected. Sponsorship is a thank-you, not a service contract: the same scope and support rules apply to everyone.

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE).

For commercial licensing options, contact [@AshDevFr](https://github.com/AshDevFr).
