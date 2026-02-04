# Changelog

All notable changes to Codex will be documented in this file.

## [1.8.1] - 2026-02-04

### ⚙️ Miscellaneous Tasks

- Add metadata-openlibrary plugin to build and CI workflows

## [1.8.0] - 2026-02-04

### 🚀 Features

- *(book-metadata)* Add comprehensive book metadata system with plugin support
- *(parsers)* Add sidecar metadata support for OPF and Mylar series.json

### 🐛 Bug Fixes

- *(migration)* Correct credential_delivery CHECK constraint values for PostgreSQL

## [1.7.1] - 2026-02-03

### 🚀 Features

- *(komga)* Add sort fields, condition filters, and integration tests for Komic compatibility

## [1.7.0] - 2026-02-03

### 🚀 Features

- *(tasks)* Handle rate-limited tasks with rescheduling instead of retries

## [1.6.3] - 2026-02-02

### 🐛 Bug Fixes

- *(komga)* Book and series search with library filter

## [1.6.2] - 2026-02-02

### 🐛 Bug Fixes

- *(komga)* Add missing series read-progress endpoints
- *(web)* Prevent navigation when triggering library menu actions

## [1.6.1] - 2026-02-02

### 🐛 Bug Fixes

- *(plugins)* Use package.json version in manifests instead of hardcoded values

## [1.6.0] - 2026-02-02

### 🚀 Features

- *(web)* Change default books per page to 50 in series detail
- *(plugin)* Add book count to metadata search results
- *(metadata)* Add cover lock to preserve user cover selection during auto-fetch
- *(release)* Add openapi-all step to release-prepare

### 🐛 Bug Fixes

- *(web)* Prevent focus loss when typing in preprocessing rule fields
- *(web)* Add cover lock field to mocks and tests

## [1.5.0] - 2026-02-02

### 🚀 Features

- *(api)* Add GET /api/v1/user endpoint for current user profile
- *(scanner)* Add ZeroPages error type for books with no pages
- *(scripts)* Add komga-sync tool for reading progress migration
- *(plugin)* Add internal retry with exponential backoff for rate-limited requests

### 🐛 Bug Fixes

- *(pdf)* Handle PDFs with indirect Kids array references in page tree
- *(api)* Include ZeroPages in retry error type handlers
- *(web)* Add zero_pages error type to bookErrors utility
- *(web)* Display completion and hasExternalIds filters in active filters bar

### 🎨 Styling

- *(api)* Standardize JSON field names to camelCase across v1 API

## [1.4.0] - 2026-02-02

### 🚀 Features

- Add external source ID filter and improve metadata display
- *(plugins)* Add config schema support and improve plugin logging

### 🐛 Bug Fixes

- *(tasks)* Handle claim race condition gracefully instead of logging error
- *(docker)* Add profiles to postgres service for dev and prod environments
- *(plugins)* Correctly clear search template when set to null
- *(mangabaka)* Strip HTML tags from summaries

### 🚜 Refactor

- *(setup)* Simplify setup wizard to single settings step

## [1.3.0] - 2026-02-01

### 🚀 Features

- *(events)* Add SSE events for plugin configuration changes
- Add preprocessing rules, search templates, auto-match conditions, and external ID tracking

### 🐛 Bug Fixes

- *(docker)* Support custom user directive in docker-compose

### 📚 Documentation

- Add branded social card and hero banner images

## [1.2.0] - 2026-01-31

### 🚀 Features

- *(web)* Upgrade Mantine to v8 and refine card styling
- *(filters)* Add series completion filter

### 🐛 Bug Fixes

- *(metadata)* Update title_sort automatically when title changes via plugin
- *(ci)* Add docker job dependency to release step
- *(plugins)* Improve npx support with npm cache dir and absolute path handling
- *(plugins)* Prevent panic when logging UTF-8 responses with multi-byte chars

## [1.1.0] - 2026-01-31

### 🚀 Features

- Add plugin system for external metadata providers
- Add plugin metrics tracking and scheduled thumbnail generation
- Add bulk operations and info modals for books and series

### ⚙️ Miscellaneous Tasks

- *(screenshots)* Fix gitignore to exclude all generated output files

## [1.0.1] - 2026-01-28

### 🚀 Features

- *(docs)* Add automated screenshot capture system with Playwright

### 🐛 Bug Fixes

- *(web)* Show focus ring only for keyboard navigation
- *(web)* Fix PDF fit modes not working on initial page load

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
