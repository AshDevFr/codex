# Writing Plugins

This guide walks you through building your own Codex plugin — from a simple metadata provider to sync and recommendation plugins that integrate with external services.

## Prerequisites

- **Node.js 22+** — plugins run as child processes launched by Codex
- **TypeScript 5.7+** — recommended for type safety; the SDK provides full type definitions
- **npm** or a compatible package manager

## Plugin Architecture Overview

Codex plugins are standalone processes that communicate with the Codex server over **stdin/stdout** using the [JSON-RPC 2.0](https://www.jsonrpc.org/specification) protocol. The SDK handles all protocol details — you implement provider interfaces and the SDK takes care of message routing, error formatting, and lifecycle management.

```
┌──────────────┐   stdin/stdout   ┌──────────────┐
│    Codex     │ ◄── JSON-RPC ──► │    Plugin    │
│    Server    │                  │   (Node.js)  │
└──────────────┘                  └──────────────┘
```

### Plugin Types

| Type | Capability | Description |
|------|-----------|-------------|
| **Metadata** | `metadataProvider: ["series"]` or `["book"]` | Fetch series/book metadata from external sources |
| **Sync** | `userReadSync: true` | Bidirectional reading progress sync with external trackers |
| **Recommendation** | `userRecommendationProvider: true` | Generate personalized series recommendations |

### Lifecycle

1. **Spawn** — Codex launches the plugin process
2. **Initialize** — Codex sends config, credentials, and a storage handle
3. **Requests** — Codex sends capability-specific requests (search, sync, etc.)
4. **Ping** — periodic health checks
5. **Shutdown** — graceful termination

## Build Your First Plugin: A Metadata Provider

Let's build a simple metadata plugin that searches a fictional API for series information.

### 1. Project Setup

Create a new directory and initialize the project:

```bash
mkdir codex-plugin-metadata-example
cd codex-plugin-metadata-example
npm init -y
```

Install the SDK and development tools:

```bash
npm install @ashdev/codex-plugin-sdk
npm install -D typescript esbuild @types/node vitest @biomejs/biome
```

### 2. Configure TypeScript

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "lib": ["ES2022"],
    "outDir": "./dist",
    "rootDir": "./src",
    "declaration": true,
    "sourceMap": true,
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "**/*.test.ts"]
}
```

Update `package.json` with build scripts and ES module settings:

```json
{
  "name": "@yourname/codex-plugin-metadata-example",
  "version": "1.0.0",
  "type": "module",
  "main": "dist/index.js",
  "bin": "dist/index.js",
  "files": ["dist"],
  "engines": { "node": ">=22.0.0" },
  "scripts": {
    "build": "esbuild src/index.ts --bundle --platform=node --target=node22 --format=esm --outfile=dist/index.js --sourcemap --banner:js='#!/usr/bin/env node'",
    "dev": "npm run build -- --watch",
    "test": "vitest run",
    "start": "node dist/index.js"
  }
}
```

Key points:
- **`"type": "module"`** — plugins use ES modules
- **`"bin"`** — makes the plugin executable via `npx`
- **esbuild** bundles everything into a single file with a Node.js shebang

### 3. Define the Manifest

The manifest tells Codex what your plugin can do. Create `src/manifest.ts`:

```typescript
import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

export const manifest = {
  name: "metadata-example",
  displayName: "Example Metadata Plugin",
  version: packageJson.version,
  description: "Fetches series metadata from Example API",
  author: "Your Name",
  homepage: "https://github.com/your/repo",
  protocolVersion: "1.0",

  capabilities: {
    metadataProvider: ["series"],   // "series", "book", or both
  },

  // Admin-configurable settings (Settings > Plugins > Configuration)
  configSchema: {
    description: "Plugin settings",
    fields: [
      {
        key: "maxResults",
        label: "Maximum Results",
        description: "Max results per search (1-20)",
        type: "number" as const,
        required: false,
        default: 5,
      },
    ],
  },
} as const satisfies PluginManifest;
```

#### Manifest Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Lowercase, alphanumeric with hyphens. Must be unique. |
| `displayName` | Yes | User-facing name shown in the UI |
| `version` | Yes | Semver string |
| `description` | Yes | Short description |
| `protocolVersion` | Yes | Always `"1.0"` for current protocol |
| `capabilities` | Yes | What the plugin provides (see Plugin Types above) |
| `configSchema` | No | Admin-configurable settings |
| `userConfigSchema` | No | Per-user settings |
| `requiredCredentials` | No | API keys or tokens (encrypted at rest) |
| `oauth` | No | OAuth 2.0 configuration for external services |
| `adminSetupInstructions` | No | Shown to admins during plugin configuration |
| `userSetupInstructions` | No | Shown to users when connecting |

### 4. Implement the Provider

Create `src/index.ts`:

```typescript
import {
  createMetadataPlugin,
  createLogger,
  type InitializeParams,
  type MetadataProvider,
  type MetadataSearchParams,
  type MetadataSearchResponse,
  type MetadataGetParams,
  type PluginSeriesMetadata,
  type MetadataMatchParams,
  type MetadataMatchResponse,
  NotFoundError,
  RateLimitError,
} from "@ashdev/codex-plugin-sdk";
import { manifest } from "./manifest.js";

