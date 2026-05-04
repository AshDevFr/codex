/**
 * Plugin server - handles JSON-RPC communication over stdio
 *
 * Provides factory functions for creating different plugin types.
 * All plugin types share a common base server that handles:
 * - stdin readline parsing
 * - JSON-RPC error handling
 * - initialize/ping/shutdown lifecycle methods
 *
 * Each plugin type adds its own method routing on top.
 */

import { createInterface } from "node:readline";
import { PluginError } from "./errors.js";
import { HostRpcClient } from "./host-rpc.js";
import { createLogger, type Logger } from "./logger.js";
import { PluginStorage } from "./storage.js";
import type {
  BookMetadataProvider,
  MetadataContentType,
  MetadataProvider,
  RecommendationProvider,
  ReleaseSourceProvider,
  SyncProvider,
} from "./types/capabilities.js";
import type { PluginManifest, ReleaseSourceCapability } from "./types/manifest.js";
import type {
  BookMatchParams,
  BookSearchParams,
  MetadataGetParams,
  MetadataMatchParams,
  MetadataSearchParams,
} from "./types/protocol.js";
import type {
  ProfileUpdateRequest,
  RecommendationDismissRequest,
  RecommendationRequest,
} from "./types/recommendations.js";
import type { ReleasePollRequest } from "./types/releases.js";
import { JSON_RPC_ERROR_CODES, type JsonRpcRequest, type JsonRpcResponse } from "./types/rpc.js";
import type { SyncPullRequest, SyncPushRequest } from "./types/sync.js";

// =============================================================================
// Parameter Validation
// =============================================================================

interface ValidationError {
  field: string;
  message: string;
}

/**
 * Validate that the required string fields are present and non-empty
 */
function validateStringFields(params: unknown, fields: string[]): ValidationError | null {
  if (params === null || params === undefined) {
    return { field: "params", message: "params is required" };
  }
  if (typeof params !== "object") {
    return { field: "params", message: "params must be an object" };
  }

  const obj = params as Record<string, unknown>;
  for (const field of fields) {
    const value = obj[field];
    if (value === undefined || value === null) {
      return { field, message: `${field} is required` };
    }
    if (typeof value !== "string") {
      return { field, message: `${field} must be a string` };
    }
    if (value.trim() === "") {
      return { field, message: `${field} cannot be empty` };
    }
  }

  return null;
}

/**
 * Validate MetadataSearchParams
 */
function validateSearchParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["query"]);
}

/**
 * Validate MetadataGetParams
 */
function validateGetParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["externalId"]);
}

/**
 * Validate MetadataMatchParams
 */
function validateMatchParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["title"]);
}

/**
 * Validate BookSearchParams - requires either isbn or query
 */
function validateBookSearchParams(params: unknown): ValidationError | null {
  if (params === null || params === undefined) {
    return { field: "params", message: "params is required" };
  }
  if (typeof params !== "object") {
    return { field: "params", message: "params must be an object" };
  }

  const obj = params as Record<string, unknown>;
  const hasIsbn = obj.isbn !== undefined && obj.isbn !== null && obj.isbn !== "";
  const hasQuery = obj.query !== undefined && obj.query !== null && obj.query !== "";

  if (!hasIsbn && !hasQuery) {
    return { field: "isbn/query", message: "either isbn or query is required" };
  }

  return null;
}

/**
 * Validate BookMatchParams
 */
function validateBookMatchParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["title"]);
}

/**
 * Create an INVALID_PARAMS error response
 */
function invalidParamsError(id: string | number | null, error: ValidationError): JsonRpcResponse {
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code: JSON_RPC_ERROR_CODES.INVALID_PARAMS,
      message: `Invalid params: ${error.message}`,
      data: { field: error.field },
    },
  };
}

// =============================================================================
// Shared Types
// =============================================================================

/**
 * Initialize parameters received from Codex
 */
export interface InitializeParams {
  /** Admin-level plugin configuration (from plugin settings) */
  adminConfig?: Record<string, unknown>;
  /** Per-user plugin configuration (from user plugin settings) */
  userConfig?: Record<string, unknown>;
  /** Plugin credentials (API keys, tokens, etc.) */
  credentials?: Record<string, string>;
  /**
   * Per-user key-value storage client.
   *
   * Use this to persist data across plugin restarts (e.g., dismissed IDs,
   * cached profiles, user preferences). Storage is scoped per user-plugin
   * instance — the host resolves the user context automatically.
   */
  storage: PluginStorage;
  /**
   * Generic host reverse-RPC client.
   *
   * Use this to call host methods outside the storage namespace, notably
   * the `releases/*` methods (`releases/list_tracked`, `releases/record`,
   * `releases/source_state/get`, `releases/source_state/set`) for plugins
   * declaring the `releaseSource` capability.
   */
  hostRpc: HostRpcClient;
}

