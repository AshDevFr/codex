/**
 * @ashdev/codex-plugin-sdk
 *
 * SDK for building Codex plugins. Provides types, utilities, and a server
 * framework for communicating with Codex via JSON-RPC over stdio.
 *
 * @example
 * ```typescript
 * import {
 *   createMetadataPlugin,
 *   type MetadataProvider,
 *   type PluginManifest,
 * } from "@ashdev/codex-plugin-sdk";
 *
 * const manifest: PluginManifest = {
 *   name: "my-plugin",
 *   displayName: "My Plugin",
 *   version: "1.0.0",
 *   description: "A custom metadata plugin",
 *   author: "Your Name",
 *   protocolVersion: "1.0",
 *   capabilities: { metadataProvider: ["series"] },
 * };
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
 * createMetadataPlugin({ manifest, provider });
 * ```
 *
 * @packageDocumentation
 */

// Errors
export {
  ApiError,
  AuthError,
  ConfigError,
  NotFoundError,
  PluginError,
  RateLimitError,
} from "./errors.js";

// Logger
export { createLogger, Logger, type LoggerOptions, type LogLevel } from "./logger.js";

// Server
export {
  createMetadataPlugin,
  createSeriesMetadataPlugin,
  createSyncPlugin,
  type InitializeParams,
  type MetadataPluginOptions,
  type SeriesMetadataPluginOptions,
  type SyncPluginOptions,
} from "./server.js";

// Storage
export {
  PluginStorage,
  type StorageClearResponse,
  type StorageDeleteResponse,
  StorageError,
  type StorageGetResponse,
  type StorageKeyEntry,
  type StorageListResponse,
  type StorageSetResponse,
} from "./storage.js";

// Sync - sync provider protocol types
export type {
  ExternalUserInfo,
  SyncEntry,
  SyncEntryResult,
  SyncEntryResultStatus,
  SyncProgress,
  SyncProvider,
  SyncPullRequest,
  SyncPullResponse,
  SyncPushRequest,
  SyncPushResponse,
  SyncReadingStatus,
  SyncStatusResponse,
} from "./sync.js";

// Types (all remaining types re-exported from barrel)
export * from "./types/index.js";
