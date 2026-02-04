---
---

# Filtering & Search

Codex provides powerful filtering and search capabilities to help you find content in your library. This guide covers the filter UI, condition-based filtering, and full-text search.

## Quick Filters

The library view includes a filter panel with common filters:

### Accessing Filters

1. Navigate to a library
2. Click the **Filter** button in the toolbar
3. The filter panel opens as a drawer

### Filter Groups

#### Read Status

Filter by reading progress:

| Status | Description |
|--------|-------------|
| **Unread** | No reading progress on any book |
| **In Progress** | Currently reading (some progress, not completed) |
| **Read** | All books completed |

#### Publication Status

Filter by series publication status:

| Status | Description |
|--------|-------------|
| **Ongoing** | Series is still being published |
| **Ended** | Series is complete |
| **Hiatus** | Series is on hold |

#### Book Type (Books)

Filter books by their type classification:

| Type | Description |
|------|-------------|
| **Comic** | Western comic books |
| **Manga** | Japanese manga |
| **Novel** | Full-length novels |
| **Novella** | Short novels |
| **Anthology** | Story collections |
| **Artbook** | Art collections |
| **Oneshot** | Single standalone issues |
| **Omnibus** | Combined volumes |
| **Graphic Novel** | Graphic novels |
| **Magazine** | Magazines and periodicals |

#### Genres & Tags

Filter by genres and tags extracted from your media metadata (ComicInfo.xml, EPUB metadata).

### Filter Modes

Each filter group supports two modes:

- **All selected (AND)**: Series must match ALL selected values
- **Any selected (OR)**: Series must match ANY of the selected values

### Include vs Exclude

Click a filter chip to cycle through states:

| State | Visual | Behavior |
|-------|--------|----------|
| **Neutral** | Gray outline | Not applied |
| **Include** | Blue filled | Must match |
| **Exclude** | Red filled with X | Must NOT match |

### Active Filters

Active filters appear as chips below the toolbar. Click the X on any chip to remove it, or click "Clear all" to reset.

## URL Persistence

Filters are saved in the URL for easy sharing and bookmarking:

```
/library/abc123?gf=all:Action,Comedy:-Horror&sf=ongoing
```

Parameters:
- `gf`: Genre filter (`mode:include1,include2:-exclude1`)
- `tf`: Tag filter
- `sf`: Status filter
- `rf`: Read status filter
- `bbt`: Book type filter (books only)

## Advanced Filtering (API)

For complex queries, use the `POST /series/list` or `POST /books/list` endpoints.

### Condition Structure

Filters use a tree structure with combinators:

```json
{
  "condition": {
    "allOf": [
      { "genre": { "operator": "is", "value": "Action" } },
      { "genre": { "operator": "isNot", "value": "Horror" } }
    ]
  }
}
```

### Combinators

| Combinator | SQL Equivalent | Description |
|------------|---------------|-------------|
| `allOf` | AND | All conditions must match |
| `anyOf` | OR | Any condition can match |

### Field Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `is` | Equals | `{"operator": "is", "value": "Action"}` |
| `isNot` | Not equals | `{"operator": "isNot", "value": "Horror"}` |
| `isNull` | Field is null | `{"operator": "isNull"}` |
| `isNotNull` | Field has value | `{"operator": "isNotNull"}` |
| `contains` | Contains substring | `{"operator": "contains", "value": "bat"}` |
| `beginsWith` | Starts with | `{"operator": "beginsWith", "value": "The"}` |

### Series Filter Fields

| Field | Description | Operators |
|-------|-------------|-----------|
| `libraryId` | Library UUID | `is`, `isNot` |
| `genre` | Genre name | `is`, `isNot` |
| `tag` | Tag name | `is`, `isNot` |
| `status` | Publication status | `is`, `isNot` |
| `publisher` | Publisher name | `is`, `isNot`, `isNull`, `isNotNull` |
| `language` | Language code (BCP47) | `is`, `isNot`, `isNull`, `isNotNull` |
| `name` | Series name | `is`, `isNot`, `contains`, `beginsWith` |
| `readStatus` | Reading status | `is`, `isNot` |

### Book Filter Fields

| Field | Description | Operators |
|-------|-------------|-----------|
| `libraryId` | Library UUID | `is`, `isNot` |
| `seriesId` | Series UUID | `is`, `isNot` |
| `genre` | Genre name | `is`, `isNot` |
| `tag` | Tag name | `is`, `isNot` |
| `title` | Book title | `is`, `isNot`, `contains` |
| `readStatus` | Reading status | `is`, `isNot` |
| `hasError` | Has parsing error | `isTrue`, `isFalse` |
| `bookType` | Book type classification | `is`, `isNot`, `isNull`, `isNotNull` |

## Example Queries

### Simple Genre Filter

```json
{
  "condition": {
    "genre": { "operator": "is", "value": "Action" }
  }
}
```

### Multiple Genres (OR)

Match series with Action OR Comedy:

```json
{
  "condition": {
    "anyOf": [
      { "genre": { "operator": "is", "value": "Action" } },
      { "genre": { "operator": "is", "value": "Comedy" } }
    ]
  }
}
```

### Genre with Exclusion

Match Action series but exclude Horror:

```json
{
  "condition": {
    "allOf": [
      { "genre": { "operator": "is", "value": "Action" } },
      { "genre": { "operator": "isNot", "value": "Horror" } }
    ]
  }
}
```

### Complex Nested Query

(Action AND Comedy) OR (Fantasy AND NOT Horror):

```json
{
  "condition": {
    "anyOf": [
      {
        "allOf": [
          { "genre": { "operator": "is", "value": "Action" } },
          { "genre": { "operator": "is", "value": "Comedy" } }
        ]
      },
      {
        "allOf": [
          { "genre": { "operator": "is", "value": "Fantasy" } },
          { "genre": { "operator": "isNot", "value": "Horror" } }
        ]
      }
    ]
  }
}
```

### Read Status Filter

Find series currently being read:

```json
{
  "condition": {
    "readStatus": { "operator": "is", "value": "in_progress" }
  }
}
```

### Multi-Field Filter

Ongoing Action series with Favorite tag:

```json
{
  "condition": {
    "allOf": [
      { "status": { "operator": "is", "value": "ongoing" } },
      { "genre": { "operator": "is", "value": "Action" } },
      { "tag": { "operator": "is", "value": "Favorite" } }
    ]
  }
}
```

### Book Type Filter

Find all manga books:

```json
{
  "condition": {
    "bookType": { "operator": "is", "value": "manga" }
  }
}
```

Find books without a type classification:

```json
{
  "condition": {
    "bookType": { "operator": "isNull" }
  }
}
```

## Full-Text Search

Combine full-text search with condition filters:

```json
{
  "fullTextSearch": "batman",
  "condition": {
    "genre": { "operator": "is", "value": "Action" }
  }
}
```

The search is case-insensitive and matches against titles. It's combined with the condition using AND logic.

### Global Search

The header search bar searches both series and books:

1. Type at least 2 characters
2. Results appear in a dropdown grouped by type
3. Click a result to navigate, or press Enter for the full search page

## Performance Tips

1. **Use specific filters**: More specific filters run faster
2. **Combine with library_id**: Always include library_id when filtering within a library
3. **Avoid deep nesting**: Very deeply nested conditions may be slower
4. **Use pagination**: Always paginate results for large libraries

## Next Steps

- [API Documentation](./api) - Full API reference
- [Libraries](./libraries) - Library management
- [OPDS](./opds) - OPDS catalog access
