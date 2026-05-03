---
---

# Book Metadata

Codex supports rich metadata for books including type classification, structured authors, awards, subjects, and more. This guide covers book types and the extended metadata fields.

## Book Types

Every book can be classified with a type that describes what kind of publication it is. Book types appear as color-coded badges throughout the UI.

| Type | Description |
|------|-------------|
| **Comic** | Western comic books and comic series |
| **Manga** | Japanese manga volumes |
| **Novel** | Full-length novels |
| **Novella** | Short novels or novellas |
| **Anthology** | Collections of short stories |
| **Artbook** | Art collections and illustration books |
| **Oneshot** | Single standalone issues |
| **Omnibus** | Combined/collected volumes |
| **Graphic Novel** | Graphic novels and visual narratives |
| **Magazine** | Magazines and periodicals |

### Setting Book Type

Book type can be set in several ways:

1. **Metadata plugins** - Plugins like Open Library can automatically detect and set book types
2. **Manual editing** - Use the book edit modal (Publication tab) to set the type
3. **API** - Use the `PATCH /api/v1/books/{id}/metadata` endpoint

### Locking Book Type

Like other metadata fields, book type supports locking. When locked, metadata plugins cannot overwrite the value during automatic metadata fetching. Lock a field by toggling the lock icon next to it in the edit modal.

## Volume and Chapter Numbers

Each book carries an optional **volume** (integer) and **chapter** (decimal) number. Codex uses these to classify each book and to compute per-series aggregates (`local_max_volume`, `local_max_chapter`, `volumes_owned`) that drive the series count display and "behind by N" indicators.

How a book is classified depends on which fields are populated:

| `volume` | `chapter` | Book detail badge | Meaning |
|----------|-----------|-------------------|---------|
| set | unset | `Vol N` (blue) | A bound volume. |
| unset | set | `Ch N` (grape) | A loose chapter. |
| set | set | `Vol V · Ch C` (blue) | A specific chapter inside a known volume. |
| unset | unset | `Vol` (gray, muted) | Unclassified; defaults to volume. Click to edit. |

### Filename Convention

When the library uses the **Filename** or **Smart** [book strategy](./scanning-strategies/book-strategies), Codex extracts `volume` and `chapter` from the filename using these canonical patterns:

| Pattern | Example | Result |
|---------|---------|--------|
| `vN`, `vol.N`, `volume N` | `One Piece v01.cbz` | `volume = 1` |
| `cN`, `ch.N`, `chapter N` | `One Piece c042.cbz` | `chapter = 42` |
| Both, separated by anything non-alphanumeric | `One Piece v15 - c126 (2023).cbz` | `volume = 15`, `chapter = 126` |
| Fractional chapters | `One Piece c042.5.cbz` | `chapter = 42.5` |

Rules and edge cases:

- The prefix (`v` / `vol` / `volume`, `c` / `ch` / `chapter`) is required and **case-insensitive**. Bare numbers without a prefix are not parsed into the volume or chapter axis. They may still be parsed as the **book number** for sort order; see [book strategies](./scanning-strategies/book-strategies).
- The prefix must sit on a **non-alphanumeric left boundary** (start of name, space, underscore, dash, `[`, or `(`). This prevents uploader tags like `[GroupName]` and words containing `c` mid-string from triggering false matches.
- **Fractional volumes are rejected.** `Series v01.5.cbz` produces `volume = NULL` because the volume column is an integer. Use the chapter axis for fractional values.
- **First match wins per axis.** If a filename contains multiple volume markers (e.g. accidental duplication), the first one is used.
- A bare year in parentheses or after a dash (e.g. `v01 - 2024 (Digital).cbz`) is **not** mistaken for a chapter number. Only `c`-prefixed numbers are read as chapters.

:::tip Recommended naming
For volume-organized libraries, name files `Series Name v01.cbz`, `Series Name v02.cbz`, etc. For chapter-organized libraries, name files `Series Name c001.cbz`. For mixed libraries with bound volumes plus loose chapters, mix both forms or use `v15 - c126.cbz` for chapters known to belong to a specific volume.
:::

### ComicInfo.xml Override

When a CBZ contains a `ComicInfo.xml`, the embedded `<Volume>` and `<Number>` tags take precedence over filename parsing if the [book strategy](./scanning-strategies/book-strategies) is **Smart** or **Metadata First**. Specifically:

- **Filename strategy**: ComicInfo is ignored. Filename is the only source.
- **Metadata First strategy**: ComicInfo is the only source. Filename is ignored.
- **Smart strategy**: ComicInfo first, filename fallback if ComicInfo doesn't carry the field.

