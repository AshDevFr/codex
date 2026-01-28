# Plugin SDK

The `@codex/plugin-sdk` package provides TypeScript types, utilities, and a server framework for building Codex plugins.

## Installation

```bash
npm install @codex/plugin-sdk
```

## Quick Example

```typescript
import {
  createSeriesMetadataPlugin,
  type SeriesMetadataProvider,
  type PluginManifest,
} from "@codex/plugin-sdk";

const manifest = {
  name: "metadata-my-plugin",
  displayName: "My Metadata Plugin",
  version: "1.0.0",
  description: "A metadata provider",
  author: "Your Name",
  protocolVersion: "1.0",
  capabilities: { seriesMetadataProvider: true },
} as const satisfies PluginManifest & { capabilities: { seriesMetadataProvider: true } };

const provider: SeriesMetadataProvider = {
  async search(params) {
    return { results: [] };
  },
  async get(params) {
    return {
      externalId: params.externalId,
      externalUrl: `https://example.com/${params.externalId}`,
      alternateTitles: [],
      genres: [],
      tags: [],
      authors: [],
      artists: [],
      externalLinks: [],
    };
  },
};

createSeriesMetadataPlugin({ manifest, provider });
```

## API Reference

### createSeriesMetadataPlugin

Creates and starts a series metadata plugin server that handles JSON-RPC communication.

```typescript
function createSeriesMetadataPlugin(options: SeriesMetadataPluginOptions): void;

interface SeriesMetadataPluginOptions {
  manifest: PluginManifest & { capabilities: { seriesMetadataProvider: true } };
  provider: SeriesMetadataProvider;
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  logLevel?: "debug" | "info" | "warn" | "error";
}
```

### SeriesMetadataProvider

Interface for implementing series metadata providers:

```typescript
interface SeriesMetadataProvider {
  search(params: MetadataSearchParams): Promise<MetadataSearchResponse>;
  get(params: MetadataGetParams): Promise<PluginSeriesMetadata>;
  match?(params: MetadataMatchParams): Promise<MetadataMatchResponse>;
}
```

### createLogger

Creates a logger that writes to stderr (safe for plugins).

```typescript
function createLogger(options: LoggerOptions): Logger;

interface LoggerOptions {
  name: string;
  level?: "debug" | "info" | "warn" | "error";
  timestamps?: boolean;
}

interface Logger {
  debug(message: string, data?: unknown): void;
  info(message: string, data?: unknown): void;
  warn(message: string, data?: unknown): void;
  error(message: string, data?: unknown): void;
}
```

**Example:**

```typescript
const logger = createLogger({ name: "metadata-my-plugin", level: "debug" });

logger.info("Plugin started");
logger.debug("Processing request", { params });
logger.error("Request failed", error);
```

## Error Classes

### RateLimitError

Thrown when rate limited by an external API.

```typescript
import { RateLimitError } from "@codex/plugin-sdk";

if (response.status === 429) {
  throw new RateLimitError(60); // Retry after 60 seconds
}
```

### NotFoundError

Thrown when a requested resource doesn't exist.

```typescript
import { NotFoundError } from "@codex/plugin-sdk";

if (response.status === 404) {
  throw new NotFoundError("Series not found");
}
```

### AuthError

Thrown when authentication fails.

```typescript
import { AuthError } from "@codex/plugin-sdk";

if (response.status === 401) {
  throw new AuthError("Invalid API key");
}
```

### ApiError

Thrown for generic API errors.

```typescript
import { ApiError } from "@codex/plugin-sdk";

if (!response.ok) {
  throw new ApiError(`API error: ${response.status}`, response.status);
}
```

### ConfigError

Thrown when the plugin is misconfigured.

```typescript
import { ConfigError } from "@codex/plugin-sdk";

if (!apiKey) {
  throw new ConfigError("api_key credential is required");
}
```

## Types

### PluginManifest

```typescript
interface PluginManifest {
  name: string;           // Unique identifier (e.g., "metadata-myplugin")
  displayName: string;
  version: string;
  description: string;
  author: string;
  homepage?: string;
  icon?: string;
  protocolVersion: "1.0";
  capabilities: PluginCapabilities;
  requiredCredentials?: CredentialField[];
}

