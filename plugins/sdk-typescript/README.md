# @ashdev/codex-plugin-sdk

Official SDK for building Codex plugins. Provides type-safe interfaces, utilities, and a server framework for communicating with Codex via JSON-RPC over stdio.

## Installation

```bash
npm install @ashdev/codex-plugin-sdk
```

## Quick Start

```typescript
import {
  createMetadataPlugin,
  type MetadataContentType,
  type MetadataProvider,
  type PluginManifest,
} from "@ashdev/codex-plugin-sdk";

// Define your plugin manifest
const manifest = {
  name: "metadata-my-plugin",
  displayName: "My Plugin",
  version: "1.0.0",
  description: "A custom metadata provider",
  author: "Your Name",
  protocolVersion: "1.0",
  capabilities: {
    metadataProvider: ["series"] as MetadataContentType[],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};

// Implement the MetadataProvider interface
const provider: MetadataProvider = {
  async search(params) {
    // Search your data source
    return {
      results: [
        {
          externalId: "123",
          title: "Example Series",
          alternateTitles: [],
          relevanceScore: 0.95,
          preview: {
            status: "ongoing",
            genres: ["Action", "Adventure"],
          },
        },
      ],
    };
  },

  async get(params) {
    // Fetch full metadata
    return {
      externalId: params.externalId,
      externalUrl: `https://example.com/series/${params.externalId}`,
      title: "Example Series",
      alternateTitles: [],
      summary: "An exciting series...",
      status: "ongoing",
      year: 2024,
      genres: ["Action", "Adventure"],
      tags: [],
      authors: ["Author Name"],
      artists: [],
      externalLinks: [],
    };
  },

  // Optional: implement match for auto-matching during library scans
  async match(params) {
    const result = await this.search({
      query: params.title,
    });

    return {
      match: result.results[0] ?? null,
      confidence: result.results[0] ? 0.8 : 0,
    };
  },
};

// Start the plugin
createMetadataPlugin({ manifest, provider });
```

## Features

- **Type-safe**: Full TypeScript support with interface contracts
- **Protocol compliance**: Types match the Codex JSON-RPC protocol exactly
- **Simple API**: Implement the `MetadataProvider` interface, SDK handles the rest
- **Error handling**: Built-in error classes that map to JSON-RPC errors
- **Logging**: Safe logging to stderr (stdout is reserved for protocol)

## Concepts

### Capabilities

Plugins declare capabilities in their manifest. Each capability has a corresponding interface:

| Capability | Interface | Description |
|------------|-----------|-------------|
| `metadataProvider` | `MetadataProvider` | Search and fetch metadata for content |

The `metadataProvider` capability is an array of content types your plugin supports:

```typescript
capabilities: {
  metadataProvider: ["series"] as MetadataContentType[],
}
```

Supported content types: `"series"` (future: `"book"`)

### Protocol Types

The SDK provides TypeScript types that exactly match the Codex JSON-RPC protocol:

- `MetadataSearchParams` / `MetadataSearchResponse` - Search parameters and results
- `MetadataGetParams` / `PluginSeriesMetadata` - Get full metadata
- `MetadataMatchParams` / `MetadataMatchResponse` - Auto-match content
- `SearchResult` - Individual search result with `relevanceScore`

### Relevance Score

Search results must include a `relevanceScore` between 0.0 and 1.0:

- `1.0` = Perfect match
- `0.7-0.9` = Good match
- `0.5-0.7` = Partial match
- `< 0.5` = Weak match

## API Reference

### `createMetadataPlugin(options)`

Creates and starts a metadata provider plugin.

```typescript
interface MetadataPluginOptions {
  manifest: PluginManifest & { capabilities: { metadataProvider: MetadataContentType[] } };
  provider: MetadataProvider;
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  logLevel?: "debug" | "info" | "warn" | "error";
}
```

### `MetadataProvider`

Interface for metadata provider plugins:

```typescript
interface MetadataProvider {
  search(params: MetadataSearchParams): Promise<MetadataSearchResponse>;
  get(params: MetadataGetParams): Promise<PluginSeriesMetadata>;
  match?(params: MetadataMatchParams): Promise<MetadataMatchResponse>;
}
```

### Error Handling

Use built-in error classes for proper error responses:

```typescript
import { NotFoundError, RateLimitError, AuthError, ApiError } from "@ashdev/codex-plugin-sdk";

// In your provider:
async get(params) {
  const data = await fetchFromApi(params.externalId);

  if (!data) {
    throw new NotFoundError(`Series ${params.externalId} not found`);
  }

  return data;
}
```

### Logging

Plugins should log to stderr (stdout is reserved for JSON-RPC):

```typescript
import { createLogger } from "@ashdev/codex-plugin-sdk";

const logger = createLogger({ name: "my-plugin", level: "debug" });
logger.info("Plugin started");
logger.debug("Processing request", { query: "..." });
logger.error("Failed to fetch", error);
```

## Credentials

Plugins can receive credentials (API keys, tokens) via the `onInitialize` callback:

```typescript
let apiKey: string | undefined;

createMetadataPlugin({
  manifest,
  provider,
  onInitialize(params) {
    apiKey = params.credentials?.api_key;
  },
});
```

Credentials are configured by admins in Codex and delivered securely to the plugin.

## Protocol

Plugins communicate with Codex via JSON-RPC 2.0 over stdio:

- **stdin**: Receives JSON-RPC requests (one per line)
- **stdout**: Sends JSON-RPC responses (one per line)
- **stderr**: Logging output (visible in Codex logs)

## Building Your Plugin

1. Create a new npm package
2. Install the SDK: `npm install @ashdev/codex-plugin-sdk`
3. Implement your provider
4. Bundle with esbuild (include shebang for npx support):

```json
{
  "name": "@your-org/plugin-metadata-example",
  "version": "1.0.0",
  "main": "dist/index.js",
  "bin": "dist/index.js",
  "type": "module",
  "files": ["dist", "README.md"],
  "engines": {
    "node": ">=22.0.0"
  },
  "scripts": {
    "build": "esbuild src/index.ts --bundle --platform=node --target=node22 --format=esm --outfile=dist/index.js --sourcemap --banner:js='#!/usr/bin/env node'",
    "prepublishOnly": "npm run lint && npm run build"
  }
}
```

**Key fields for npx support:**
- `bin`: Points to the entry file, tells npx what to execute
- `--banner:js='#!/usr/bin/env node'`: Adds shebang so the file is directly executable
- `files`: Only publish dist and README to npm

## Example Plugins

- [`@ashdev/codex-plugin-metadata-echo`](https://github.com/AshDevFr/codex/tree/main/plugins/metadata-echo) - Test metadata plugin that echoes back queries
- [`@ashdev/codex-plugin-metadata-mangabaka`](https://github.com/AshDevFr/codex/tree/main/plugins/metadata-mangabaka) - MangaBaka metadata provider

## License

MIT
