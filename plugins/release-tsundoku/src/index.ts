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
  type ReleaseCandidate,
  type ReleasePollRequest,
  type ReleasePollResponse,
  type TrackedSeriesEntry,
} from "@ashdev/codex-plugin-sdk";
import { feedItemToCandidate } from "./candidate.js";
import { fetchFeedPage } from "./fetcher.js";
import { manifest } from "./manifest.js";
import { buildIndex, matchItem } from "./matcher.js";

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
// Cursor persistence (plugin KV store)
// =============================================================================

/**
 * Load the feed cursor bookmark from the KV store. Returns `null` when no
 * cursor has been stored yet (first run) or when the read fails — a missing
 * cursor simply restarts the walk from the beginning, which is safe given
 * at-least-once delivery + host-side dedup.
 */
export async function loadCursor(storage: PluginStorage): Promise<string | null> {
  try {
    const res = await storage.get(CURSOR_STORAGE_KEY);
    const data = res?.data;
    return typeof data === "string" && data.length > 0 ? data : null;
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    logger.warn(`failed to load cursor; restarting from the beginning: ${reason}`);
    return null;
  }
}

/**
 * Persist the feed cursor bookmark. Best-effort: a failed write is logged but
 * never aborts a poll — the worst case is re-walking already-seen pages on
 * the next poll, which dedups host-side.
 */