// Logger writes to stderr (stdout is reserved for JSON-RPC)
const logger = createLogger({ name: "example", level: "debug" });

// Plugin state (populated during initialization)
let maxResults = 5;

// Implement the MetadataProvider interface
const provider: MetadataProvider = {
  async search(params: MetadataSearchParams): Promise<MetadataSearchResponse> {
    logger.info(`Searching for: ${params.query}`);

    // Call your external API here
    const results = await fetchFromApi(params.query);

    return {
      results: results.slice(0, maxResults).map((item, i) => ({
        externalId: item.id,
        title: item.title,
        alternateTitles: item.altTitles || [],
        year: item.year,
        relevanceScore: Math.max(0.1, 1.0 - i * 0.1),
        preview: {
          status: item.status,
          genres: item.genres,
          description: item.description,
        },
      })),
    };
  },

  async get(params: MetadataGetParams): Promise<PluginSeriesMetadata> {
    logger.info(`Getting metadata for: ${params.externalId}`);

    const item = await fetchById(params.externalId);
    if (!item) {
      throw new NotFoundError(`Series not found: ${params.externalId}`);
    }

    return {
      externalId: item.id,
      externalUrl: item.url,
      title: item.title,
      summary: item.description,
      status: item.status,
      year: item.year,
      genres: item.genres,
      tags: item.tags,
      authors: item.authors,
      coverUrl: item.coverUrl,
      rating: item.rating
        ? { score: item.rating, voteCount: item.votes, source: "example" }
        : undefined,
    };
  },

  // Optional: auto-match by title (called during library scans)
  async match(params: MetadataMatchParams): Promise<MetadataMatchResponse> {
    logger.info(`Matching: ${params.title}`);

    const results = await fetchFromApi(params.title);
    const best = results[0];

    if (!best) {
      return { match: null, confidence: 0, alternatives: [] };
    }

    return {
      match: {
        externalId: best.id,
        title: best.title,
        alternateTitles: [],
        year: best.year,
        relevanceScore: 0.9,
      },
      confidence: 0.85,
      alternatives: results.slice(1, 4).map((r) => ({
        externalId: r.id,
        title: r.title,
        alternateTitles: [],
        relevanceScore: 0.6,
      })),
    };
  },
};

// Start the plugin
createMetadataPlugin({
  manifest,
  provider,
  logLevel: "debug",
  onInitialize(params: InitializeParams) {
    // Read admin configuration
    const configured = params.adminConfig?.maxResults as number | undefined;
    if (configured !== undefined) {
      maxResults = Math.min(Math.max(1, configured), 20);
    }
    logger.info(`Plugin initialized (maxResults: ${maxResults})`);
  },
});

logger.info("Example plugin started");
```

#### Error Handling

The SDK provides error classes that automatically convert to proper JSON-RPC error responses:

```typescript
import {
  NotFoundError,    // Resource not found (code: -32001)
  RateLimitError,   // Rate limited, includes retryAfterSeconds (code: -32003)
  AuthError,        // Authentication failed (code: -32002)
  ApiError,         // External API error (code: -32004)
  ConfigError,      // Configuration error (code: -32005)
} from "@ashdev/codex-plugin-sdk";

