# Changelog

All notable changes to Codex will be documented in this file.

## [1.11.2] - 2026-02-15

### 🐛 Bug Fixes

- *(migration)* Handle SQLite blob UUIDs and PostgreSQL INT8 in consolidate_authors

## [1.11.1] - 2026-02-15

### 🐛 Bug Fixes

- *(migration)* Make consolidate_authors migration idempotent for partial failure recovery

## [1.11.0] - 2026-02-15

### 🚀 Features

- *(books)* Add many-to-many genre/tag support, custom metadata editor, and title_sort_lock
- *(books)* Enrich BookDetail page with full metadata and generic badge chips
- *(plugins)* Add externalIds support to PluginBookMetadata for book cross-references
- *(authors)* Consolidate individual author columns into unified authors_json

### 🐛 Bug Fixes

- *(covers)* Extract shared CoverEditor and fix book cover selection/thumbnail pipeline
- *(books)* Expose title/number lock fields and fix tooltip z-index in metadata modals
- *(pdf)* Use per-page content detection to correctly render mixed-content PDFs

### 📚 Documentation

- *(showcase)* Promote plugins to dedicated section and add missing screenshots
- Replace GitHub links with feature board and remove binary download references

### ⚙️ Miscellaneous Tasks

- *(ui)* Increase sync task polling interval to 3 seconds

## [1.10.9] - 2026-02-13

### 🚀 Features

- *(ui)* Redesign recommendation cards with float-based grid layout
- *(filter)* Add hasUserRating filter and fix rating sort for PostgreSQL
- *(plugins)* Add user-scoped plugin task endpoint for sync status visibility

### 🐛 Bug Fixes

- *(release)* Improve release-prepare with SDK versioning and progress output
- *(recommendations)* Allow dismissing recommendations already available in Codex

## [1.10.8] - 2026-02-13

### 🚀 Features

- *(recommendations)* Move settings to server-side with per-user seed curation

### 🐛 Bug Fixes

- *(ui)* Remove duplicate "Based on" text in recommendation cards
- *(recommendations)* Only exclude read series and enrich instead of filtering local matches

## [1.10.7] - 2026-02-13

### 🚀 Features

- *(api)* Add glob pattern support for rate limit exempt paths
- *(ui)* Display description preview in metadata search results

### 🐛 Bug Fixes

- *(ui)* Correct series download button endpoint URL
- *(api)* Use FlexibleAuthContext for series download to support cookie auth

## [1.10.6] - 2026-02-13

### 🚀 Features

- *(plugins)* Add searchURITemplate to plugin manifests with external search link in metadata modal
- *(ui)* Add dynamic document titles to series, book, reader, and search pages
- *(ui)* Add collapsible overflow for tags and alternate titles on series detail page
- *(api)* Add sort by rating for series (user, community, external)

### 🐛 Bug Fixes

- *(scheduler)* Normalize 5-part cron expressions for tokio-cron-scheduler compatibility
- *(recommendations)* Filter local series, populate exclude_ids, and add polling fallback
- *(api)* Implement book count sorting for series queries

## [1.10.5] - 2026-02-12

### 🚀 Features

- *(screenshots)* Rewrite plugin scenarios for official store and update docs

### 🐛 Bug Fixes

- *(plugins)* Clamp mangabaka search limit to 50, add sort_by config, handle null JSON-RPC result

## [1.10.4] - 2026-02-12

### 🚀 Features

- *(metadata)* Add series metadata reset endpoint with bulk support and UI
- *(plugins)* Add official plugin store with 3D flip card carousel

### 🚜 Refactor

- *(plugins)* Extract General tab in plugin config modal

## [1.10.3] - 2026-02-12

### 🚀 Features

