/**
 * Tsundoku API-feed release-source plugin for Codex.
 *
 * Tsundoku exposes a single, catalog-wide incremental feed
 * (`GET /api/v1/series/feed`) ordered by `(updatedAt, id)` and walked with an
 * opaque keyset cursor. Each item carries the provider external IDs Codex
 * matches on plus the merged volume/chapter coverage for the series. This
 * plugin polls that feed, matches each item to a tracked Codex series by
 * *exact* external ID (no fuzzy matching), and records release candidates.
 *
 * Unlike the per-series RSS plugins (MangaUpdates, Nyaa), the feed is not
 * scoped to the user's tracked series — it's the whole Tsundoku catalog's
 * recent activity. So each poll:
 *   1. Loads the stored cursor from the plugin KV store.
 *   2. Builds a reverse index `"provider:id" -> codexSeriesId` from the
 *      host's `releases/list_tracked` rows (scoped by `requiresExternalIds`).
 *   3. Walks the feed from the cursor, matching each item against the index
 *      and streaming matches via `releases/record`.
 *   4. Persists the advancing cursor back to the KV store.
 *
 * The feed walk and matching land in dedicated modules (`fetcher`,
 * `matcher`, `candidate`); this entry point owns plugin lifecycle, config,
 * source registration, and the poll orchestration that ties them together.
 */

import {
  createLogger,
  createReleaseSourcePlugin,
  type HostRpcClient,
  HostRpcError,
  type InitializeParams,
  type PluginStorage,
  RELEASES_METHODS,
  type ReleasePollRequest,
  type ReleasePollResponse,
} from "@ashdev/codex-plugin-sdk";
import { manifest } from "./manifest.js";

const logger = createLogger({ name: manifest.name, level: "info" });

/** KV-store key under which the feed cursor bookmark is persisted. */
export const CURSOR_STORAGE_KEY = "feed_cursor";

/** Default feed page size when config omits / mis-types `pageLimit`. */
const DEFAULT_PAGE_LIMIT = 100;
/** Tsundoku caps the feed page size at 500. */
const MAX_PAGE_LIMIT = 500;
/** Default per-request timeout when config omits / mis-types `requestTimeoutMs`. */
const DEFAULT_TIMEOUT_MS = 10_000;
const MIN_TIMEOUT_MS = 1_000;
const MAX_TIMEOUT_MS = 60_000;
const DEFAULT_LANGUAGE = "en";

// =============================================================================
// Plugin-level state (set during initialize)
// =============================================================================

interface PluginState {
  hostRpc: HostRpcClient | null;
  storage: PluginStorage | null;
  /** Tsundoku instance base URL (no trailing slash), e.g. `https://t.example.com`. */
  baseUrl: string;
  /** ISO 639-1 tag stamped on every candidate (the feed carries none). */
  defaultLanguage: string;
  /** Feed page size (1..=MAX_PAGE_LIMIT). */
  pageLimit: number;
  /** Hard timeout for a single feed-page fetch. */
  requestTimeoutMs: number;
}

const state: PluginState = {
  hostRpc: null,
  storage: null,
  baseUrl: "",
  defaultLanguage: DEFAULT_LANGUAGE,
  pageLimit: DEFAULT_PAGE_LIMIT,
  requestTimeoutMs: DEFAULT_TIMEOUT_MS,
};

/** Reset state. Exported for tests; not part of the plugin contract. */
export function _resetState(): void {
  state.hostRpc = null;
  state.storage = null;
  state.baseUrl = "";
  state.defaultLanguage = DEFAULT_LANGUAGE;
  state.pageLimit = DEFAULT_PAGE_LIMIT;
  state.requestTimeoutMs = DEFAULT_TIMEOUT_MS;
}

/** Strip a single trailing slash so URL building stays predictable. */
export function normalizeBaseUrl(raw: string): string {
  return raw.trim().replace(/\/+$/, "");
}

// =============================================================================
// Source registration
// =============================================================================

