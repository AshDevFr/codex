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