export async function saveCursor(storage: PluginStorage, cursor: string): Promise<void> {
  try {
    await storage.set(CURSOR_STORAGE_KEY, cursor);
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    logger.warn(`failed to persist cursor "${cursor}": ${reason}`);
  }
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
// Reverse-RPC wrappers
// =============================================================================

interface ListTrackedResponse {
  tracked: TrackedSeriesEntry[];
  nextOffset?: number;
}

interface RecordResponse {
  ledgerId: string;
  deduped: boolean;
}

/** Page size for the tracked-series sweep that builds the match index. */
const TRACKED_PAGE_SIZE = 200;

/**
 * Lazily walk all tracked-series pages from the host. Yields one entry at a
 * time so the caller can build the reverse index without materializing every
 * page at once.
 */
async function* iterateTrackedSeries(
  rpc: HostRpcClient,
  sourceId: string,
): AsyncGenerator<TrackedSeriesEntry> {
  let offset = 0;
  while (true) {
    const page = await rpc.call<ListTrackedResponse>(RELEASES_METHODS.LIST_TRACKED, {
      sourceId,
      offset,
      limit: TRACKED_PAGE_SIZE,
    });
    for (const entry of page.tracked) {
      yield entry;
    }
    if (page.nextOffset === undefined || page.tracked.length === 0) return;
    offset = page.nextOffset;
  }
}

/**
 * Submit one candidate to the host ledger. Per-candidate failures (threshold
 * rejection, validation, transient host error) are logged and swallowed so a
 * single bad item never aborts the walk; the next poll retries it.
 */
async function recordCandidate(
  rpc: HostRpcClient,
  sourceId: string,
  candidate: ReleaseCandidate,
): Promise<RecordResponse | null> {
  try {
    return await rpc.call<RecordResponse>(RELEASES_METHODS.RECORD, { sourceId, candidate });
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    const code = err instanceof HostRpcError ? ` (code ${err.code})` : "";
    logger.warn(`record failed for ${candidate.externalReleaseId}: ${reason}${code}`);
    return null;
  }
}

/**
 * Best-effort progress emit. Failures (including older hosts without the
 * method) are swallowed — progress is a UX nicety, never a reason to abort.
 */
async function reportProgress(
  rpc: HostRpcClient,
  current: number,
  total: number,
  message: string,
): Promise<void> {
  try {
    await rpc.call(RELEASES_METHODS.REPORT_PROGRESS, { current, total, message });
  } catch (err) {
    if (err instanceof HostRpcError && err.code === -32601) return;
    const reason = err instanceof Error ? err.message : String(err);
    logger.debug(`report_progress dropped: ${reason}`);
  }
}

// =============================================================================
// Poll
// =============================================================================

/** Dependencies a poll needs, defaulted from plugin state at the call site. */
export interface PollDeps {
  storage: PluginStorage;
  /** Tsundoku base URL (no trailing slash). */
  baseUrl: string;
  /** Language stamped on every candidate. */
  language: string;
  /** Feed page size. */
  pageLimit: number;
  /** Per-page fetch timeout. */
  timeoutMs: number;
  /** Custom `fetch` impl (tests). */
  fetchImpl?: typeof fetch;
}

/**
 * Top-level poll handler.
 *
 * Builds the exact-match index from the host's tracked series, then walks the
 * Tsundoku feed from the stored cursor: each item is matched by external ID
 * and, on a hit, recorded as a candidate. The cursor is persisted after every
 * processed page so an interrupted walk resumes from the last completed page
 * (keyset pagination is gap-free, and host-side dedup makes re-processing
 * safe). Exported for tests.
 */
export async function poll(
  params: ReleasePollRequest,
  rpc: HostRpcClient,
  deps: PollDeps,
): Promise<ReleasePollResponse> {
  const sourceId = params.sourceId;

  // 1. Build the reverse index from the user's tracked series. The feed spans
  //    the whole Tsundoku catalog, so this is what scopes it to the user.
  const trackedEntries: TrackedSeriesEntry[] = [];
  for await (const entry of iterateTrackedSeries(rpc, sourceId)) {
    trackedEntries.push(entry);
  }
  const index = buildIndex(trackedEntries);
  if (index.size === 0) {
    logger.info(
      `poll: no tracked series carry a Tsundoku-known external ID (source=${sourceId}); nothing to match`,
    );
  }

  // 2. Walk the feed from the stored cursor.
  let cursor = await loadCursor(deps.storage);
  let parsed = 0;
  let matched = 0;
  let recorded = 0;
  let deduped = 0;
  let worstStatus = 200;

  while (true) {
    const result = await fetchFeedPage(deps.baseUrl, cursor, deps.pageLimit, {
      timeoutMs: deps.timeoutMs,
      fetchImpl: deps.fetchImpl,
    });

    if (result.kind === "error") {
      worstStatus = Math.max(worstStatus, result.status);
      logger.warn(
        `feed fetch failed (status ${result.status}): ${result.message}; stopping walk, cursor preserved`,
      );
      break;
    }

    const page = result.data;
    for (const item of page.items) {
      parsed++;
      const match = matchItem(item, index);
      if (!match) continue;
      matched++;
      const candidate = feedItemToCandidate(item, match, {
        baseUrl: deps.baseUrl,
        language: deps.language,
      });
      const outcome = await recordCandidate(rpc, sourceId, candidate);
      if (!outcome) continue;
      if (outcome.deduped) {
        deduped++;
      } else {
        recorded++;
      }
    }

    // Advance + persist the cursor before deciding whether to continue, so an
    // error or crash on the next page resumes from this point.
    const next = page.nextCursor ?? null;
    if (next) {
      cursor = next;
      await saveCursor(deps.storage, next);
    }

    await reportProgress(rpc, parsed, parsed, `Processed ${parsed} feed items`);

    if (!page.hasMore) break;
    if (!next) {
      // hasMore with no advancing cursor would loop forever; stop defensively.
      logger.warn("feed reported hasMore but no nextCursor; stopping walk");
      break;
    }
    if (page.items.length === 0) break;
  }

  logger.info(
    `poll complete: source=${sourceId} tracked=${trackedEntries.length} parsed=${parsed} matched=${matched} recorded=${recorded} deduped=${deduped} worst_status=${worstStatus}`,
  );

  return {
    notModified: false,
    upstreamStatus: worstStatus,
    parsed,
    matched,
    recorded,
    deduped,
  };
}

// =============================================================================
// Plugin Initialization
// =============================================================================

createReleaseSourcePlugin({
  manifest,
  provider: {
    async poll(params: ReleasePollRequest): Promise<ReleasePollResponse> {
      if (!state.hostRpc || !state.storage) {
        throw new Error("Plugin not initialized: host RPC / storage client missing");
      }
      if (!state.baseUrl) {
        throw new Error("Plugin not configured: baseUrl is required");
      }
      return poll(params, state.hostRpc, {
        storage: state.storage,
        baseUrl: state.baseUrl,
        language: state.defaultLanguage,
        pageLimit: state.pageLimit,
        timeoutMs: state.requestTimeoutMs,
      });
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
