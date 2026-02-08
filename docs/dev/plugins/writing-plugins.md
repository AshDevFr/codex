# Writing Plugins

This guide walks you through creating a Codex metadata plugin from scratch using TypeScript and the official SDK.

## Prerequisites

- Node.js 18 or later
- npm or pnpm
- Basic TypeScript knowledge

## Quick Start

### 1. Create a New Project

```bash
mkdir codex-plugin-metadata-myplugin
cd codex-plugin-metadata-myplugin
npm init -y
```

### 2. Install Dependencies

```bash
npm install @ashdev/codex-plugin-sdk
npm install -D typescript @types/node esbuild
```

### 3. Configure TypeScript

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "outDir": "./dist",
    "rootDir": "./src",
    "declaration": true,
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["src/**/*"]
}
```

Update `package.json`:

```json
{
  "name": "@ashdev/codex-plugin-metadata-myplugin",
  "type": "module",
  "main": "dist/index.js",
  "scripts": {
    "build": "esbuild src/index.ts --bundle --platform=node --target=node18 --format=esm --outfile=dist/index.js --sourcemap",
    "start": "node dist/index.js",
    "typecheck": "tsc --noEmit"
  }
}
```

### 4. Write Your Plugin

Create `src/index.ts`:

```typescript
import {
  createMetadataPlugin,
  type MetadataProvider,
  type MetadataSearchParams,
  type MetadataSearchResponse,
  type MetadataGetParams,
  type PluginSeriesMetadata,
  type PluginManifest,
  type MetadataContentType,
} from "@ashdev/codex-plugin-sdk";

// Define your plugin manifest
const manifest = {
  name: "metadata-myplugin",
  displayName: "My Metadata Plugin",
  version: "1.0.0",
  description: "A custom metadata provider",
  author: "Your Name",
  protocolVersion: "1.0",
  capabilities: {
    metadataProvider: ["series"] as MetadataContentType[],
  },
  // Optional: credentials your plugin needs
  requiredCredentials: [
    {
      key: "api_key",
      label: "API Key",
      description: "Your API key for the metadata service",
      required: true,
      sensitive: true,
      type: "password",
    },
  ],
} as const satisfies PluginManifest & { capabilities: { metadataProvider: MetadataContentType[] } };

// Implement the MetadataProvider interface
const provider: MetadataProvider = {
  async search(params: MetadataSearchParams): Promise<MetadataSearchResponse> {
    // Implement your search logic
    const results = await fetchResults(params.query);

    return {
      results: results.map(r => ({
        externalId: r.id,
        title: r.title,
        alternateTitles: [],
        year: r.year,
        coverUrl: r.cover,
        relevanceScore: 0.9,
        preview: {
          status: r.status,
          genres: r.genres.slice(0, 3),
          description: r.summary?.slice(0, 200),
        },
      })),
    };
  },

  async get(params: MetadataGetParams): Promise<PluginSeriesMetadata> {
    // Implement your get logic
    const series = await fetchSeries(params.externalId);

    return {
      externalId: series.id,
      externalUrl: `https://example.com/series/${series.id}`,
      title: series.title,
      alternateTitles: [
        { title: series.nativeTitle, language: "ja", titleType: "native" },
      ],
      summary: series.description,
      status: series.status,
      year: series.year,
      genres: series.genres,
      tags: series.tags,
      authors: series.authors,
      artists: series.artists,
      externalLinks: [],
    };
  },
};

// Start the plugin
createMetadataPlugin({ manifest, provider });
```

### 5. Build and Test

```bash
npm run build

# Test manually
echo '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | node dist/index.js
```

## Plugin Manifest

The manifest describes your plugin's capabilities and requirements:

```typescript
interface PluginManifest {
  // Required
  name: string;           // Unique identifier (lowercase, alphanumeric, hyphens)
  displayName: string;    // Human-readable name
  version: string;        // Semver version
  description: string;    // Short description
  author: string;         // Author name
  protocolVersion: "1.0"; // Protocol version