/**
 * A method router handles capability-specific JSON-RPC methods.
 * Returns a response for known methods, or null to indicate "not my method".
 */
type MethodRouter = (
  method: string,
  params: unknown,
  id: string | number | null,
) => Promise<JsonRpcResponse | null>;

// =============================================================================
// Shared Plugin Server
// =============================================================================

interface PluginServerOptions {
  manifest: PluginManifest;
  onInitialize?: ((params: InitializeParams) => void | Promise<void>) | undefined;
  logLevel?: "debug" | "info" | "warn" | "error" | undefined;
  label?: string | undefined;
  router: MethodRouter;
}

/**
 * Shared plugin server that handles JSON-RPC communication over stdio.
 *
 * Handles the common lifecycle methods (initialize, ping, shutdown) and
 * delegates capability-specific methods to the provided router.
 */
function createPluginServer(options: PluginServerOptions): void {
  const { manifest, onInitialize, logLevel = "info", label, router } = options;
  const logger = createLogger({ name: manifest.name, level: logLevel });
  const prefix = label ? `${label} plugin` : "plugin";
  const storage = new PluginStorage();
  const hostRpc = new HostRpcClient();

  logger.info(`Starting ${prefix}: ${manifest.displayName} v${manifest.version}`);

  const rl = createInterface({
    input: process.stdin,
    terminal: false,
  });

  rl.on("line", (line) => {
    void handleLine(line, manifest, onInitialize, router, logger, storage, hostRpc);
  });

  rl.on("close", () => {
    logger.info("stdin closed, shutting down");
    storage.cancelAll();
    hostRpc.cancelAll();
    process.exit(0);
  });

  process.on("uncaughtException", (error) => {
    logger.error("Uncaught exception", error);
    process.exit(1);
  });

  process.on("unhandledRejection", (reason) => {
    logger.error("Unhandled rejection", reason);
  });
}

/**
 * Detect whether a parsed JSON object is a JSON-RPC response (not a request).
 *
 * A response has `id` and either `result` or `error`, but no `method`.
 * A request always has `method`.
 */
function isJsonRpcResponse(obj: Record<string, unknown>): boolean {
  if (obj.method !== undefined) return false;
  if (obj.id === undefined || obj.id === null) return false;
  return "result" in obj || "error" in obj;
}

async function handleLine(
  line: string,
  manifest: PluginManifest,
  onInitialize: ((params: InitializeParams) => void | Promise<void>) | undefined,
  router: MethodRouter,
  logger: Logger,
  storage: PluginStorage,
  hostRpc: HostRpcClient,
): Promise<void> {
  const trimmed = line.trim();
  if (!trimmed) return;

  // Try to detect responses (storage or host-rpc) before full request handling.
  // Both come from the host on stdin — they have id + (result|error) but no
  // method field. The two clients use disjoint id ranges so each can claim
  // ownership without coordination; whichever owns the id resolves it.
  let parsed: Record<string, unknown> | undefined;
  try {
    parsed = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    // Will be handled as a parse error below
  }

  if (parsed && isJsonRpcResponse(parsed)) {
    logger.debug("Routing reverse-RPC response", { id: parsed.id });
    if (!hostRpc.handleResponse(trimmed)) {
      storage.handleResponse(trimmed);
    }
    return;
  }

  let id: string | number | null = null;

  try {
    const request = (parsed ?? JSON.parse(trimmed)) as JsonRpcRequest;
    id = request.id;

    logger.debug(`Received request: ${request.method}`, { id: request.id });

    const response = await handleRequest(
      request,
      manifest,
      onInitialize,
      router,
      logger,
      storage,
      hostRpc,
    );
    if (response !== null) {
      writeResponse(response);
    }
  } catch (error) {
    if (error instanceof SyntaxError) {
      writeResponse({
        jsonrpc: "2.0",
        id: null,
        error: {
          code: JSON_RPC_ERROR_CODES.PARSE_ERROR,
          message: "Parse error: invalid JSON",
        },
      });
    } else if (error instanceof PluginError) {
      writeResponse({
        jsonrpc: "2.0",
        id,
        error: error.toJsonRpcError(),
      });
    } else {
      const message = error instanceof Error ? error.message : "Unknown error";
      logger.error("Request failed", error);
      writeResponse({
        jsonrpc: "2.0",
        id,
        error: {
          code: JSON_RPC_ERROR_CODES.INTERNAL_ERROR,
          message,
        },
      });
    }
  }
}