/**
 * Register the single static source row representing the Tsundoku feed. The
 * whole catalog is polled under one logical source keyed `default`. Retries
 * on `METHOD_NOT_FOUND` to absorb the brief race where the host has not yet
 * installed the releases reverse-RPC handler at startup.
 */
export async function registerSources(
  rpc: HostRpcClient,
): Promise<{ registered: number; pruned: number } | null> {
  const sources = [
    {
      sourceKey: "default",
      displayName: "Tsundoku Releases",
      kind: "api-feed" as const,
      config: null,
    },
  ];
  const maxAttempts = 5;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await rpc.call<{ registered: number; pruned: number }>(
        RELEASES_METHODS.REGISTER_SOURCES,
        { sources },
      );
    } catch (err) {
      const isMethodNotFound = err instanceof HostRpcError && err.code === -32601;
      if (isMethodNotFound && attempt < maxAttempts) {
        await new Promise((r) => setTimeout(r, 50 * attempt));
        continue;
      }
      const reason = err instanceof Error ? err.message : String(err);
      logger.error(`register_sources failed: ${reason}`);
      return null;
    }
  }
  return null;
}

// =============================================================================
// Poll
// =============================================================================

/**
 * Top-level poll handler. The feed walk + matching are added in dedicated
 * modules; until they land, the source registers and polls cleanly while
 * recording no candidates. Exported for tests.
 */
export async function poll(
  _params: ReleasePollRequest,
  _rpc: HostRpcClient,
): Promise<ReleasePollResponse> {
  return {
    notModified: false,
    upstreamStatus: 200,
    parsed: 0,
    matched: 0,
    recorded: 0,
    deduped: 0,
  };
}

// =============================================================================
// Plugin Initialization
// =============================================================================

createReleaseSourcePlugin({
  manifest,
  provider: {
    async poll(params: ReleasePollRequest): Promise<ReleasePollResponse> {
      if (!state.hostRpc) {
        throw new Error("Plugin not initialized: hostRpc client missing");
      }
      if (!state.baseUrl) {
        throw new Error("Plugin not configured: baseUrl is required");
      }
      return poll(params, state.hostRpc);
    },
  },
  logLevel: "info",
  async onInitialize(params: InitializeParams) {
    state.hostRpc = params.hostRpc;
    state.storage = params.storage;

    const ac = params.adminConfig ?? {};
    if (typeof ac.baseUrl === "string") {
      state.baseUrl = normalizeBaseUrl(ac.baseUrl);
    }
    if (typeof ac.defaultLanguage === "string" && ac.defaultLanguage.trim().length > 0) {
      state.defaultLanguage = ac.defaultLanguage.trim().toLowerCase();
    }
    if (typeof ac.pageLimit === "number" && Number.isFinite(ac.pageLimit)) {
      state.pageLimit = Math.max(1, Math.min(Math.trunc(ac.pageLimit), MAX_PAGE_LIMIT));
    }
    if (typeof ac.requestTimeoutMs === "number" && Number.isFinite(ac.requestTimeoutMs)) {
      state.requestTimeoutMs = Math.max(
        MIN_TIMEOUT_MS,
        Math.min(ac.requestTimeoutMs, MAX_TIMEOUT_MS),
      );
    }

    if (!state.baseUrl) {
      logger.warn(
        "initialized without a baseUrl — set it in the plugin config; polls will error until then",
      );
    }
    logger.info(
      `initialized: baseUrl=${state.baseUrl || "(unset)"} defaultLanguage=${state.defaultLanguage} pageLimit=${state.pageLimit} timeoutMs=${state.requestTimeoutMs}`,
    );

    // Materialize the single static source row. Deferred to a microtask so we
    // run *after* the host installs the releases reverse-RPC handler.
    queueMicrotask(() => {
      void registerSources(params.hostRpc).then((result) => {
        if (result) {
          logger.info(`register_sources: registered=${result.registered} pruned=${result.pruned}`);
        }
      });
    });
  },
});

logger.info("Tsundoku release-source plugin started");
