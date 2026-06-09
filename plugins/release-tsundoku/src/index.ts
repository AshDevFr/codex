/**
 * Tsundoku API-feed release-source plugin for Codex.
 *
 * Tsundoku exposes a series feed at `/api/v1/series/feed` carrying, per series,
 * the provider external IDs Codex matches on plus the merged volume/chapter
 * coverage. This plugin polls the **filtered** `POST` variant, matches each
 * returned series to a tracked Codex series by weighted external-ID voting, and
 * records release candidates.
 *
 * Each poll:
 *   1. Builds a match context from the host's `releases/list_tracked` rows
 *      (scoped by `requiresExternalIds`) and derives the `provider:externalId`
 *      filter set.
 *   2. `POST`s that filter to `/series/feed`, so the response contains only the
 *      tracked series — not the whole catalog. There is no persisted cursor:
 *      each poll re-walks the tracked set's current coverage and relies on
 *      host-side dedup to suppress unchanged releases. This keeps newly
 *      tracked series backfilled and untracked ones dropped, automatically.
 *   3. Matches each item (weighted voting), resolves cross-item (one feed entry
 *      per Codex series), and records via `releases/record`.
 *
 * The fetch, matching, and candidate mapping live in dedicated modules
 * (`fetcher`, `matcher`, `candidate`); this entry point owns plugin lifecycle,
 * config, source registration, and the poll orchestration.
 */

import {
  createLogger,
  createReleaseSourcePlugin,
  type HostRpcClient,
  HostRpcError,
  type InitializeParams,
  RELEASES_METHODS,
  type ReleaseCandidate,
  type ReleasePollRequest,
  type ReleasePollResponse,
  type TrackedSeriesEntry,
} from "@ashdev/codex-plugin-sdk";
import { feedItemToCandidate } from "./candidate.js";
import { type FeedItem, fetchFeedPage } from "./fetcher.js";
import { manifest } from "./manifest.js";
import { buildMatchContext, externalIdFilter, type MatchResult, matchItem } from "./matcher.js";

const logger = createLogger({ name: manifest.name, level: "info" });

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
  baseUrl: "",
  defaultLanguage: DEFAULT_LANGUAGE,
  pageLimit: DEFAULT_PAGE_LIMIT,
  requestTimeoutMs: DEFAULT_TIMEOUT_MS,
};