async function handleRequest(
  request: JsonRpcRequest,
  manifest: PluginManifest,
  onInitialize: ((params: InitializeParams) => void | Promise<void>) | undefined,
  router: MethodRouter,
  logger: Logger,
  storage: PluginStorage,
  hostRpc: HostRpcClient,
): Promise<JsonRpcResponse | null> {
  const { method, params, id } = request;

  // Common lifecycle methods
  switch (method) {
    case "initialize": {
      const initParams = (params ?? {}) as InitializeParams;
      // Inject the reverse-RPC clients so plugins can persist data and
      // call host-side methods (e.g. releases/list_tracked).
      initParams.storage = storage;
      initParams.hostRpc = hostRpc;
      if (onInitialize) {
        await onInitialize(initParams);
      }
      return { jsonrpc: "2.0", id, result: manifest };
    }

    case "ping":
      return { jsonrpc: "2.0", id, result: "pong" };

    case "shutdown": {
      logger.info("Shutdown requested");
      storage.cancelAll();
      hostRpc.cancelAll();
      const response: JsonRpcResponse = { jsonrpc: "2.0", id, result: null };
      process.stdout.write(`${JSON.stringify(response)}\n`, () => {
        process.exit(0);
      });
      // Response already written above; return null so handleLine skips the write
      return null;
    }
  }

  // Delegate to capability-specific router
  const response = await router(method, params, id);
  if (response !== null) {
    return response;
  }

  // Unknown method
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code: JSON_RPC_ERROR_CODES.METHOD_NOT_FOUND,
      message: `Method not found: ${method}`,
    },
  };
}

function writeResponse(response: JsonRpcResponse): void {
  process.stdout.write(`${JSON.stringify(response)}\n`);
}

// =============================================================================
// Response Helpers
// =============================================================================

function methodNotFound(id: string | number | null, message: string): JsonRpcResponse {
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code: JSON_RPC_ERROR_CODES.METHOD_NOT_FOUND,
      message,
    },
  };
}

function success(id: string | number | null, result: unknown): JsonRpcResponse {
  return { jsonrpc: "2.0", id, result };
}

// =============================================================================
// Metadata Plugin
// =============================================================================

/**
 * Options for creating a metadata plugin
 */
export interface MetadataPluginOptions {
  /** Plugin manifest - must have capabilities.metadataProvider with content types */
  manifest: PluginManifest & {
    capabilities: { metadataProvider: MetadataContentType[] };
  };
  /** Series MetadataProvider implementation (required if "series" in metadataProvider) */
  provider?: MetadataProvider;
  /** Book MetadataProvider implementation (required if "book" in metadataProvider) */
  bookProvider?: BookMetadataProvider;
  /** Called when plugin receives initialize with credentials/config */
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  /** Log level (default: "info") */
  logLevel?: "debug" | "info" | "warn" | "error";
}

/**
 * Create and run a metadata provider plugin
 *
 * Creates a plugin server that handles JSON-RPC communication over stdio.
 * The TypeScript compiler will ensure you implement all required methods.
 *
 * @example
 * ```typescript
 * import { createMetadataPlugin, type MetadataProvider } from "@ashdev/codex-plugin-sdk";
 *
 * const provider: MetadataProvider = {
 *   async search(params) {
 *     return {
 *       results: [{
 *         externalId: "123",
 *         title: "Example",
 *         alternateTitles: [],
 *         relevanceScore: 0.95,
 *       }],
 *     };
 *   },
 *   async get(params) {
 *     return {
 *       externalId: params.externalId,
 *       externalUrl: "https://example.com/123",
 *       alternateTitles: [],
 *       genres: [],
 *       tags: [],
 *       authors: [],
 *       artists: [],
 *       externalLinks: [],
 *     };
 *   },
 * };
 *
 * createMetadataPlugin({
 *   manifest: {
 *     name: "my-plugin",
 *     displayName: "My Plugin",
 *     version: "1.0.0",
 *     description: "Example plugin",
 *     author: "Me",
 *     protocolVersion: "1.0",
 *     capabilities: { metadataProvider: ["series"] },
 *   },
 *   provider,
 * });
 * ```
 */