  // Capabilities
  capabilities: {
    metadataProvider?: MetadataContentType[];  // Content types: ["series"] or ["series", "book"]
    syncProvider?: boolean;                     // Can sync reading progress (future)
    userRecommendationProvider?: boolean;       // Can provide recommendations (future)
  };

  // Optional
  homepage?: string;           // Documentation URL
  icon?: string;               // Icon URL
  requiredCredentials?: CredentialField[]; // API keys, etc.
}
```

## MetadataProvider Interface

Plugins must implement the `MetadataProvider` interface:

```typescript
interface MetadataProvider {
  search(params: MetadataSearchParams): Promise<MetadataSearchResponse>;
  get(params: MetadataGetParams): Promise<PluginSeriesMetadata>;
  match?(params: MetadataMatchParams): Promise<MetadataMatchResponse>;
}
```

The SDK automatically routes scoped method calls to your provider:
- `metadata/series/search` → `provider.search()`
- `metadata/series/get` → `provider.get()`
- `metadata/series/match` → `provider.match()`

### search

Search for metadata by query string:

```typescript
async search(params: MetadataSearchParams): Promise<MetadataSearchResponse> {
  // params.query - Search query string
  // params.limit - Maximum results to return
  // params.cursor - Pagination cursor from previous response

  return {
    results: [
      {
        externalId: "123",
        title: "Series Title",
        alternateTitles: ["Alt Title"],
        year: 2024,
        coverUrl: "https://example.com/cover.jpg",
        relevanceScore: 0.95, // 0.0-1.0
        preview: {
          status: "ongoing",
          genres: ["Action", "Adventure"],
          rating: 8.5,
          description: "Brief description...",
        },
      },
    ],
    nextCursor: "page2", // Optional: for pagination
  };
}
```

### get

Get full metadata for an external ID:

```typescript
async get(params: MetadataGetParams): Promise<PluginSeriesMetadata> {
  // params.externalId - ID from search results

  return {
    externalId: "123",
    externalUrl: "https://example.com/series/123",
    title: "Series Title",
    alternateTitles: [
      { title: "日本語タイトル", language: "ja", titleType: "native" },
      { title: "Romanized Title", language: "ja-Latn", titleType: "romaji" },
    ],
    summary: "Full description...",
    status: "ongoing",
    year: 2024,
    genres: ["Action", "Adventure"],
    tags: ["Fantasy", "Magic"],
    authors: ["Author Name"],
    artists: ["Artist Name"],
    publisher: "Publisher Name",
    rating: { score: 85, voteCount: 1000, source: "example" },
    externalLinks: [
      { url: "https://example.com/123", label: "Example", linkType: "provider" },
    ],
  };
}
```

### match (Optional)

Find best match for existing content (used for auto-matching):

```typescript
async match(params: MetadataMatchParams): Promise<MetadataMatchResponse> {
  // params.title - Title to match
  // params.year - Year hint
  // params.author - Author hint

  return {
    match: bestResult,      // Best match or null
    confidence: 0.85,       // 0.0-1.0 confidence score
    alternatives: [...],    // Other possible matches if confidence is low
  };
}
```

## Error Handling

Use SDK error classes for proper error reporting:

```typescript
import {
  RateLimitError,
  NotFoundError,
  AuthError,
  ApiError,
  ConfigError,
} from "@ashdev/codex-plugin-sdk";

// Rate limited by API
if (response.status === 429) {
  const retryAfter = response.headers.get("Retry-After") || "60";
  throw new RateLimitError(parseInt(retryAfter, 10));
}

// Resource not found
if (response.status === 404) {
  throw new NotFoundError("Series not found");
}

// Authentication failed
if (response.status === 401) {
  throw new AuthError("Invalid API key");
}

// Generic API error
if (!response.ok) {
  throw new ApiError(`API error: ${response.status}`, response.status);
}