- *(recommendations)* Make GET and dismiss endpoints non-blocking with SSE task tracking
- *(plugins)* Add internal_config with search results limit and external ID lookup in search modal
- *(external-ids)* Add series external ID CRUD endpoints and edit modal for series/books
- *(recommendations)* Add "View in Library" button and link to series detail page
- *(reader)* Improve volume change overlay with boundary-aware navigation
- *(recommendations)* Add rating, popularity, status, volume count, and in-codex enrichment

### 🐛 Bug Fixes

- *(plugins)* Detect RPC-level rate limits in task retry system
- *(recommendations)* Persist plugin response to DB so GET endpoint can read cached data
- *(ui)* Use router location for sidebar active state instead of static route patterns

## [1.10.2] - 2026-02-11

### 🚀 Features

- *(plugins)* Add configurable task timeout via DB settings
- *(plugins)* Add private and hiddenFromStatusLists visibility controls to AniList sync

### 🐛 Bug Fixes

- Renumber series books on delete and restore, clear deleted book numbers
- *(plugins)* Prevent plugin process leaks by aborting reader task on drop
- *(plugins)* Map plugin errors to proper HTTP status codes instead of blanket 500s

## [1.10.1] - 2026-02-11

### 🚀 Features

- *(mangabaka)* Sort search results by title similarity scoring

### 🐛 Bug Fixes

- Stop recording reading progress on v1 page fetch, move to OPDS-only endpoint
- *(ui)* Swap sort by and page size positions in library toolbar
- Use natural sort order for book numbering

## [1.10.0] - 2026-02-11

### 🚀 Features

- *(plugins)* Add user plugin system database schema and core infrastructure
- *(plugins)* Add OAuth authentication flow and user plugin API endpoints
- *(plugins)* Add plugin storage system with bidirectional RPC and SDK support
- *(plugins)* Add user plugin settings UI, management API, and OpenAPI registration
- *(plugins)* Add sync provider protocol, background task, and AniList reference plugin
- *(plugins)* Add recommendation engine protocol, API, UI, and AniList reference plugin
- *(plugins)* Add cross-reference external IDs to metadata protocol
- *(plugins)* Match sync-pulled entries to series via externalIdSource
- *(plugins)* Add bidirectional sync, personal access tokens, and user plugin settings
- *(plugins)* Add configurable push with volumes/chapters and fix credential delivery
- *(plugins)* Add sync error reporting, improve AniList docs, and simplify pull query
- *(plugins)* Add rating and notes sync to AniList plugin
- *(plugins)* Add auto-pause and auto-drop for stale AniList series
- *(plugins)* Add recommendation plugin UI, task handler, and library data builder
- *(plugins)* Split InitializeParams config and enrich sync protocol
- *(plugins)* Replace PushConfig with CodexSyncSettings and separate server/plugin concerns
- *(plugins)* Make reference plugins showcase-quality with tests and hardening
- *(plugins)* Add batch query methods and eliminate N+1 patterns in sync
- *(plugins)* Wire OAuth token refresh into plugin manager
- *(plugins)* Structural cleanup — split large files, wire PluginStorage, add OAuth timeout
- *(plugins)* Add security hardening — OAuth cleanup scheduling, storage quotas, and rate limiting
- *(plugins)* Deduplicate sync and recommendation tasks per user+plugin
- *(plugins)* Add fetch timeouts, rate-limit retry, pagination, and configurable seeds
- *(plugins)* Add searchFallback user config toggle to recommendations-anilist
- *(plugins)* Add search fallback support to sync-anilist plugin

### 🐛 Bug Fixes

- *(mangabaka)* Prioritize exact title matches in auto-match scoring
- *(plugins)* Use consistent "integration" terminology in user-facing strings
- *(docker)* Add missing recommendations-anilist and sync-anilist plugins
- *(plugins)* Only show Metadata Provider badge when capability is non-empty
- *(web)* Improve plugin form placeholders
- *(plugins)* Infer plugin_type from manifest capabilities on manifest sync
- *(plugins)* Fix IntegrationsSettings test and apply formatting
- *(plugins)* Replace string matching with structured OAuth error classification and add circuit breaker