interface PluginCapabilities {
  seriesMetadataProvider?: boolean;
  syncProvider?: boolean;
  recommendationProvider?: boolean;
}

interface CredentialField {
  key: string;
  label: string;
  description?: string;
  required: boolean;
  sensitive: boolean;
  type: "text" | "password" | "url";
  placeholder?: string;
}
```

### MetadataSearchParams / MetadataSearchResponse

```typescript
interface MetadataSearchParams {
  query: string;
  limit?: number;
  cursor?: string;
}

interface MetadataSearchResponse {
  results: SearchResult[];
  nextCursor?: string;
}

interface SearchResult {
  externalId: string;
  title: string;
  alternateTitles: string[];
  year?: number;
  coverUrl?: string;
  relevanceScore: number;  // 0.0-1.0
  preview?: SearchResultPreview;
}

interface SearchResultPreview {
  status?: SeriesStatus;
  genres?: string[];
  rating?: number;
  description?: string;
}
```

### MetadataGetParams / PluginSeriesMetadata

```typescript
interface MetadataGetParams {
  externalId: string;
}

interface PluginSeriesMetadata {
  externalId: string;
  externalUrl?: string;
  title?: string;
  alternateTitles: AlternateTitle[];
  summary?: string;
  status?: SeriesStatus;
  year?: number;
  totalBookCount?: number;
  language?: string;
  ageRating?: number;
  readingDirection?: ReadingDirection;
  genres: string[];
  tags: string[];
  authors: string[];
  artists: string[];
  publisher?: string;
  coverUrl?: string;
  bannerUrl?: string;
  rating?: ExternalRating;
  externalRatings?: ExternalRating[];
  externalLinks: ExternalLink[];
}

interface AlternateTitle {
  title: string;
  language?: string;
  titleType?: "english" | "native" | "romaji" | string;
}

type SeriesStatus = "ongoing" | "ended" | "cancelled" | "hiatus" | "unknown";
type ReadingDirection = "ltr" | "rtl" | "ttb" | "btt";
```

### MetadataMatchParams / MetadataMatchResponse

```typescript
interface MetadataMatchParams {
  title: string;
  year?: number;
  author?: string;
}

interface MetadataMatchResponse {
  match: SearchResult | null;
  confidence: number;  // 0.0-1.0
  alternatives?: SearchResult[];
}
```

### Supporting Types

```typescript
interface ExternalRating {
  score: number;        // 0-100
  voteCount?: number;
  source: string;
}

interface ExternalLink {
  url: string;
  label: string;
  linkType?: ExternalLinkType;
}

type ExternalLinkType =
  | "provider"
  | "official"
  | "social"
  | "purchase"
  | "info"
  | "other";
```

## JSON-RPC Types

```typescript
interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: string | number | null;
  method: string;
  params?: unknown;
}

interface JsonRpcSuccessResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  result: unknown;
}

interface JsonRpcErrorResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  error: JsonRpcError;
}

interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}
```

## Error Codes

```typescript
// Standard JSON-RPC errors
const JSON_RPC_ERROR_CODES = {
  PARSE_ERROR: -32700,
  INVALID_REQUEST: -32600,
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
  INTERNAL_ERROR: -32603,
};

// Plugin-specific errors
const PLUGIN_ERROR_CODES = {
  RATE_LIMITED: -32001,
  NOT_FOUND: -32002,
  AUTH_FAILED: -32003,
  API_ERROR: -32004,
  CONFIG_ERROR: -32005,
};
```

## Initialize Callback

Use `onInitialize` to receive credentials and configuration:

```typescript
createSeriesMetadataPlugin({
  manifest,
  provider,
  onInitialize(params) {
    // params.credentials - Credential values (e.g., { api_key: "..." })
    // params.config - Configuration values
    if (!params.credentials?.api_key) {
      throw new ConfigError("api_key credential is required");
    }
    apiKey = params.credentials.api_key;
  },
});
```
