/**
 * Plugin server - handles JSON-RPC communication over stdio
 */

import { createInterface } from "node:readline";
import { PluginError } from "./errors.js";
import { createLogger, type Logger } from "./logger.js";
import type { MetadataContentType, MetadataProvider } from "./types/capabilities.js";
import type { PluginManifest } from "./types/manifest.js";
import type {
  MetadataGetParams,
  MetadataMatchParams,
  MetadataSearchParams,
} from "./types/protocol.js";
import {
  JSON_RPC_ERROR_CODES,
  type JsonRpcError,
  type JsonRpcRequest,
  type JsonRpcResponse,
} from "./types/rpc.js";

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
    } as JsonRpcError,
  };
}

/**
 * Initialize parameters received from Codex
 */
export interface InitializeParams {
  /** Plugin configuration */
  config?: Record<string, unknown>;
  /** Plugin credentials (API keys, tokens, etc.) */
  credentials?: Record<string, string>;
}

/**
 * Options for creating a metadata plugin
 */
export interface MetadataPluginOptions {
  /** Plugin manifest - must have capabilities.metadataProvider with content types */
  manifest: PluginManifest & {
    capabilities: { metadataProvider: MetadataContentType[] };
  };
  /** MetadataProvider implementation */
  provider: MetadataProvider;
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
  const { manifest, provider, onInitialize, logLevel = "info" } = options;
  const logger = createLogger({ name: manifest.name, level: logLevel });

  logger.info(`Starting plugin: ${manifest.displayName} v${manifest.version}`);

  const rl = createInterface({
    input: process.stdin,
    terminal: false,
  });

  rl.on("line", (line) => {
    void handleLine(line, manifest, provider, onInitialize, logger);
  });

  rl.on("close", () => {
    logger.info("stdin closed, shutting down");
    process.exit(0);
  });

  // Handle uncaught errors
  process.on("uncaughtException", (error) => {
    logger.error("Uncaught exception", error);
    process.exit(1);
  });

  process.on("unhandledRejection", (reason) => {
    logger.error("Unhandled rejection", reason);
  });
}

// =============================================================================
// Backwards Compatibility (deprecated)
// =============================================================================

/**
 * @deprecated Use createMetadataPlugin instead
 */
export function createSeriesMetadataPlugin(options: SeriesMetadataPluginOptions): void {
  // Convert legacy options to new format
  const newOptions: MetadataPluginOptions = {
    ...options,
    manifest: {
      ...options.manifest,
      capabilities: {
        ...options.manifest.capabilities,
        metadataProvider: ["series"] as MetadataContentType[],
      },
    },
  };
  createMetadataPlugin(newOptions);
}

/**
 * @deprecated Use MetadataPluginOptions instead
 */
export interface SeriesMetadataPluginOptions {
  /** Plugin manifest - must have capabilities.seriesMetadataProvider: true */
  manifest: PluginManifest & {
    capabilities: { seriesMetadataProvider: true };
  };
  /** SeriesMetadataProvider implementation */
  provider: MetadataProvider;
  /** Called when plugin receives initialize with credentials/config */
  onInitialize?: (params: InitializeParams) => void | Promise<void>;
  /** Log level (default: "info") */
  logLevel?: "debug" | "info" | "warn" | "error";
}

// =============================================================================
// Internal Implementation
// =============================================================================

async function handleLine(
  line: string,
  manifest: PluginManifest,
  provider: MetadataProvider,
  onInitialize: ((params: InitializeParams) => void | Promise<void>) | undefined,
  logger: Logger,
): Promise<void> {
  const trimmed = line.trim();
  if (!trimmed) return;

  let id: string | number | null = null;

  try {
    const request = JSON.parse(trimmed) as JsonRpcRequest;
    id = request.id;

    logger.debug(`Received request: ${request.method}`, { id: request.id });

    const response = await handleRequest(request, manifest, provider, onInitialize, logger);
    // Shutdown handler writes response directly and returns null
    if (response !== null) {
      writeResponse(response);
    }
  } catch (error) {
    if (error instanceof SyntaxError) {
      // JSON parse error
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
  provider: MetadataProvider,
  onInitialize: ((params: InitializeParams) => void | Promise<void>) | undefined,
  logger: Logger,
): Promise<JsonRpcResponse> {
  const { method, params, id } = request;

  switch (method) {
    case "initialize":
      // Call onInitialize callback if provided (to receive credentials/config)
      if (onInitialize) {
        await onInitialize(params as InitializeParams);
      }
      return {
        jsonrpc: "2.0",
        id,
        result: manifest,
      };

    case "ping":
      return {
        jsonrpc: "2.0",
        id,
        result: "pong",
      };

    case "shutdown": {
      logger.info("Shutdown requested");
      // Write response directly with callback to ensure it's flushed before exit
      const response: JsonRpcResponse = {
        jsonrpc: "2.0",
        id,
        result: null,
      };
      process.stdout.write(`${JSON.stringify(response)}\n`, () => {
        // Callback is called after the write is flushed to the OS
        process.exit(0);
      });
      // Return a sentinel that handleLine will recognize and skip normal writeResponse
      return null as unknown as JsonRpcResponse;
    }

    // Series metadata methods (scoped by content type)
    case "metadata/series/search": {
      const validationError = validateSearchParams(params);
      if (validationError) {
        return invalidParamsError(id, validationError);
      }
      return {
        jsonrpc: "2.0",
        id,
        result: await provider.search(params as MetadataSearchParams),
      };
    }

    case "metadata/series/get": {
      const validationError = validateGetParams(params);
      if (validationError) {
        return invalidParamsError(id, validationError);
      }
      return {
        jsonrpc: "2.0",
        id,
        result: await provider.get(params as MetadataGetParams),
      };
    }

    case "metadata/series/match": {
      if (!provider.match) {
        return {
          jsonrpc: "2.0",
          id,
          error: {
            code: JSON_RPC_ERROR_CODES.METHOD_NOT_FOUND,
            message: "This plugin does not support match",
          },
        };
      }
      const validationError = validateMatchParams(params);
      if (validationError) {
        return invalidParamsError(id, validationError);
      }
      return {
        jsonrpc: "2.0",
        id,
        result: await provider.match(params as MetadataMatchParams),
      };
    }

    // Future: book metadata methods
    // case "metadata/book/search":
    // case "metadata/book/get":
    // case "metadata/book/match":

    default:
      return {
        jsonrpc: "2.0",
        id,
        error: {
          code: JSON_RPC_ERROR_CODES.METHOD_NOT_FOUND,
          message: `Method not found: ${method}`,
        },
      };
  }
}

function writeResponse(response: JsonRpcResponse): void {
  // Write to stdout - this is the JSON-RPC channel
  process.stdout.write(`${JSON.stringify(response)}\n`);
}