### 🚜 Refactor

- *(plugins)* Consolidate SDK types and deduplicate server boilerplate
- *(plugins)* Remove deprecated syncProvider in favor of userSyncProvider
- *(web)* Replace manual type definitions with generated OpenAPI types
- *(web)* Move plugin permissions to capability-aware configure modal
- *(plugins)* Rename userSyncProvider to userReadSync across codebase
- *(plugins)* Rename recommendationProvider to userRecommendationProvider across codebase
- *(plugins)* Split PluginConfigModal into sub-components and generate OpenAPI recommendation types
- *(plugins)* Remove deprecated SDK exports and replace magic strings with constants
- *(plugins)* Replace blanket dead_code suppressions with targeted annotations
- *(plugins)* Remove ~540 lines of dead code and fix unsafe ApiError type cast

### 📚 Documentation

- *(plugins)* Add security model, privacy, and OAuth troubleshooting documentation
- *(plugins)* Rewrite developer documentation to reflect current plugin system
- *(plugins)* Document encryption key setup and rotation procedure

### ⚡ Performance

- *(plugins)* Use database-level JSON filtering for task deduplication

### 🧪 Testing

- *(plugins)* Add recommendation API data transformation and error path tests
- *(plugins)* Add recommendation scoring, ID resolution, and merge logic tests
- *(plugins)* Add searchFallback toggle and title-based search tests

### ⚙️ Miscellaneous Tasks

- *(plugins)* Auto-discover plugins in Makefile, add new plugins to CI

## [1.9.3] - 2026-02-07

### 🐛 Bug Fixes

- *(api)* Use COALESCE for title sort in v1 series endpoints to handle NULL title_sort values

## [1.9.2] - 2026-02-07

### 🐛 Bug Fixes

- *(komga)* Set page to last page on mark-as-read and sort on-deck by recency
- *(reader)* Correct progress bar click mapping in RTL mode
- *(komga)* Use COALESCE for title sort to handle NULL title_sort values

## [1.9.1] - 2026-02-07

### 🚀 Features

- *(progress)* Auto-detect book completion when current page reaches page count

## [1.9.0] - 2026-02-06

### 🚀 Features

- *(auth)* Add OIDC configuration and data model
- *(auth)* Implement OIDC service core
- *(auth)* Add OIDC API endpoints
- *(auth)* Add OIDC frontend integration
- *(auth)* Improve OIDC config, role sync, and permission-aware frontend
- *(auth)* Add OIDC tests, documentation, and role mapping env overrides

### 🐛 Bug Fixes

- *(duplicates)* Exclude books with empty file_hash from duplicate detection

### ⚙️ Miscellaneous Tasks

- Upgrade to Rust 2024 edition
- Increase test parallelism from 3 to 5 partitions

## [1.8.5] - 2026-02-05

### 🐛 Bug Fixes

- *(komga)* Populate genres, tags, links, authors, and alternate titles in series endpoint
- *(komga)* Populate book metadata fields (authors, tags, summary, dates)

### ⚙️ Miscellaneous Tasks

- Fix gh CLI auth failure in release job

## [1.8.4] - 2026-02-05

### 🐛 Bug Fixes

- *(komga)* Restore oneshot fields removed in book-metadata commit

### 📚 Documentation

- *(plugins)* Add Open Library plugin and update architecture diagram

### 🎨 Styling

- Fix formatting in docs, ASCII diagrams, and markdown tables

## [1.8.3] - 2026-02-05

### 🐛 Bug Fixes

- *(reader)* Improve double-page spread handling and RTL progress bar
- *(reader)* Fix double-page spread initialization and page jump on load

### 📚 Documentation

- *(screenshots)* Update documentation screenshots and add expanded plugin details view

## [1.8.2] - 2026-02-04

### 🚀 Features

- *(plugins)* Enhance plugin details with two-column layout, metadata targets, and search config indicators

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
