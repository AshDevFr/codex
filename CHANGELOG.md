# Changelog

All notable changes to Codex will be documented in this file.

## [1.0.0] - 2026-01-26

**Initial Release** - Codex is a next-generation digital library server for comics, manga, and ebooks.

### Core Features

- **Multi-format support**: CBZ, CBR (with UnRAR), EPUB, and PDF
- **Dual database support**: SQLite for simplicity, PostgreSQL for scale
- **Library management**: Multiple libraries with customizable scanning strategies
- **Series organization**: Automatic series detection with configurable naming strategies

### Reading Experience

- **Built-in readers**: Comic reader with continuous scroll, EPUB reader with typography customization
- **Reading progress**: Track progress across devices with read/unread status
- **Incognito mode**: Read without tracking progress
- **Auto-advance**: Automatically continue to the next book in a series

### API & Integrations

- **REST API**: Full-featured API with OpenAPI documentation
- **OPDS 1.2 & 2.0**: Compatible with e-readers and reading apps
- **Komga-compatible API**: Works with third-party apps like Komic
- **Real-time updates**: Server-Sent Events for live progress and notifications

### User Management

- **Authentication**: JWT tokens, API keys, and cookie-based auth
- **Permissions**: Role-based access with sharing tags
- **Multi-user**: Support for multiple users with individual preferences

### Administration

- **Web UI**: Modern React-based interface with Mantine components
- **Scheduled scanning**: Cron-based automatic library updates
- **Thumbnail generation**: Automatic cover thumbnails with caching
- **Custom branding**: Personalize the application name and appearance
- **Rate limiting**: Configurable request throttling

### Deployment

- **Docker support**: Official images with PUID/PGID support
- **Horizontal scaling**: Stateless architecture with separate worker containers
- **Graceful shutdown**: Clean handling of in-flight requests