export function createMetadataPlugin(options: MetadataPluginOptions): void {
  const { manifest, provider, bookProvider, onInitialize, logLevel } = options;

  // Validate that required providers are present based on manifest
  const contentTypes = manifest.capabilities.metadataProvider;
  if (contentTypes.includes("series") && !provider) {
    throw new Error(
      "Series metadata provider is required when 'series' is in metadataProvider capabilities",
    );
  }
  if (contentTypes.includes("book") && !bookProvider) {
    throw new Error(
      "Book metadata provider is required when 'book' is in metadataProvider capabilities",
    );
  }

  const router: MethodRouter = async (method, params, id) => {
    switch (method) {
      // Series metadata methods
      case "metadata/series/search": {
        if (!provider) return methodNotFound(id, "This plugin does not support series metadata");
        const err = validateSearchParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await provider.search(params as MetadataSearchParams));
      }
      case "metadata/series/get": {
        if (!provider) return methodNotFound(id, "This plugin does not support series metadata");
        const err = validateGetParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await provider.get(params as MetadataGetParams));
      }
      case "metadata/series/match": {
        if (!provider) return methodNotFound(id, "This plugin does not support series metadata");
        if (!provider.match) return methodNotFound(id, "This plugin does not support series match");
        const err = validateMatchParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await provider.match(params as MetadataMatchParams));
      }

      // Book metadata methods
      case "metadata/book/search": {
        if (!bookProvider) return methodNotFound(id, "This plugin does not support book metadata");
        const err = validateBookSearchParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await bookProvider.search(params as BookSearchParams));
      }
      case "metadata/book/get": {
        if (!bookProvider) return methodNotFound(id, "This plugin does not support book metadata");
        const err = validateGetParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await bookProvider.get(params as MetadataGetParams));
      }
      case "metadata/book/match": {
        if (!bookProvider) return methodNotFound(id, "This plugin does not support book metadata");
        if (!bookProvider.match)
          return methodNotFound(id, "This plugin does not support book match");
        const err = validateBookMatchParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await bookProvider.match(params as BookMatchParams));
      }

      default:
        return null;
    }
  };

  createPluginServer({ manifest, onInitialize, logLevel, router });
}

// =============================================================================
// Sync Plugin
// =============================================================================

/**
 * Options for creating a sync provider plugin
 */
export interface SyncPluginOptions {
  /** Plugin manifest - must have capabilities.userReadSync: true */
  manifest: PluginManifest & {
    capabilities: { userReadSync: true };
  };
  /** SyncProvider implementation */
  provider: SyncProvider;
  /** Called when plugin receives initialize with credentials/config */
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  /** Log level (default: "info") */
  logLevel?: "debug" | "info" | "warn" | "error";
}

/**
 * Create and run a sync provider plugin
 *
 * Creates a plugin server that handles JSON-RPC communication over stdio
 * for sync operations (push/pull reading progress with external services).
 *
 * @example
 * ```typescript
 * import { createSyncPlugin, type SyncProvider } from "@ashdev/codex-plugin-sdk";
 *
 * const provider: SyncProvider = {
 *   async getUserInfo() {
 *     return { externalId: "123", username: "user" };
 *   },
 *   async pushProgress(params) {
 *     return { success: [], failed: [] };
 *   },
 *   async pullProgress(params) {
 *     return { entries: [], hasMore: false };
 *   },
 * };
 *
 * createSyncPlugin({
 *   manifest: {
 *     name: "my-sync-plugin",
 *     displayName: "My Sync Plugin",
 *     version: "1.0.0",
 *     description: "Syncs reading progress",
 *     author: "Me",
 *     protocolVersion: "1.0",
 *     capabilities: { userReadSync: true },
 *   },
 *   provider,
 * });
 * ```
 */
export function createSyncPlugin(options: SyncPluginOptions): void {
  const { manifest, provider, onInitialize, logLevel } = options;

  const router: MethodRouter = async (method, params, id) => {
    switch (method) {
      case "sync/getUserInfo":
        return success(id, await provider.getUserInfo());
      case "sync/pushProgress":
        return success(id, await provider.pushProgress(params as SyncPushRequest));
      case "sync/pullProgress":
        return success(id, await provider.pullProgress(params as SyncPullRequest));
      case "sync/status": {
        if (!provider.status) return methodNotFound(id, "This plugin does not support sync/status");
        return success(id, await provider.status());
      }
      default:
        return null;
    }
  };

  createPluginServer({ manifest, onInitialize, logLevel, label: "sync", router });
}

// =============================================================================
// Recommendation Plugin
// =============================================================================

/**
 * Options for creating a recommendation provider plugin
 */
