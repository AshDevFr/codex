# Database Module

This module contains the database models and data access layer for Codex.

## Overview

The database layer is designed to support both SQLite (for homelab/single-instance) and PostgreSQL (for production/multi-instance) deployments. We use SQLx for compile-time checked SQL queries.

## Models

### Core Models

#### Library

Top-level container for content collections.

**Fields:**

- `id` (UUID): Unique identifier
- `name` (String): Human-readable library name
- `path` (String): Filesystem path to content
- `scanning_strategy` (String): Strategy for organizing content
- `scanning_config` (JSON String): Strategy-specific configuration
- `created_at`, `updated_at`, `last_scanned_at` (DateTime)

**Scanning Strategies:**

- `komga_compatible`: Direct child folders = series
- `volume_chapter`: Parent folder = series, child folders = volumes
- `flat`: All files at root, series from filename/metadata
- `publisher_hierarchy`: Skip first N levels then apply Komga rules
- `custom`: User-defined regex patterns

#### Series

Collection of related books within a library.

**Fields:**

- `id` (UUID): Unique identifier
- `library_id` (UUID): Parent library reference
- `name` (String): Series name
- `normalized_name` (String): Lowercase, alphanumeric for searching
- `sort_name` (String, optional): Custom sort order
- `summary` (String, optional): Series description
- `publisher` (String, optional): Publisher name
- `year` (i32, optional): Publication year
- `book_count` (i32): Number of books in series
- **Rating Fields:**
  - `user_rating` (f32, optional): User's personal rating (0.0-10.0)
  - `external_rating` (f32, optional): Rating from external source (0.0-10.0)
  - `external_rating_count` (i32, optional): Number of ratings from external source
  - `external_rating_source` (String, optional): Source name (e.g., "anilist", "mangaupdates")
- **Custom Metadata:**
  - `custom_metadata` (JSON String, optional): User-defined metadata fields
- `created_at`, `updated_at` (DateTime)

**Helper Methods:**

- `set_user_rating(rating)`: Set user rating with validation (0.0-10.0)
- `set_external_rating(rating, count, source)`: Set external rating from integration
- `get_custom_metadata_json()`: Parse custom metadata as JSON
- `set_custom_metadata(json)`: Store custom metadata from JSON value

#### Book

Individual file in a series.

**Fields:**

- `id` (UUID): Unique identifier
- `series_id` (UUID): Parent series reference
- `title` (String, optional): Book title
- `number` (f32, optional): Book number in series (can be fractional)
- `file_path` (String): Full filesystem path
- `file_name` (String): Just the filename
- `file_size` (i64): Size in bytes
- `file_hash` (String): SHA-256 hash for duplicate detection
- `format` (String): File format (cbz, cbr, epub, pdf)
- `page_count` (i32): Number of pages
- `modified_at` (DateTime): File modification time
- `created_at`, `updated_at` (DateTime)

**Helper Methods:**

- `get_format()`: Convert string to `FileFormat` enum
- `set_format()`: Set format from enum

#### BookMetadataRecord

Extended metadata extracted from ComicInfo.xml, EPUB metadata, etc.

**Fields:**

- `id` (UUID): Unique identifier
- `book_id` (UUID): Parent book reference
- **Content Metadata:**
  - `summary`, `writer`, `penciller`, `inker`, `colorist`, `letterer`
  - `cover_artist`, `editor`, `publisher`, `imprint`, `genre`
  - `web`, `language_iso`, `format_detail`
  - `black_and_white`, `manga` (booleans)
- **Date Information:**
  - `year`, `month`, `day` (integers)
- **Series Information:**
  - `volume`, `count` (total books in series)
- **Identifiers:**
  - `isbns` (JSON array as string)
- `created_at`, `updated_at` (DateTime)

#### Page

Individual page within a book.

**Fields:**

- `id` (UUID): Unique identifier
- `book_id` (UUID): Parent book reference
- `page_number` (i32): 1-indexed page number
- `file_name` (String): Filename within archive
- `format` (String): Image format (jpeg, png, webp, etc.)
- `width`, `height` (i32): Dimensions in pixels
- `file_size` (i64): Size in bytes
- `created_at` (DateTime)

