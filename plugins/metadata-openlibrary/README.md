# Open Library Metadata Plugin

A Codex metadata plugin that fetches book metadata from [Open Library](https://openlibrary.org), a free and open book database with extensive ISBN coverage.

## Features

- **ISBN Lookup**: Direct, accurate book matching by ISBN-10 or ISBN-13
- **Title Search**: Fuzzy search by title and/or author
- **Cover Images**: Fetches cover images in multiple sizes (small, medium, large)
- **Author Resolution**: Resolves author references to full names
- **Subject Extraction**: Extracts subjects/topics from Open Library data
- **Ratings**: Includes Open Library community ratings when available

## Installation

### From npm

```bash
npm install @ashdev/codex-plugin-metadata-openlibrary
```

### From source

```bash
cd plugins/metadata-openlibrary
npm install
npm run build
```

## Configuration

Register the plugin in Codex with optional configuration:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `maxResults` | number | 10 | Maximum search results to return (1-50) |

### Example Configuration

```yaml
plugins:
  - name: metadata-openlibrary
    config:
      maxResults: 20
```

## API Endpoints Used

The plugin uses the following Open Library API endpoints:

| Endpoint | Usage |
|----------|-------|
| `/isbn/{isbn}.json` | Direct ISBN lookup (edition data) |
| `/works/{id}.json` | Work details (description, subjects) |
| `/authors/{id}.json` | Author name resolution |
| `/search.json` | Title/author search |

### Cover Image URLs

Cover images are fetched from:

```
https://covers.openlibrary.org/b/isbn/{isbn}-{S|M|L}.jpg
https://covers.openlibrary.org/b/id/{cover_id}-{S|M|L}.jpg
```

## Field Mapping

| Open Library | Codex Field | Notes |
|--------------|-------------|-------|
| `title` | `title` | |
| `subtitle` | `subtitle` | |
| `authors[].name` | `authors` | Resolved to full names with role="author" |
| `publishers[0]` | `publisher` | First publisher |
| `publish_date` | `year` | Parsed to extract year |
| `isbn_13` / `isbn_10` | `isbns` | All ISBNs, preferring ISBN-13 |
| `subjects` | `subjects` | Combined from edition and work |
| `description` | `summary` | From edition or work |
| `first_publish_date` | `originalYear` | From work data |
| `languages[0]` | `language` | Converted to BCP47 format |
| `key` | `externalId` | Work or edition key |
| `covers[0]` | `covers` | S/M/L URLs generated |
| `ratings_average` | `rating` | Normalized 0-100 scale |

## Supported Content Types

This plugin only provides **book** metadata (not series). It's designed for:

- EPUBs with ISBN metadata
- PDFs with ISBN information
- Novels and non-fiction books
- Graphic novels (limited support)

For manga and comics, consider using a dedicated manga/comic metadata plugin.

## Search Behavior

### ISBN Search

When an ISBN is provided:
1. Direct lookup via `/isbn/{isbn}.json`
2. Returns single result with 100% confidence
3. Falls back to title search if ISBN not found

### Title Search

When searching by title/author:
1. Searches via `/search.json`
2. Returns multiple results ranked by relevance
3. Relevance score based on: author data, ISBN availability, cover, year, subjects, ratings

### Match Behavior

The `match` method tries to automatically identify a book:
1. **ISBN match** (if available): Highest confidence (99%)
2. **Title match**: Lower confidence (max 85%), boosted by title similarity and year match

## Rate Limiting

Open Library recommends limiting requests to 100 per 5 minutes. The plugin includes:

- 15-minute response caching
- Respectful request headers
- Single concurrent request pattern

## Examples

### Search by ISBN

```typescript
// Plugin will fetch: https://openlibrary.org/isbn/9780553418026.json
const results = await plugin.searchBooks({
  isbn: "978-0-553-41802-6"
});
```

### Search by Title

```typescript
// Plugin will search: https://openlibrary.org/search.json?q=The+Martian&author=Andy+Weir
const results = await plugin.searchBooks({
  query: "The Martian",
  author: "Andy Weir"
});
```

### Get Full Metadata

```typescript
// Fetches edition, work, and author data
const metadata = await plugin.getBook({
  externalId: "/works/OL17091769W"
});
```

## Development

### Build

```bash
npm run build
```

### Test

```bash
npm test
```

### Lint

```bash
npm run lint
```

### Type Check

```bash
npm run typecheck
```

## API Documentation

For detailed API documentation, see:
- [Open Library API](https://openlibrary.org/developers/api)
- [Open Library Covers API](https://openlibrary.org/dev/docs/api/covers)

## License

MIT
