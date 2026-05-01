# Plugin SDK

The `@ashdev/codex-plugin-sdk` package provides TypeScript types, utilities, and a server framework for building Codex plugins.

## Installation

```bash
npm install @ashdev/codex-plugin-sdk
```

Requires Node.js 22+.

## Quick Example

```typescript
import {
  createMetadataPlugin,
  type MetadataProvider,
  type PluginManifest,
  type MetadataContentType,
} from "@ashdev/codex-plugin-sdk";

const manifest = {
  name: "metadata-my-plugin",
  displayName: "My Metadata Plugin",
  version: "1.0.0",
  description: "A metadata provider",
  author: "Your Name",
  protocolVersion: "1.0",
  capabilities: {
    metadataProvider: ["series"] as MetadataContentType[],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};

const provider: MetadataProvider = {
  async search(params) {
    return { results: [] };
  },
  async get(params) {
    return {
      externalId: params.externalId,
      externalUrl: `https://example.com/${params.externalId}`,
    };
  },
};

createMetadataPlugin({ manifest, provider });
```

## Factory Functions

### createMetadataPlugin

Creates a metadata plugin server for series and/or book metadata.

```typescript
function createMetadataPlugin(options: MetadataPluginOptions): void;

interface MetadataPluginOptions {
  manifest: PluginManifest & { capabilities: { metadataProvider: MetadataContentType[] } };
  provider?: MetadataProvider;       // Series metadata provider
  bookProvider?: BookMetadataProvider; // Book metadata provider
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  logLevel?: "debug" | "info" | "warn" | "error";
}
```

Routes methods automatically:
- `metadata/series/search` → `provider.search()`
- `metadata/series/get` → `provider.get()`
- `metadata/series/match` → `provider.match()`
- `metadata/book/search` → `bookProvider.search()`
- `metadata/book/get` → `bookProvider.get()`
- `metadata/book/match` → `bookProvider.match()`

### createSyncPlugin

Creates a sync plugin server for reading progress synchronization.

```typescript
function createSyncPlugin(options: SyncPluginOptions): void;

interface SyncPluginOptions {
  manifest: PluginManifest & { capabilities: { userReadSync: true } };
  provider: SyncProvider;
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  logLevel?: "debug" | "info" | "warn" | "error";
}
```

Routes methods:
- `sync/getUserInfo` → `provider.getUserInfo()`
- `sync/pushProgress` → `provider.pushProgress()`
- `sync/pullProgress` → `provider.pullProgress()`
- `sync/status` → `provider.status()`

### createRecommendationPlugin

Creates a recommendation plugin server.

```typescript
function createRecommendationPlugin(options: RecommendationPluginOptions): void;

interface RecommendationPluginOptions {
  manifest: PluginManifest & { capabilities: { userRecommendationProvider: true } };
  provider: RecommendationProvider;
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  logLevel?: "debug" | "info" | "warn" | "error";
}
```

Routes methods:
- `recommendations/get` → `provider.get()`
- `recommendations/updateProfile` → `provider.updateProfile()`
- `recommendations/clear` → `provider.clear()`
- `recommendations/dismiss` → `provider.dismiss()`

### InitializeParams

Passed to the `onInitialize` callback:

```typescript
interface InitializeParams {
  adminConfig?: Record<string, unknown>;   // From manifest.configSchema
  userConfig?: Record<string, unknown>;    // From manifest.userConfigSchema
  credentials?: Record<string, string>;    // From manifest.requiredCredentials
  storage: PluginStorage;                  // Scoped storage client
}
```

## Provider Interfaces

### MetadataProvider

```typescript
interface MetadataProvider {
  search(params: MetadataSearchParams): Promise<MetadataSearchResponse>;
  get(params: MetadataGetParams): Promise<PluginSeriesMetadata>;
  match?(params: MetadataMatchParams): Promise<MetadataMatchResponse>;
}
```

### BookMetadataProvider

```typescript
interface BookMetadataProvider {
  search(params: BookSearchParams): Promise<MetadataSearchResponse>;
  get(params: MetadataGetParams): Promise<PluginBookMetadata>;
  match?(params: BookMatchParams): Promise<MetadataMatchResponse>;
}
```

### SyncProvider

```typescript
interface SyncProvider {
  getUserInfo(): Promise<ExternalUserInfo>;
  pushProgress(params: SyncPushRequest): Promise<SyncPushResponse>;
  pullProgress(params: SyncPullRequest): Promise<SyncPullResponse>;
  status?(): Promise<SyncStatusResponse>;
}
```

### RecommendationProvider

```typescript
interface RecommendationProvider {
  get(params: RecommendationRequest): Promise<RecommendationResponse>;
  updateProfile?(params: ProfileUpdateRequest): Promise<ProfileUpdateResponse>;
  clear?(): Promise<RecommendationClearResponse>;
  dismiss?(params: RecommendationDismissRequest): Promise<RecommendationDismissResponse>;
}
```

## Logging

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

Writes to **stderr** only (stdout is reserved for JSON-RPC).

```typescript
import { createLogger } from "@ashdev/codex-plugin-sdk";