// In your provider methods:
throw new NotFoundError("Series not found");
throw new RateLimitError(60, "Rate limited by external API");
throw new AuthError("Invalid API key");
```

### 5. Build and Test Locally

Build the plugin:

```bash
npm run build
```

Test it by running directly — the plugin reads JSON-RPC from stdin:

```bash
echo '{"jsonrpc":"2.0","method":"initialize","params":{"adminConfig":{},"userConfig":{},"credentials":{}},"id":1}' | node dist/index.js
```

You should see a JSON-RPC response with the manifest on stdout, and log messages on stderr.

### 6. Install in Codex

Three ways to install your plugin:

#### Option A: Local Path (Development)

In Codex **Settings > Plugins > Add Plugin**:
- **Command**: `node`
- **Arguments**: `/absolute/path/to/dist/index.js`

#### Option B: npx (No Install Needed)

Publish to npm, then configure:
- **Command**: `npx`
- **Arguments**: `-y @yourname/codex-plugin-metadata-example@1.0.0`

#### Option C: Global Install

```bash
npm install -g @yourname/codex-plugin-metadata-example
```

Then configure:
- **Command**: `codex-plugin-metadata-example` (or whatever your `bin` name is)

After adding the plugin, go to **Settings > Plugins**, review the requested permissions, and enable it.

## Logging

Plugins must **only write to stderr** for logging — stdout is reserved for JSON-RPC communication. The SDK logger handles this automatically:

```typescript
import { createLogger } from "@ashdev/codex-plugin-sdk";

const logger = createLogger({ name: "my-plugin", level: "debug" });

logger.debug("Detailed debug info", { query: "naruto" });
logger.info("Operation completed");
logger.warn("Something unexpected", { code: 429 });
logger.error("Operation failed", { error: err.message });
```

Log levels: `debug`, `info`, `warn`, `error`.

## Plugin Storage

Plugins can persist data across restarts using the storage API. Storage is scoped per user-plugin connection — each user's data is isolated.

```typescript
import { type PluginStorage } from "@ashdev/codex-plugin-sdk";

// Storage is provided during initialization
let storage: PluginStorage;

onInitialize(params) {
  storage = params.storage;
}

// Basic operations
await storage.set("cache-key", { data: "value" });
await storage.set("temp-key", { data: "value" }, "2025-12-31T00:00:00Z"); // With TTL
const result = await storage.get("cache-key");   // { data, expiresAt? }
await storage.delete("cache-key");

// List and clear
const keys = await storage.list();               // { keys: [{ key, expiresAt? }] }
await storage.clear();                           // { deletedCount }
```

### Storage Limits

- **100 keys** per user-plugin connection
- **1 MB** per value
- Limits enforced on writes only

## Configuration Patterns

Plugins receive configuration during initialization from three sources:

### Admin Config (`configSchema`)

Set by the Codex administrator in **Settings > Plugins > Configuration**. Use this for settings that apply to all users (e.g., result limits, API endpoints).

```typescript
configSchema: {
  fields: [
    {
      key: "maxResults",
      label: "Maximum Results",
      type: "number" as const,
      required: false,
      default: 10,
    },
  ],
},
```

### User Config (`userConfigSchema`)

Per-user settings configured in **Settings > Integrations > Plugin Settings**. Use this for personal preferences.

```typescript
userConfigSchema: {
  fields: [
    {
      key: "progressUnit",
      label: "Progress Unit",
      type: "string" as const,
      required: false,
      default: "volumes",
    },
  ],
},
```

### The `_codex` Namespace

For sync plugins, the server stores generic sync settings under the `_codex` key in the user config. These are **server-interpreted** — the plugin never reads them. They control which entries the server sends:

| Key | Default | Description |
|-----|---------|-------------|
| `includeCompleted` | `true` | Include fully-read series |
| `includeInProgress` | `true` | Include partially-read series |
| `countPartialProgress` | `false` | Count partially-read books |
| `syncRatings` | `true` | Include scores and notes |

### Credentials (`requiredCredentials`)

API keys and tokens, encrypted at rest by Codex:

```typescript
requiredCredentials: [
  {
    key: "api_key",
    label: "API Key",
    type: "password" as const,
    required: true,
    sensitive: true,
  },
],
```

### Reading Config in `onInitialize`

```typescript
onInitialize(params: InitializeParams) {
  const adminMax = params.adminConfig?.maxResults as number | undefined;
  const userUnit = params.userConfig?.progressUnit as string | undefined;
  const apiKey = params.credentials?.api_key as string | undefined;

  // storage is also available here
  storage = params.storage;
}
```

## Building a Sync Plugin

Sync plugins enable bidirectional reading progress synchronization with external tracking services (e.g., AniList, MyAnimeList).

### Manifest

```typescript
import type { PluginManifest } from "@ashdev/codex-plugin-sdk";