// Configuration error
if (!apiKey) {
  throw new ConfigError("api_key credential is required");
}
```

## Logging

Always log to stderr (stdout is reserved for JSON-RPC):

```typescript
import { createLogger } from "@ashdev/codex-plugin-sdk";

const logger = createLogger({ name: "metadata-myplugin", level: "info" });

logger.debug("Processing request", { params });
logger.info("Search completed", { resultCount: 10 });
logger.warn("Rate limit approaching");
logger.error("Request failed", error);

// NEVER use console.log() - it goes to stdout and breaks the protocol!
// Instead use:
console.error("Debug message"); // This is safe
```

## Credential Delivery

Codex supports three methods for delivering credentials to plugins:

| Method | Value | Description |
|--------|-------|-------------|
| Environment Variables | `env` | Credentials passed as uppercase env vars (default) |
| Initialize Message | `init_message` | Credentials passed in the `initialize` JSON-RPC request |
| Both | `both` | Credentials passed both ways |

### Using onInitialize Callback (Recommended)

Credentials are passed in the `initialize` request params:

```typescript
import { createMetadataPlugin, ConfigError, type InitializeParams } from "@ashdev/codex-plugin-sdk";

let apiKey: string | undefined;

createMetadataPlugin({
  manifest,
  provider,
  onInitialize(params: InitializeParams) {
    apiKey = params.credentials?.api_key;
    if (!apiKey) {
      throw new ConfigError("api_key credential is required");
    }
  },
});
```

### Using Environment Variables

Credentials are passed as environment variables (credential key in uppercase):

```typescript
// Credential key "api_key" becomes environment variable "API_KEY"
const apiKey = process.env.API_KEY;

if (!apiKey) {
  throw new ConfigError("API_KEY environment variable is required");
}
```

## Testing Your Plugin

### Manual Testing

```bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | node dist/index.js

# Search (note the scoped method name)
echo '{"jsonrpc":"2.0","id":2,"method":"metadata/series/search","params":{"query":"test"}}' | node dist/index.js

# Ping
echo '{"jsonrpc":"2.0","id":3,"method":"ping"}' | node dist/index.js
```

### Unit Tests

```typescript
import { describe, it, expect } from "vitest";
import { mapSearchResult } from "./mappers";

describe("mappers", () => {
  it("should map API response to SearchResult", () => {
    const apiResponse = { id: "123", name: "Test" };
    const result = mapSearchResult(apiResponse);

    expect(result.externalId).toBe("123");
    expect(result.title).toBe("Test");
  });
});
```

## Deploying Your Plugin

### Local Installation

1. Build your plugin: `npm run build`
2. In Codex admin UI, add a new plugin:
   - Command: `node`
   - Args: `/path/to/plugin/dist/index.js`
   - Configure credentials

### Docker

If running Codex in Docker, mount the plugins directory:

```yaml
volumes:
  - ./my-plugin:/opt/codex/plugins/my-plugin:ro
```

Then configure:
- Command: `node`
- Args: `/opt/codex/plugins/my-plugin/dist/index.js`

## Best Practices

1. **Handle Rate Limits**: Respect API rate limits, throw `RateLimitError` with retry time
2. **Cache Responses**: Consider caching API responses to reduce load
3. **Normalize Data**: Map external data to standard Codex formats
4. **Graceful Degradation**: Return partial data rather than failing completely
5. **Log Appropriately**: Use debug level for request details, info for summary
6. **Test Thoroughly**: Write unit tests for mappers, integration tests for API client

## Example Plugins

- **Echo Plugin**: Simple test plugin - `plugins/metadata-echo/`
- **MangaBaka Plugin**: Full metadata provider - `plugins/metadata-mangabaka/`

## Next Steps

- [Plugin Protocol](./protocol.md) - Detailed protocol specification
- [Plugin SDK](./sdk.md) - Full SDK API documentation