const logger = createLogger({ name: "my-plugin", level: "debug" });
logger.info("Plugin started");
logger.debug("Processing request", { params });
logger.error("Request failed", error);
```

## Error Classes

All error classes extend `PluginError` and automatically convert to JSON-RPC error responses.

### RateLimitError

```typescript
import { RateLimitError } from "@ashdev/codex-plugin-sdk";
throw new RateLimitError(60);                           // Retry after 60 seconds
throw new RateLimitError(60, "API rate limit exceeded"); // With message
// Code: -32001
```

### NotFoundError

```typescript
import { NotFoundError } from "@ashdev/codex-plugin-sdk";
throw new NotFoundError("Series not found");
// Code: -32002
```

### AuthError

```typescript
import { AuthError } from "@ashdev/codex-plugin-sdk";
throw new AuthError("Invalid API key");
// Code: -32003
```

### ApiError

```typescript
import { ApiError } from "@ashdev/codex-plugin-sdk";
throw new ApiError("External API returned 500", 500);
// Code: -32004
```

### ConfigError

```typescript
import { ConfigError } from "@ashdev/codex-plugin-sdk";
throw new ConfigError("api_key credential is required");
// Code: -32005
```

## Storage

The `PluginStorage` class provides a key-value store scoped per user-plugin connection.

```typescript
class PluginStorage {
  async get(key: string): Promise<StorageGetResponse>;
  async set(key: string, data: unknown, expiresAt?: string): Promise<StorageSetResponse>;
  async delete(key: string): Promise<StorageDeleteResponse>;
  async list(): Promise<StorageListResponse>;
  async clear(): Promise<StorageClearResponse>;
}
```

**Response types:**

```typescript
interface StorageGetResponse { data: unknown | null; expiresAt?: string; }
interface StorageSetResponse { success: boolean; }
interface StorageDeleteResponse { deleted: boolean; }
interface StorageListResponse { keys: StorageKeyEntry[]; }
interface StorageClearResponse { deletedCount: number; }
interface StorageKeyEntry { key: string; expiresAt?: string; updatedAt: string; }
```

**Limits:** 100 keys per user-plugin, 1 MB per value.

## Types

### Manifest

```typescript
interface PluginManifest {
  name: string;                          // Unique ID (lowercase, alphanumeric, hyphens)
  displayName: string;                   // User-facing name
  version: string;                       // Semver
  description: string;
  author: string;
  homepage?: string;
  icon?: string;
  protocolVersion: "1.0";
  capabilities: PluginCapabilities;
  requiredCredentials?: CredentialField[];
  configSchema?: ConfigSchema;           // Admin settings
  userConfigSchema?: ConfigSchema;       // Per-user settings
  oauth?: OAuthConfig;                   // OAuth 2.0 configuration
  userDescription?: string;
  adminSetupInstructions?: string;
  userSetupInstructions?: string;
}