export const manifest = {
  name: "sync-example",
  displayName: "Example Sync",
  version: "1.0.0",
  protocolVersion: "1.0",
  description: "Sync reading progress with Example Tracker",
  author: "Your Name",

  capabilities: {
    userReadSync: true,
    externalIdSource: "api:example",  // Prefix for external ID matching
  },

  // OAuth for automatic authentication
  oauth: {
    authorizationUrl: "https://example.com/oauth/authorize",
    tokenUrl: "https://example.com/oauth/token",
    scopes: ["read", "write"],
    pkce: true,  // Recommended when supported
  },

  requiredCredentials: [
    {
      key: "access_token",
      label: "Access Token",
      type: "password" as const,
      required: true,
      sensitive: true,
    },
  ],

  userConfigSchema: {
    description: "Sync settings",
    fields: [
      {
        key: "progressUnit",
        label: "Progress Unit",
        type: "string" as const,
        required: false,
        default: "volumes",
      },
    ],
  },
} as const satisfies PluginManifest;
```

### SyncProvider Interface

```typescript
import {
  createSyncPlugin,
  createLogger,
  type SyncProvider,
  type ExternalUserInfo,
  type SyncPushRequest,
  type SyncPushResponse,
  type SyncPullRequest,
  type SyncPullResponse,
  AuthError,
} from "@ashdev/codex-plugin-sdk";
import { manifest } from "./manifest.js";

const logger = createLogger({ name: "sync-example" });
let accessToken: string;

const provider: SyncProvider = {
  // Return the authenticated user's profile
  async getUserInfo(): Promise<ExternalUserInfo> {
    const user = await fetchUser(accessToken);
    return {
      externalId: user.id.toString(),
      username: user.name,
      avatarUrl: user.avatar,
      profileUrl: user.url,
    };
  },

  // Push local reading progress to the external service
  async pushProgress(params: SyncPushRequest): Promise<SyncPushResponse> {
    const successes: string[] = [];
    const failures: Array<{ externalId: string; error: string }> = [];

    for (const entry of params.entries) {
      try {
        await updateExternalProgress(accessToken, {
          externalId: entry.externalId,
          status: entry.status,         // reading, completed, on_hold, dropped, plan_to_read
          progress: entry.progress,     // { chapters?, volumes?, pages? }
          rating: entry.rating,         // 0-100
          startedAt: entry.startedAt,
          completedAt: entry.completedAt,
        });
        successes.push(entry.externalId);
      } catch (err) {
        failures.push({ externalId: entry.externalId, error: String(err) });
      }
    }

    return { successes, failures };
  },

  // Pull reading progress from the external service
  async pullProgress(params: SyncPullRequest): Promise<SyncPullResponse> {
    const list = await fetchReadingList(accessToken, {
      page: params.page || 1,
      updatedSince: params.updatedSince,
    });

    return {
      entries: list.items.map((item) => ({
        externalId: item.id.toString(),
        title: item.title,
        status: mapStatus(item.status),
        progress: {
          chapters: item.chaptersRead,
          volumes: item.volumesRead,
        },
        rating: item.score,
        startedAt: item.startDate,
        lastReadAt: item.updatedAt,
        completedAt: item.completionDate,
        latestUpdatedAt: item.updatedAt,  // Used for staleness detection
      })),
      hasMore: list.hasNextPage,
      nextPage: list.hasNextPage ? (params.page || 1) + 1 : undefined,
    };
  },

  // Optional: return sync status summary
  async status() {
    return { lastSyncAt: new Date().toISOString() };
  },
};

createSyncPlugin({
  manifest,
  provider,
  onInitialize(params) {
    accessToken = params.credentials?.access_token as string;
    if (!accessToken) throw new AuthError("No access token provided");
  },
});
```

### External ID Matching

Sync plugins declare an `externalIdSource` in their manifest (e.g., `"api:example"`). Codex uses this to match series in your library with entries on the external service via the `series_external_ids` table. When pushing progress, Codex only sends entries that have a matching external ID.

Define the source string as a constant in your plugin using the `api:<service>` convention:

```typescript
const EXTERNAL_ID_SOURCE_ANILIST = "api:anilist" as const;
```

### OAuth Configuration

When `oauth` is defined in the manifest, Codex handles the full OAuth flow:

1. User clicks "Connect" in **Settings > Integrations**
2. Codex opens the authorization URL with CSRF state token and PKCE challenge
3. User authorizes on the external service
4. External service redirects to Codex's callback endpoint
5. Codex exchanges the code for tokens and stores them encrypted
6. Tokens are passed to the plugin as `credentials.access_token`

The plugin never handles OAuth flows directly — it just receives the token.

## Building a Recommendation Plugin

Recommendation plugins analyze the user's library and suggest new series.

### Manifest

```typescript
import type { PluginManifest } from "@ashdev/codex-plugin-sdk";