/** Reset state. Exported for tests; not part of the plugin contract. */
export function _resetState(): void {
  state.hostRpc = null;
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
 * whole catalog is polled under one logical source keyed `default`.
 *
 * No retry needed: the host parks an early reverse-RPC on its readiness
 * barrier until the plugin's capabilities + handlers are installed, so this
 * single call resolves cleanly even when fired from `onInitialize`.
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
  try {
    return await rpc.call<{ registered: number; pruned: number }>(
      RELEASES_METHODS.REGISTER_SOURCES,
      { sources },
    );
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    logger.error(`register_sources failed: ${reason}`);
    return null;
  }
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
 * Builds the match context from the host's tracked series and posts their
 * `provider:externalId` set to Tsundoku's filtered feed, so the response
 * contains only the tracked series (not the whole catalog). It walks every
 * page of that filtered feed each poll — there is no persisted cursor; the
 * in-poll cursor only paginates the current response, and host-side dedup
 * suppresses unchanged releases. Matched items are resolved cross-item (one
 * feed entry per Codex series) and recorded. Exported for tests.
 */
export async function poll(
  params: ReleasePollRequest,
  rpc: HostRpcClient,
  deps: PollDeps,
): Promise<ReleasePollResponse> {
  const sourceId = params.sourceId;

  // 1. Build the match context from the user's tracked series, and derive the
  //    `provider:externalId` filter we post to Tsundoku.
  const trackedEntries: TrackedSeriesEntry[] = [];
  for await (const entry of iterateTrackedSeries(rpc, sourceId)) {
    trackedEntries.push(entry);
  }
  const ctx = buildMatchContext(trackedEntries);
  const externalIds = externalIdFilter(ctx);
  if (externalIds.length === 0) {
    // Nothing to query. Posting an empty filter would mean "no filter" upstream
    // (the whole catalog), so skip entirely instead.
    logger.info(
      `poll: no tracked series carry a Tsundoku-known external ID (source=${sourceId}); nothing to fetch`,
    );
    return {
      notModified: false,
      upstreamStatus: 200,
      parsed: 0,
      matched: 0,
      recorded: 0,
      deduped: 0,
    };
  }

  // 2. Walk the filtered feed, collecting per-item matches. We resolve them
  //    after the walk (cross-item) rather than recording inline, so that when
  //    several feed entries map to the same Codex series we keep only the best
  //    one instead of polluting the ledger. The cursor here is ephemeral — it
  //    paginates this poll's response and is never persisted.
  let cursor: string | null = null;
  let parsed = 0;
  let worstStatus = 200;
  let pagesFetched = 0;
  const hits: Array<{ item: FeedItem; match: MatchResult }> = [];

  while (true) {
    const result = await fetchFeedPage(
      deps.baseUrl,
      { externalIds, cursor, limit: deps.pageLimit },
      { timeoutMs: deps.timeoutMs, fetchImpl: deps.fetchImpl },
    );

    if (result.kind === "error") {
      worstStatus = Math.max(worstStatus, result.status);
      // Couldn't fetch even the first page: surface a hard failure so the host
      // records `last_error` and the source shows it (e.g. an unreachable or
      // misconfigured `baseUrl`). A mid-walk failure, by contrast, keeps the
      // pages already processed and just stops.
      if (pagesFetched === 0) {
        throw new Error(`feed fetch failed (status ${result.status}): ${result.message}`);
      }
      logger.warn(`feed fetch failed (status ${result.status}): ${result.message}; stopping walk`);
      break;
    }

    pagesFetched++;
    const page = result.data;
    for (const item of page.items) {
      parsed++;
      const match = matchItem(item, ctx);
      if (match) {
        hits.push({ item, match });
      }
    }

    await reportProgress(rpc, parsed, parsed, `Processed ${parsed} feed items`);

    const next = page.nextCursor ?? null;
    if (!page.hasMore) break;
    if (!next) {
      // hasMore with no advancing cursor would loop forever; stop defensively.
      logger.warn("feed reported hasMore but no nextCursor; stopping walk");
      break;
    }
    if (page.items.length === 0) break;
    cursor = next;
  }

  // 3. Cross-item resolution: a Codex series should map to at most one feed
  //    entry. Group hits by Codex series; keep the highest-scoring one. If the
  //    top two tie (e.g. two entries match only via the same low-trust ID),
  //    it's genuinely ambiguous — skip both rather than record the wrong one.
  const byCodex = new Map<string, Array<{ item: FeedItem; match: MatchResult }>>();
  for (const hit of hits) {
    const arr = byCodex.get(hit.match.codexSeriesId);
    if (arr) {
      arr.push(hit);
    } else {
      byCodex.set(hit.match.codexSeriesId, [hit]);
    }
  }

  let matched = 0;
  let recorded = 0;
  let deduped = 0;
  let ambiguous = 0;
  let superseded = 0;

  for (const [codexSeriesId, group] of byCodex) {
    // Best score first; for ties prefer the most recently updated entry (newest
    // coverage). The same Tsundoku series appearing twice in one walk is not a
    // conflict — only *different* series tying is.
    group.sort((a, b) => b.match.score - a.match.score || b.item.updatedAt - a.item.updatedAt);
    if (
      group.length > 1 &&
      group[0].match.score === group[1].match.score &&
      group[0].item.seriesId !== group[1].item.seriesId
    ) {
      ambiguous += group.length;
      logger.warn(
        `ambiguous: feed entries from different Tsundoku series match Codex series ${codexSeriesId} at score ${group[0].match.score}; skipping`,
      );
      continue;
    }
    superseded += group.length - 1;

    const { item, match } = group[0];
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

  logger.info(
    `poll complete: source=${sourceId} tracked=${trackedEntries.length} parsed=${parsed} matched=${matched} recorded=${recorded} deduped=${deduped} ambiguous=${ambiguous} superseded=${superseded} worst_status=${worstStatus}`,
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
      if (!state.hostRpc) {
        throw new Error("Plugin not initialized: host RPC client missing");
      }
      if (!state.baseUrl) {
        throw new Error("Plugin not configured: baseUrl is required");
      }
      return poll(params, state.hostRpc, {
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
