# Plugin Protocol

This document describes the JSON-RPC 2.0 protocol used for communication between Codex and plugins.

## Overview

Plugins communicate with Codex via JSON-RPC 2.0 over stdio:

- **stdin**: Receives JSON-RPC requests from Codex (one request per line)
- **stdout**: Sends JSON-RPC responses to Codex (one response per line)
- **stderr**: Logging output (visible in Codex logs)

## Message Format

### Request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "metadata/series/search",
  "params": {
    "query": "naruto",
    "limit": 10
  }
}
```

### Success Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "results": [...]
  }
}
```

### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "Rate limited, retry after 60s",
    "data": {
      "retryAfterSeconds": 60
    }
  }
}
```

## Lifecycle Methods

### initialize

Called when Codex first connects to the plugin. Sends configuration and credentials, receives the plugin manifest.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "adminConfig": { "maxResults": 10 },
    "userConfig": { "progressUnit": "volumes" },
    "credentials": { "access_token": "..." }
  }
}
```

The `params` object contains:

| Field | Type | Description |
|-------|------|-------------|
| `adminConfig` | `Record<string, unknown>` | Admin-configured plugin settings |
| `userConfig` | `Record<string, unknown>` | Per-user settings (includes `_codex` namespace for sync) |
| `credentials` | `Record<string, string>` | API keys, OAuth tokens (encrypted at rest) |

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "name": "my-plugin",
    "displayName": "My Plugin",
    "version": "1.0.0",
    "description": "A metadata provider",
    "author": "Your Name",
    "protocolVersion": "1.0",
    "capabilities": {
      "metadataProvider": ["series"]
    }
  }
}
```

### ping

Health check. Used by Codex to verify the plugin is responsive.

**Request:**

```json
{ "jsonrpc": "2.0", "id": 2, "method": "ping" }
```

**Response:**

```json
{ "jsonrpc": "2.0", "id": 2, "result": "pong" }
```

### shutdown

Called when Codex is shutting down or disabling the plugin. Plugins should clean up resources and exit.

**Request:**

```json
{ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }
```

**Response:**

```json
{ "jsonrpc": "2.0", "id": 3, "result": null }
```

## Metadata Methods

Methods are scoped by content type: `metadata/series/*` for series, `metadata/book/*` for books.

### metadata/series/search

Search for series metadata by query string.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "metadata/series/search",
  "params": {
    "query": "one piece",
    "limit": 10
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "results": [
      {
        "externalId": "12345",
        "title": "One Piece",
        "alternateTitles": ["ワンピース"],
        "year": 1997,
        "coverUrl": "https://example.com/cover.jpg",
        "relevanceScore": 0.95,
        "preview": {
          "status": "ongoing",
          "genres": ["Action", "Adventure"],
          "rating": 9.5,
          "description": "A pirate adventure..."
        }
      }
    ],
    "nextCursor": null
  }
}
```

### metadata/series/get

Get full metadata for an external ID.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "metadata/series/get",
  "params": { "externalId": "12345" }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "externalId": "12345",
    "externalUrl": "https://example.com/series/12345",
    "title": "One Piece",
    "alternateTitles": [
      { "title": "ワンピース", "language": "ja", "titleType": "native" },
      { "title": "Wan Piisu", "language": "ja-Latn", "titleType": "romaji" }
    ],
    "summary": "A long, epic pirate adventure...",
    "status": "ongoing",
    "year": 1997,
    "totalBookCount": 108,
    "language": "ja",
    "readingDirection": "rtl",
    "genres": ["Action", "Adventure", "Comedy"],
    "tags": ["Pirates", "Superpowers"],
    "authors": ["Eiichiro Oda"],
    "artists": ["Eiichiro Oda"],
    "publisher": "Shueisha",
    "coverUrl": "https://example.com/cover.jpg",
    "rating": { "score": 95, "voteCount": 100000, "source": "example" },
    "externalRatings": [
      { "score": 95, "voteCount": 100000, "source": "example" }
    ],
    "externalLinks": [
      { "url": "https://example.com/12345", "label": "Example", "linkType": "provider" }
    ]
  }
}
```

### metadata/series/match

Find best match for existing content (auto-matching during library scans).

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "metadata/series/match",
  "params": {
    "title": "One Piece",
    "year": 1997,
    "author": "Eiichiro Oda"
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "match": {
      "externalId": "12345",
      "title": "One Piece",
      "alternateTitles": [],
      "year": 1997,
      "relevanceScore": 0.95
    },
    "confidence": 0.98,
    "alternatives": []
  }
}
```

### metadata/book/search

Search for book metadata by ISBN, query, or author.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "metadata/book/search",
  "params": {
    "isbn": "978-0-306-40615-7",
    "limit": 5
  }
}
```

Parameters: `isbn`, `query`, `author`, `year`, `limit`, `cursor` (all optional, at least one of `isbn`/`query` required).

### metadata/book/get

Get full book metadata. Same request format as `metadata/series/get`. Response includes book-specific fields: `volume`, `pageCount`, `isbn`, `isbns`, `edition`, `seriesPosition`, `authors` (with roles), `covers`, `awards`, etc.

### metadata/book/match

Match a book by ISBN (preferred) or title. Parameters: `title`, `isbn`, `authors`, `year`, `publisher`.

## Sync Methods

### sync/getUserInfo

Get the authenticated user's profile on the external service.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "sync/getUserInfo",
  "params": {}
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "result": {
    "externalId": "42",
    "username": "reader123",
    "avatarUrl": "https://example.com/avatar.jpg",
    "profileUrl": "https://example.com/user/42"
  }
}
```

### sync/pushProgress

Push local reading progress to the external service.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "sync/pushProgress",
  "params": {
    "entries": [
      {
        "externalId": "12345",
        "title": "One Piece",
        "status": "reading",
        "progress": { "chapters": 50, "volumes": 5 },
        "rating": 95,
        "startedAt": "2024-01-15",
        "completedAt": null,
        "latestUpdatedAt": "2024-06-01T12:00:00Z"
      }
    ]
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "result": {
    "successes": ["12345"],
    "failures": []
  }
}
```

### sync/pullProgress

Pull reading progress from the external service.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "sync/pullProgress",
  "params": {
    "page": 1,
    "updatedSince": "2024-01-01T00:00:00Z"
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "result": {
    "entries": [
      {
        "externalId": "12345",
        "title": "One Piece",
        "status": "reading",
        "progress": { "chapters": 50, "volumes": 5 },
        "rating": 95,
        "startedAt": "2024-01-15",
        "lastReadAt": "2024-06-01",
        "latestUpdatedAt": "2024-06-01T12:00:00Z"
      }
    ],
    "hasMore": false
  }
}
```

### sync/status (Optional)

Return sync status summary.

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 13,
  "result": {
    "lastSyncAt": "2024-06-01T12:00:00Z",
    "totalEntries": 150,
    "syncedEntries": 148,
    "conflicts": 2
  }
}
```

## Recommendation Methods

### recommendations/get

Generate recommendations based on the user's library.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "method": "recommendations/get",
  "params": {
    "library": [
      {
        "seriesId": "abc",
        "title": "One Piece",
        "genres": ["Action", "Adventure"],
        "tags": ["Pirates"],
        "booksRead": 50,
        "booksOwned": 108,
        "userRating": 95,
        "externalIds": [{ "source": "api:anilist", "externalId": "21" }]
      }
    ],
    "limit": 20,
    "excludeIds": ["67890"]
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "result": {
    "recommendations": [
      {
        "externalId": "99999",
        "url": "https://example.com/series/99999",
        "title": "Naruto",
        "coverUrl": "https://example.com/naruto-cover.jpg",
        "description": "A ninja's journey...",
        "genres": ["Action", "Adventure"],
        "rating": 0.85,
        "why": "Recommended because you liked \"One Piece\""
      }
    ]
  }
}
```

### recommendations/dismiss (Optional)

Dismiss a recommendation so it won't appear again.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 21,
  "method": "recommendations/dismiss",
  "params": {
    "externalId": "99999",
    "reason": "not_interested"
  }
}
```

Dismiss reasons: `not_interested`, `already_read`, `already_owned`.

### recommendations/clear (Optional)

Clear cached recommendation data.

### recommendations/updateProfile (Optional)

Update the user's taste profile with new library data.

## Storage Methods (Plugin → Host)

Plugins can send storage requests to the host (Codex) via stdout. These are JSON-RPC requests sent *from* the plugin *to* the host.

### storage/get

```json
{ "jsonrpc": "2.0", "id": "s1", "method": "storage/get", "params": { "key": "cache-key" } }
```

### storage/set

```json
{ "jsonrpc": "2.0", "id": "s2", "method": "storage/set", "params": { "key": "cache-key", "data": {...}, "expiresAt": "2025-12-31T00:00:00Z" } }
```

### storage/delete

```json
{ "jsonrpc": "2.0", "id": "s3", "method": "storage/delete", "params": { "key": "cache-key" } }
```

### storage/list

```json
{ "jsonrpc": "2.0", "id": "s4", "method": "storage/list", "params": {} }
```

### storage/clear

```json
{ "jsonrpc": "2.0", "id": "s5", "method": "storage/clear", "params": {} }
```

**Storage limits:** 100 keys per user-plugin, 1 MB per value.

## Data Types

### Reading Status

```
"reading" | "completed" | "on_hold" | "dropped" | "plan_to_read"
```

### Series Status

```
"ongoing" | "ended" | "hiatus" | "abandoned" | "unknown"
```

### Reading Direction

```
"ltr" | "rtl" | "ttb"
```

### External Link Types

```
"provider" | "official" | "social" | "purchase" | "read" | "other"
```

### Dismiss Reasons

```
"not_interested" | "already_read" | "already_owned"
```

## Error Codes

### Standard JSON-RPC Errors

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Not a valid JSON-RPC request |
| -32601 | Method not found | Method doesn't exist |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Internal plugin error |

### Plugin-Specific Errors

| Code | Message | Description |
|------|---------|-------------|
| -32001 | Rate limited | API rate limit exceeded |
| -32002 | Not found | Resource not found |
| -32003 | Auth failed | Authentication failed |
| -32004 | API error | External API error |
| -32005 | Config error | Plugin configuration error |

## Lifecycle Diagram

```
Codex                                              Plugin Process
  │                                                      │
  │─── spawn(command, args, env) ───────────────────────▶│
  │                                                      │
  │◀─────────────────── process starts ──────────────────│
  │                                                      │
  │─── {"method":"initialize"} ─────────────────────────▶│
  │◀─── {"result": manifest} ────────────────────────────│
  │                                                      │
  │─── {"method":"ping"} ───────────────────────────────▶│
  │◀─── {"result": "pong"} ──────────────────────────────│
  │                                                      │
  │─── {"method":"metadata/series/search",...} ─────────▶│
  │◀─── {"result": {results: [...]}} ───────────────────│
  │                                                      │
  │◀─── {"method":"storage/set",...} ────────────────────│  (plugin → host)
  │─── {"result": {success: true}} ─────────────────────▶│
  │                                                      │
  │     ... more requests ...                            │
  │                                                      │
  │─── {"method":"shutdown"} ───────────────────────────▶│
  │◀─── {"result": null} ────────────────────────────────│
  │                                                      │
  │                                          process exits
```

## Best Practices

1. **Never write to stdout except for JSON-RPC responses** (and storage requests)
2. **Use stderr for all logging**
3. **Handle unknown methods gracefully** — return METHOD_NOT_FOUND error
4. **Include request ID in responses** — even for errors
5. **Exit cleanly on shutdown** — clean up resources, then exit
6. **Handle malformed requests** — don't crash on bad input
7. **Ignore unknown fields** — ensures forward compatibility with protocol additions