export const manifest = {
  name: "recommendations-example",
  displayName: "Example Recommendations",
  version: "1.0.0",
  protocolVersion: "1.0",
  description: "Personalized recommendations from Example Service",
  author: "Your Name",

  capabilities: {
    userRecommendationProvider: true,
  },

  configSchema: {
    description: "Recommendation settings",
    fields: [
      {
        key: "maxRecommendations",
        label: "Maximum Recommendations",
        type: "number" as const,
        default: 20,
      },
      {
        key: "maxSeeds",
        label: "Seed Titles",
        description: "Number of top-rated library titles to use as input",
        type: "number" as const,
        default: 10,
      },
    ],
  },

  // OAuth if the service requires authentication
  oauth: {
    authorizationUrl: "https://example.com/oauth/authorize",
    tokenUrl: "https://example.com/oauth/token",
  },

  requiredCredentials: [
    { key: "access_token", label: "Access Token", type: "password" as const, required: true, sensitive: true },
  ],
} as const satisfies PluginManifest;
```

### RecommendationProvider Interface

```typescript
import {
  createRecommendationPlugin,
  createLogger,
  type RecommendationProvider,
  type RecommendationRequest,
  type RecommendationResponse,
  type PluginStorage,
} from "@ashdev/codex-plugin-sdk";
import { manifest } from "./manifest.js";

const logger = createLogger({ name: "recs-example" });
let storage: PluginStorage;
let maxRecommendations = 20;
let maxSeeds = 10;

const provider: RecommendationProvider = {
  // Generate recommendations based on user's library
  async get(params: RecommendationRequest): Promise<RecommendationResponse> {
    // params.library contains the user's series with ratings, genres, tags
    const seeds = params.library
      .sort((a, b) => (b.userRating || 0) - (a.userRating || 0))
      .slice(0, maxSeeds);

    logger.info(`Generating recommendations from ${seeds.length} seeds`);

    // Fetch recommendations from external API based on seeds
    const recs = await fetchRecommendations(seeds);

    // Exclude series already in the library
    const libraryIds = new Set(
      params.library.flatMap((e) => e.externalIds?.map((id) => id.externalId) || [])
    );
    // Also exclude explicitly dismissed series
    const excludeIds = new Set(params.excludeIds || []);

    const filtered = recs
      .filter((r) => !libraryIds.has(r.externalId) && !excludeIds.has(r.externalId))
      .slice(0, maxRecommendations);

    return {
      recommendations: filtered.map((r) => ({
        externalId: r.externalId,
        url: r.url,
        title: r.title,
        coverUrl: r.coverUrl,
        description: r.description,
        genres: r.genres,
        rating: r.rating,
        why: `Recommended because you liked "${r.basedOn}"`,
      })),
    };
  },

  // Optional: dismiss a recommendation
  async dismiss(params) {
    // Store dismissed IDs to exclude from future results
    const dismissed = ((await storage.get("dismissed"))?.data as string[]) || [];
    dismissed.push(params.externalId);
    await storage.set("dismissed", dismissed);
    return { success: true };
  },

  // Optional: clear cached data
  async clear() {
    await storage.clear();
    return { success: true };
  },
};

createRecommendationPlugin({
  manifest,
  provider,
  onInitialize(params) {
    storage = params.storage;
    maxRecommendations = (params.adminConfig?.maxRecommendations as number) || 20;
    maxSeeds = (params.adminConfig?.maxSeeds as number) || 10;
  },
});
```

### Scoring Tips

When scoring recommendations, consider:

- **Community rating** from the external API (e.g., AniList `averageScore / 10`)
- **Relevance** to seed titles (genre overlap, tag similarity)
- **Duplicate boost** — if the same title appears from multiple seeds, boost its score (e.g., +0.05 per duplicate)
- **Score clamping** — keep final scores in the 0.0-1.0 range

## Testing Your Plugin

### Unit Tests with Vitest

```typescript
// src/manifest.test.ts
import { describe, it, expect } from "vitest";
import { manifest } from "./manifest.js";