interface PluginCapabilities {
  metadataProvider?: MetadataContentType[];  // "series" and/or "book"
  userReadSync?: boolean;
  externalIdSource?: string;                 // e.g., "api:anilist"
  userRecommendationProvider?: boolean;
}

type MetadataContentType = "series" | "book";
```

### OAuth

```typescript
interface OAuthConfig {
  authorizationUrl: string;
  tokenUrl: string;
  scopes?: string[];
  pkce?: boolean;        // Default: true
  userInfoUrl?: string;
  clientId?: string;     // Default client ID
}
```

### Config & Credentials

```typescript
interface ConfigSchema {
  description: string;
  fields: ConfigField[];
}

interface ConfigField {
  key: string;
  label: string;
  description?: string;
  type: "string" | "number" | "boolean";
  required?: boolean;
  default?: unknown;
  example?: unknown;
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

### Metadata Search

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
  relevanceScore: number;       // 0.0-1.0
  preview?: SearchResultPreview;
}

interface SearchResultPreview {
  status?: SeriesStatus;
  genres?: string[];
  rating?: number;
  description?: string;
  bookCount?: number;
  authors?: string[];
  /**
   * Content format discriminator for visually disambiguating results.
   * Free-form string. Recommended values (lowercase snake_case):
   *   manga, manhwa, manhua, novel, light_novel, comic, webtoon,
   *   one_shot, doujin, artbook.
   * The UI maps known values to colored badges and falls back to a
   * neutral badge for anything else.
   */
  format?: string;
}
```

### Series Metadata

```typescript
interface PluginSeriesMetadata {
  externalId: string;
  externalUrl?: string;
  title?: string;
  alternateTitles?: AlternateTitle[];
  summary?: string;
  status?: SeriesStatus;
  year?: number;
  totalBookCount?: number;
  language?: string;
  ageRating?: number;
  readingDirection?: ReadingDirection;
  genres?: string[];
  tags?: string[];
  authors?: string[];
  artists?: string[];
  publisher?: string;
  coverUrl?: string;
  bannerUrl?: string;
  rating?: ExternalRating;
  externalRatings?: ExternalRating[];
  externalLinks?: ExternalLink[];
}

type SeriesStatus = "ongoing" | "ended" | "hiatus" | "abandoned" | "unknown";
type ReadingDirection = "ltr" | "rtl" | "ttb";
```

### Book Metadata

```typescript
interface PluginBookMetadata {
  externalId: string;
  externalUrl?: string;
  title?: string;
  subtitle?: string;
  alternateTitles?: AlternateTitle[];
  summary?: string;
  bookType?: string;
  volume?: number;
  pageCount?: number;
  releaseDate?: string;
  year?: number;
  isbn?: string;
  isbns?: string[];
  edition?: string;
  originalTitle?: string;
  originalYear?: number;
  translator?: string;
  language?: string;
  seriesPosition?: number;
  seriesTotal?: number;
  genres?: string[];
  tags?: string[];
  subjects?: string[];
  authors?: BookAuthor[];
  artists?: string[];
  publisher?: string;
  coverUrl?: string;
  covers?: BookCover[];
  rating?: ExternalRating;
  externalRatings?: ExternalRating[];
  awards?: BookAward[];
  externalLinks?: ExternalLink[];
}

interface BookSearchParams {
  isbn?: string;
  query?: string;
  author?: string;
  year?: number;
  limit?: number;
  cursor?: string;
}

interface BookAuthor {
  name: string;
  role?: BookAuthorRole;
  sortName?: string;
}

type BookAuthorRole = "author" | "coauthor" | "editor" | "translator" | "illustrator" | "contributor";
```

### Matching

```typescript
interface MetadataMatchParams {
  title: string;
  year?: number;
  author?: string;
}

interface BookMatchParams {
  title: string;
  authors?: string[];
  isbn?: string;
  year?: number;
  publisher?: string;
}

