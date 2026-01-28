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
  "method": "metadata/search",
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
    "results": [...],
    "page": 1,
    "hasNextPage": true
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

## Methods

### initialize

Called when Codex first connects to the plugin. Returns the plugin manifest and optionally receives credentials and configuration.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "credentials": {
      "api_key": "your-api-key"
    },
    "config": {
      "base_url": "https://api.example.com"
    }
  }
}
```

The `params` object is optional and depends on the plugin's **credential delivery** setting:

| Delivery Method | Value | Behavior |
|-----------------|-------|----------|
| Environment Variables | `env` | Credentials passed as env vars (e.g., `API_KEY`). No `credentials` in params. |
| Initialize Message | `init_message` | Credentials passed in `params.credentials`. |
| Both | `both` | Credentials passed both as env vars and in `params.credentials`. |

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
      "seriesMetadataProvider": true
    }
  }
}
```

### ping

Health check method. Used by Codex to verify the plugin is responsive.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "ping"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": "pong"
}
```

### shutdown

Called when Codex is shutting down or disabling the plugin. Plugins should clean up resources and exit.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "shutdown"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": null
}
```

### metadata/search

Search for metadata by query string.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "metadata/search",
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
        "summary": "A pirate adventure...",
        "year": 1997,
        "coverUrl": "https://example.com/cover.jpg",
        "status": "ongoing",
        "score": 95,
        "providerData": {
          "url": "https://example.com/series/12345"
        }
      }
    ],
    "totalResults": 1,
    "page": 1,
    "hasNextPage": false
  }
}
```

### metadata/get

Get full metadata for an external ID.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "metadata/get",
  "params": {
    "externalId": "12345"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "externalId": "12345",
    "titles": [
      { "value": "One Piece", "language": "en", "primary": true },
      { "value": "ワンピース", "language": "ja" }
    ],
    "summary": "A long, epic pirate adventure...",
    "status": "ongoing",
    "year": 1997,
    "coverUrl": "https://example.com/cover.jpg",
    "genres": ["Action", "Adventure", "Comedy"],
    "tags": ["Pirates", "Superpowers", "Long Running"],
    "authors": [
      { "name": "Eiichiro Oda", "role": "author" }
    ],
    "publisher": "Shueisha",
    "rating": 9.5,
    "ratingCount": 100000,
    "externalLinks": [
      { "name": "MangaBaka", "url": "https://mangabaka.org/series/12345" }
    ]
  }
}
```

### metadata/match

Find best match for existing content (auto-matching).

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "metadata/match",
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
      "year": 1997,
      "score": 95
    },
    "confidence": 98,
    "alternatives": [
      {
        "externalId": "67890",
        "title": "One Piece: Strong World",
        "score": 60
      }
    ]
  }
}
```

## Data Types

### SearchResult

```typescript
interface SearchResult {
  externalId: string;      // Provider's ID for this item
  title: string;           // Primary display title
  alternateTitles?: Title[];
  year?: number;
  coverUrl?: string;
  summary?: string;
  status?: "ongoing" | "completed" | "hiatus" | "cancelled" | "unknown";
  score?: number;          // Relevance score (0-100)
  providerData?: object;   // Passed to metadata/get
}
```

### SeriesMetadata

```typescript
interface SeriesMetadata {
  externalId: string;
  titles: Title[];
  summary?: string;
  status?: "ongoing" | "completed" | "hiatus" | "cancelled" | "unknown";
  year?: number;
  yearEnd?: number;
  coverUrl?: string;
  bannerUrl?: string;
  genres?: string[];
  tags?: string[];
  contentRating?: string;
  authors?: Person[];
  artists?: Person[];
  publisher?: string;
  originalLanguage?: string;
  country?: string;
  rating?: number;         // 0-10 scale
  ratingCount?: number;
  externalLinks?: ExternalLink[];
  providerData?: object;
}

interface Title {
  value: string;
  language?: string;       // ISO 639-1 code
  primary?: boolean;
}

interface Person {
  name: string;
  role?: string;
}

interface ExternalLink {
  name: string;
  url: string;
}
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

## Lifecycle

```
Codex                                              Plugin Process
  │                                                      │
  │─── spawn(command, args, env) ───────────────────────▶│
  │                                                      │
  │◀─────────────────── process starts ─────────────────│
  │                                                      │
  │─── {"method":"initialize"} ─────────────────────────▶│
  │◀─── {"result": manifest} ───────────────────────────│
  │                                                      │
  │─── {"method":"ping"} ───────────────────────────────▶│
  │◀─── {"result": "pong"} ─────────────────────────────│
  │                                                      │
  │─── {"method":"metadata/search",...} ────────────────▶│
  │◀─── {"result": [...]} ──────────────────────────────│
  │                                                      │
  │     ... more requests ...                            │
  │                                                      │
  │─── {"method":"shutdown"} ────────────────────────────▶│
  │◀─── {"result": null} ───────────────────────────────│
  │                                                      │
  │                                          process exits
```

## Best Practices

1. **Never write to stdout except for JSON-RPC responses**
2. **Use stderr for all logging**
3. **Handle unknown methods gracefully** - return METHOD_NOT_FOUND error
4. **Include request ID in responses** - even for errors
5. **Exit cleanly on shutdown** - clean up resources, then exit
6. **Handle malformed requests** - don't crash on bad input