describe("manifest", () => {
  it("has required fields", () => {
    expect(manifest.name).toBe("metadata-example");
    expect(manifest.protocolVersion).toBe("1.0");
    expect(manifest.capabilities.metadataProvider).toContain("series");
  });
});
```

```typescript
// src/index.test.ts
import { describe, it, expect, vi } from "vitest";

describe("search", () => {
  it("returns results for a query", async () => {
    // Test your provider logic directly
    const results = generateResults("naruto");
    expect(results).toHaveLength(5);
    expect(results[0].title).toContain("naruto");
  });
});
```

### Running Tests

```bash
# Run all tests
npx vitest run

# Watch mode during development
npx vitest

# With coverage
npx vitest run --coverage
```

### Manual Testing

You can test the JSON-RPC protocol directly:

```bash
# Build first
npm run build

# Send initialize + search requests
echo '{"jsonrpc":"2.0","method":"initialize","params":{"adminConfig":{},"userConfig":{},"credentials":{}},"id":1}
{"jsonrpc":"2.0","method":"metadata/series/search","params":{"query":"test"},"id":2}' | node dist/index.js
```

## Common Patterns

### Rate Limiting

When calling external APIs, handle rate limits gracefully:

```typescript
import { RateLimitError, ApiError } from "@ashdev/codex-plugin-sdk";

async function callApi(url: string) {
  const response = await fetch(url);

  if (response.status === 429) {
    const retryAfter = parseInt(response.headers.get("Retry-After") || "60", 10);
    throw new RateLimitError(retryAfter, "API rate limit exceeded");
  }

  if (!response.ok) {
    throw new ApiError(`API error: ${response.status}`, response.status);
  }

  return response.json();
}
```

### Pagination

For pull operations that may return large datasets:

```typescript
async pullProgress(params: SyncPullRequest): Promise<SyncPullResponse> {
  const page = params.page || 1;
  const data = await fetchPage(page);

  return {
    entries: data.items,
    hasMore: data.hasNextPage,
    nextPage: data.hasNextPage ? page + 1 : undefined,
  };
}
```

Codex will keep calling `pullProgress` with incrementing pages until `hasMore` is `false`.

### Caching with Storage TTL

```typescript
const CACHE_KEY = "api-cache";
const CACHE_TTL_HOURS = 24;

async function getCachedOrFetch(key: string): Promise<unknown> {
  const cached = await storage.get(key);
  if (cached?.data) return cached.data;

  const fresh = await fetchFromApi(key);
  const expiresAt = new Date(Date.now() + CACHE_TTL_HOURS * 3600_000).toISOString();
  await storage.set(key, fresh, expiresAt);
  return fresh;
}
```

## Reference Implementations

The Codex repository includes three reference plugins:

| Plugin | Location | Type | Description |
|--------|----------|------|-------------|
| **Echo** | `plugins/metadata-echo/` | Metadata | Minimal test plugin; echoes back queries as results. Great starting point. |
| **AniList Sync** | `plugins/sync-anilist/` | Sync | Full bidirectional sync with AniList. Shows OAuth, GraphQL, conflict resolution, staleness detection. |
| **AniList Recommendations** | `plugins/recommendations-anilist/` | Recommendation | Personalized recommendations from AniList. Shows scoring, deduplication, external ID resolution. |

## Security Notes

- **stdout** is reserved for JSON-RPC — never `console.log()` in production code; use the SDK logger (writes to stderr)
- **Credentials** (API keys, tokens) are encrypted at rest by Codex; treat them as sensitive
- **Storage** is scoped per user — one user cannot access another's plugin data
- Plugins run in a **sandboxed child process** with restricted environment variables
- All JSON-RPC requests have a **30-second timeout**

## Protocol Versioning

Plugins declare `protocolVersion: "1.0"` in their manifest. The versioning contract:

- **Additive changes** (new optional fields, new methods) do NOT bump the version
- **Breaking changes** (removed fields, changed semantics) bump the major version
- Plugins should **ignore unknown fields** — this ensures forward compatibility
- Plugins built for `1.x` continue working as long as Codex supports major version `1`

## Next Steps

- [Plugin Protocol](./protocol.md) — Detailed protocol specification
- [Plugin SDK](./sdk.md) — Full SDK API documentation