interface MetadataMatchResponse {
  match: SearchResult | null;
  confidence: number;          // 0.0-1.0
  alternatives?: SearchResult[];
}
```

### Sync Types

```typescript
type SyncReadingStatus = "reading" | "completed" | "on_hold" | "dropped" | "plan_to_read";

interface ExternalUserInfo {
  externalId: string;
  username: string;
  avatarUrl?: string;
  profileUrl?: string;
}

interface SyncProgress {
  chapters?: number;
  volumes?: number;
  pages?: number;
  totalChapters?: number;
  totalVolumes?: number;
}

interface SyncEntry {
  externalId: string;
  title?: string;
  status: SyncReadingStatus;
  progress?: SyncProgress;
  rating?: number;             // 0-100
  startedAt?: string;
  lastReadAt?: string;
  completedAt?: string;
  latestUpdatedAt?: string;    // For staleness detection
}

interface SyncPushRequest { entries: SyncEntry[]; }
interface SyncPushResponse {
  successes: string[];
  failures: Array<{ externalId: string; error: string }>;
}

interface SyncPullRequest {
  page?: number;
  updatedSince?: string;
  limit?: number;
  cursor?: string;
}
interface SyncPullResponse {
  entries: SyncEntry[];
  hasMore: boolean;
  nextPage?: number;
  nextCursor?: string;
}

interface SyncStatusResponse {
  lastSyncAt?: string;
  totalEntries?: number;
  syncedEntries?: number;
  conflicts?: number;
}
```

### Recommendation Types

```typescript
type DismissReason = "not_interested" | "already_read" | "already_owned";

interface UserLibraryEntry {
  seriesId: string;
  title: string;
  genres?: string[];
  tags?: string[];
  booksRead?: number;
  booksOwned?: number;
  userRating?: number;         // 0-100
  externalIds?: ExternalId[];
}

interface RecommendationRequest {
  library: UserLibraryEntry[];
  limit?: number;
  excludeIds?: string[];
}

interface Recommendation {
  externalId: string;
  url?: string;
  title: string;
  coverUrl?: string;
  description?: string;
  genres?: string[];
  rating?: number;             // 0.0-1.0
  why?: string;
  basedOn?: Array<{ title: string; externalId: string }>;
}

interface RecommendationResponse {
  recommendations: Recommendation[];
}

interface RecommendationDismissRequest {
  externalId: string;
  reason?: DismissReason;
}
```

### Supporting Types

```typescript
interface AlternateTitle {
  title: string;
  language?: string;
  titleType?: "english" | "native" | "romaji" | string;
}

interface ExternalRating {
  score: number;               // 0-100
  voteCount?: number;
  source: string;
}

interface ExternalLink {
  url: string;
  label: string;
  linkType?: ExternalLinkType;
}

type ExternalLinkType = "provider" | "official" | "social" | "purchase" | "read" | "other";

interface ExternalId {
  source: string;              // e.g., "api:anilist"
  externalId: string;
}

interface BookCover { url: string; width?: number; height?: number; size?: BookCoverSize; }
type BookCoverSize = "small" | "medium" | "large";

interface BookAward { name: string; year?: number; category?: string; won?: boolean; }
```

### External ID Source Convention

Plugins that match entries to external services should declare an `externalIdSource` in their capabilities using the `api:<service>` convention:

```typescript
capabilities: {
  externalIdSource: "api:anilist",  // or "api:myanimelist", "api:kitsu", etc.
}
```

Define the source string as a constant in your plugin (not in the SDK, since it's service-specific).

### JSON-RPC Types

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

// Standard JSON-RPC error codes
const JSON_RPC_ERROR_CODES = {
  PARSE_ERROR: -32700,
  INVALID_REQUEST: -32600,
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
  INTERNAL_ERROR: -32603,
};

// Plugin-specific error codes
const PLUGIN_ERROR_CODES = {
  RATE_LIMITED: -32001,
  NOT_FOUND: -32002,
  AUTH_FAILED: -32003,
  API_ERROR: -32004,
  CONFIG_ERROR: -32005,
};
```