export interface RecommendationPluginOptions {
  /** Plugin manifest - must have capabilities.userRecommendationProvider: true */
  manifest: PluginManifest & {
    capabilities: { userRecommendationProvider: true };
  };
  /** RecommendationProvider implementation */
  provider: RecommendationProvider;
  /** Called when plugin receives initialize with credentials/config */
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  /** Log level (default: "info") */
  logLevel?: "debug" | "info" | "warn" | "error";
}

/**
 * Create and run a recommendation provider plugin
 *
 * Creates a plugin server that handles JSON-RPC communication over stdio
 * for recommendation operations (get recommendations, update profile, dismiss).
 */
export function createRecommendationPlugin(options: RecommendationPluginOptions): void {
  const { manifest, provider, onInitialize, logLevel } = options;

  const router: MethodRouter = async (method, params, id) => {
    switch (method) {
      case "recommendations/get":
        return success(id, await provider.get(params as RecommendationRequest));
      case "recommendations/updateProfile": {
        if (!provider.updateProfile)
          return methodNotFound(id, "This plugin does not support recommendations/updateProfile");
        return success(id, await provider.updateProfile(params as ProfileUpdateRequest));
      }
      case "recommendations/clear": {
        if (!provider.clear)
          return methodNotFound(id, "This plugin does not support recommendations/clear");
        return success(id, await provider.clear());
      }
      case "recommendations/dismiss": {
        if (!provider.dismiss)
          return methodNotFound(id, "This plugin does not support recommendations/dismiss");
        const err = validateStringFields(params, ["externalId"]);
        if (err) return invalidParamsError(id, err);
        return success(id, await provider.dismiss(params as RecommendationDismissRequest));
      }
      default:
        return null;
    }
  };

  createPluginServer({ manifest, onInitialize, logLevel, label: "recommendation", router });
}

// =============================================================================
// Release Source Plugin
// =============================================================================

/**
 * Validate `releases/poll` parameters. Requires a non-empty `sourceId` string;
 * `etag` is optional.
 */
function validateReleasePollParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["sourceId"]);
}

/**
 * Options for creating a release-source plugin.
 */
export interface ReleaseSourcePluginOptions {
  /** Plugin manifest. Must declare `capabilities.releaseSource`. */
  manifest: PluginManifest & {
    capabilities: { releaseSource: ReleaseSourceCapability };
  };
  /** ReleaseSourceProvider implementation. */
  provider: ReleaseSourceProvider;
  /** Called when plugin receives initialize with credentials/config. */
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  /** Log level (default: "info"). */
  logLevel?: "debug" | "info" | "warn" | "error";
}

/**
 * Create and run a release-source plugin.
 *
 * The host calls `releases/poll` on a schedule (per `release_sources` row).
 * The plugin returns candidates either inline (in the poll response) or by
 * streaming `releases/record` reverse-RPC calls during the poll. Both styles
 * are supported by the host.
 *
 * Plugins typically:
 *   1. Fetch tracked series via `releases/list_tracked`.
 *   2. For each series, GET the upstream feed (with `If-None-Match` from the
 *      previous ETag).
 *   3. Parse + filter (language, group blocklist, etc.).
 *   4. Either return all candidates in the poll response or call
 *      `releases/record` for each.
 *   5. Persist the new ETag via `releases/source_state/set` (or include it on
 *      the poll response).
 *
 * @example
 * ```typescript
 * import { createReleaseSourcePlugin, type ReleaseSourceProvider } from "@ashdev/codex-plugin-sdk";
 *
 * const provider: ReleaseSourceProvider = {
 *   async poll({ sourceId, etag }) {
 *     // ...fetch + parse...
 *     return { candidates: [...], etag: "new-etag" };
 *   },
 * };
 *
 * createReleaseSourcePlugin({ manifest, provider });
 * ```
 */
export function createReleaseSourcePlugin(options: ReleaseSourcePluginOptions): void {
  const { manifest, provider, onInitialize, logLevel } = options;

  if (!manifest.capabilities.releaseSource) {
    throw new Error(
      "manifest.capabilities.releaseSource is required for createReleaseSourcePlugin",
    );
  }

  const router: MethodRouter = async (method, params, id) => {
    switch (method) {
      case "releases/poll": {
        const err = validateReleasePollParams(params);
        if (err) return invalidParamsError(id, err);
        return success(id, await provider.poll(params as ReleasePollRequest));
      }
      default:
        return null;
    }
  };

  createPluginServer({ manifest, onInitialize, logLevel, label: "release-source", router });
}