This means a series that drops to using filename naming after dropping ComicInfo (or vice versa) keeps the same volume/chapter values across rescans on the **Smart** strategy.

### Custom Regex (Non-Canonical Filenames)

If your filenames don't match the canonical patterns above, configure a [custom book strategy](./scanning-strategies/book-strategies#custom) with a regex that names `volume` and `chapter` capture groups. See [Configuration Examples](./scanning-strategies/examples) for worked examples covering scanlation-bracketed releases, `SxxExx`-style episode numbering, and other non-canonical layouts.

### Locking Volume and Chapter

Like every other metadata field, volume and chapter have **independent lock toggles**. Setting a value through the manual edit modal locks that field automatically, so a future rescan won't clobber it. You can lock or unlock either field independently in the edit modal's Publication tab.

## Extended Metadata Fields

Books support additional metadata fields beyond the basic title, summary, and publisher:

### Publication Fields

| Field | Description |
|-------|-------------|
| **Subtitle** | Book subtitle (e.g., "A Novel") |
| **Edition** | Edition information (e.g., "First Edition", "Revised") |
| **Original Title** | Title in the original language |
| **Original Year** | Year of first publication in original language |
| **Series Position** | Position within a numbered series (e.g., 1.0, 2.5) |
| **Series Total** | Total number of entries in the series |
| **Translator** | Translator name (for translated works) |

### Classification Fields

| Field | Description |
|-------|-------------|
| **Subjects** | Subject categories (e.g., "Science Fiction", "Space Exploration") |
| **Book Type** | Publication type classification (see above) |
| **Volume** | The volume this book belongs to (integer). Optional. |
| **Chapter** | The chapter this book contains (decimal, e.g. `42.5`). Optional. |

### Credits Fields

| Field | Description |
|-------|-------------|
| **Authors** | Structured author list with roles (author, co-author, editor, translator, illustrator, contributor) |
| **Awards** | Awards and nominations with year and category |

### Editing Metadata

All extended fields can be edited through the book edit modal:

1. Navigate to a book's detail page
2. Click the **Edit** button
3. Use the tabs to navigate between field groups:
   - **General** - Title, summary, publisher, year
   - **Publication** - Book type, subtitle, edition, original title/year, series position, translator, subjects
   - **Authors** - Structured author list with roles
   - **Tags** - Genre and tag management
   - **Links** - External links
   - **Cover** - Cover image management

## External Sources

Books can track their metadata sources through external IDs. When a metadata plugin fetches information for a book, the external ID is stored so future re-fetches can skip the search step.

### Supported Sources

| Source | Description |
|--------|-------------|
| `plugin:openlibrary` | Open Library metadata plugin |
| `epub` | ISBN extracted from EPUB metadata |
| `pdf` | ISBN extracted from PDF metadata |
| `manual` | Manually entered by user |

### API Endpoints

Manage external IDs via the API:

```bash
# List external IDs for a book
GET /api/v1/books/{id}/external-ids

# Add an external ID
POST /api/v1/books/{id}/external-ids
{
  "source": "manual",
  "externalId": "978-0553418026",
  "externalUrl": "https://openlibrary.org/isbn/978-0553418026"
}

# Delete an external ID
DELETE /api/v1/books/{id}/external-ids/{external_id_id}
```

## Book Covers

Books support multiple cover images from different sources. One cover is selected as the primary display cover.

### Cover Sources

| Source | Description |
|--------|-------------|
| `extracted` | Cover extracted from the book file (EPUB, PDF, CBZ) |
| `plugin:openlibrary` | Cover downloaded from Open Library |
| `custom` | User-uploaded cover image |
| `url` | Cover downloaded from a user-provided URL |

### API Endpoints

```bash
# List all covers for a book
GET /api/v1/books/{id}/covers

# Select a cover as primary
PUT /api/v1/books/{id}/covers/{cover_id}/select

# Reset cover selection
DELETE /api/v1/books/{id}/covers/selected

# Get cover image
GET /api/v1/books/{id}/covers/{cover_id}/image

# Delete a cover
DELETE /api/v1/books/{id}/covers/{cover_id}
```

## Next Steps

- [Filtering & Search](./filtering) - Filter by book type
- [Open Library Plugin](./plugins/open-library) - Automatic metadata fetching
- [Custom Metadata](./custom-metadata) - Custom metadata templates
- [Libraries](./libraries) - Library management