**Helper Methods:**

- `get_image_format()`: Convert string to `ImageFormat` enum
- `set_image_format()`: Set format from enum

### User & Progress Models

#### User

User accounts for authentication and authorization.

**Fields:**

- `id` (UUID): Unique identifier
- `username` (String): Unique username
- `email` (String): Email address
- `password_hash` (String): Hashed password (argon2)
- `is_admin` (bool): Admin privileges flag
- `created_at`, `updated_at`, `last_login_at` (DateTime)

#### ReadProgress

Tracks reading progress for each user.

**Fields:**

- `id` (UUID): Unique identifier
- `user_id` (UUID): User reference
- `book_id` (UUID): Book reference
- `current_page` (i32): Last page read
- `completed` (bool): Whether book is finished
- `started_at`, `updated_at`, `completed_at` (DateTime)

### Metadata Integration Models

#### MetadataSource

Tracks external metadata sources (MangaBaka, AniList, etc.).

**Fields:**

- `id` (UUID): Unique identifier
- `series_id` (UUID): Series reference
- `source_name` (String): Integration name (e.g., "mangabaka")
- `external_id` (String): ID in external system
- `external_url` (String, optional): Link to external page
- `confidence` (f32): Match confidence score (0.0-1.0)
- `metadata_json` (String): Full metadata as JSON
- `created_at`, `updated_at` (DateTime)

## Database Schema

### Relationships

```
libraries (1) ──< (N) series
series (1) ──< (N) books
books (1) ──< (N) pages
books (1) ─── (1) book_metadata_records

users (1) ──< (N) read_progress
books (1) ──< (N) read_progress

series (1) ──< (N) metadata_sources
```

### Indexes (Planned)

Performance-critical indexes to be created:

- `books.file_hash` - Fast duplicate detection
- `books.file_path` - File lookup during scanning
- `series.normalized_name` - Search functionality
- `pages.book_id, page_number` - Page retrieval
- `read_progress.user_id, book_id` - Progress lookup

## Type Conversions

### FileFormat

`CBZ`, `CBR`, `EPUB`, `PDF` stored as lowercase strings in database.

### ImageFormat

`JPEG`, `PNG`, `WEBP`, `GIF`, `AVIF`, `BMP` stored as lowercase strings.

### ScanningStrategy

Strategy enums stored as snake_case strings:

- `komga_compatible`
- `volume_chapter`
- `flat`
- `publisher_hierarchy`
- `custom`

### DateTime

All timestamps use `chrono::DateTime<Utc>` and are stored as UTC in the database.

### UUIDs

All primary keys use UUID v4 for distributed generation without coordination.

## Next Steps

1. **Create Migrations** - SQL migration files for schema creation
2. **Repository Layer** - Data access abstractions
3. **Connection Pool** - SQLx pool configuration
4. **Transaction Support** - Safe concurrent access
5. **Query Builders** - Complex query construction
6. **Seeding** - Test data generation

## Usage Example

```rust
use codex::db::{Library, Series, ScanningStrategy};

// Create a new library
let library = Library::new(
    "My Comics".to_string(),
    "/mnt/comics".to_string(),
    ScanningStrategy::KomgaCompatible,
);

println!("Library ID: {}", library.id);
println!("Strategy: {}", library.scanning_strategy);

// Create a series with ratings and custom metadata
let mut series = Series::new(
    library.id,
    "One Piece".to_string(),
);

// Set user rating
series.set_user_rating(9.5).unwrap();

// Set external rating from AniList
series.set_external_rating(8.7, Some(15234), "anilist".to_string()).unwrap();

// Add custom metadata
let custom_data = serde_json::json!({
    "reading_status": "ongoing",
    "tags": ["adventure", "shonen"],
    "notes": "Best manga ever!"
});
series.set_custom_metadata(custom_data);

// Retrieve custom metadata
let metadata = series.get_custom_metadata_json();
println!("Tags: {:?}", metadata["tags"]);
```

## Testing

Run model tests:

```bash
cargo test --lib db::models
```

All models include comprehensive unit tests covering:

- Constructor functions
- Type conversions
- Helper methods
- Field validation
