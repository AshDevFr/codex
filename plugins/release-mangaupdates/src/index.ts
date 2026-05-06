/**
 * MangaUpdates RSS Release-Source Plugin for Codex.
 *
 * Polls per-series RSS feeds at MangaUpdates and announces new chapter /
 * volume releases for tracked series. The plugin is the first writer of
 * `release_ledger` rows in production — earlier phases build the
 * infrastructure, this one delivers the first real notification feed.
 *
 * Flow per `releases/poll`:
 *   1. Pull tracked-series scope from the host (`releases/list_tracked`).
 *      Filtered server-side to series with a `mangaupdates` external ID.
 *   2. For each series, conditional GET the RSS feed.
 *   3. Parse the response into items, then filter by:
 *      - per-series language list (admin / per-series config)
 *      - admin-configured group blocklist
 *   4. Build `ReleaseCandidate` rows and stream them via
 *      `releases/record`. The host's matcher applies the threshold and
 *      ledger dedup.
 *   5. Pass the new ETag back via the poll response so the host updates
 *      the source row.
 *
 * **Concurrency note:** The plugin host already serializes RPCs per plugin
 * process, so we don't need to throttle internally beyond an in-poll loop
 * that walks tracked series sequentially.
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
import { fetchSeriesFeed } from "./fetcher.js";
import { parseCommaList, passesFilters, resolveFilters } from "./filter.js";
import { EXTERNAL_ID_SOURCE_MANGAUPDATES, manifest } from "./manifest.js";
import { type ParsedRssItem, parseFeed } from "./parser.js";

const logger = createLogger({ name: manifest.name, level: "info" });

// =============================================================================
// Plugin-level state (set during initialize)
// =============================================================================

interface PluginState {
  hostRpc: HostRpcClient | null;
  /** Admin-configured group blocklist (lowercased exact match). */
  blockedGroupsCsv: string;
  /** Hard timeout for upstream fetches. */
  requestTimeoutMs: number;
}

const state: PluginState = {
  hostRpc: null,
  blockedGroupsCsv: "",
  requestTimeoutMs: 10_000,
};

/** Reset state. Exported for tests; not part of the plugin contract. */
export function _resetState(): void {
  state.hostRpc = null;
  state.blockedGroupsCsv = "";
  state.requestTimeoutMs = 10_000;
}

// =============================================================================
// Reverse-RPC wrappers (typed shorthands so the poll code reads cleanly)
// =============================================================================

interface ListTrackedResponse {
  tracked: TrackedSeriesEntry[];
  nextOffset?: number;
}

interface RecordResponse {
  ledgerId: string;
  deduped: boolean;
}

async function listTracked(
  rpc: HostRpcClient,
  sourceId: string,
  offset: number,
  limit: number,
): Promise<ListTrackedResponse> {
  return rpc.call<ListTrackedResponse>(RELEASES_METHODS.LIST_TRACKED, {
    sourceId,
    offset,
    limit,
  });
}

async function recordCandidate(
  rpc: HostRpcClient,
  sourceId: string,
  candidate: ReleaseCandidate,
): Promise<RecordResponse | null> {
  try {
    return await rpc.call<RecordResponse>(RELEASES_METHODS.RECORD, {
      sourceId,
      candidate,
    });
  } catch (err) {
    if (err instanceof HostRpcError) {
      // Threshold rejection / validation error / unknown source. Log and
      // skip; the next poll will retry the still-eligible candidates.
      logger.warn(
        `record failed for ${candidate.externalReleaseId}: ${err.message} (code ${err.code})`,
      );
    } else {
      const msg = err instanceof Error ? err.message : "unknown error";
      logger.warn(`record failed for ${candidate.externalReleaseId}: ${msg}`);
    }
    return null;
  }
}

// =============================================================================
// Iteration helpers
// =============================================================================

/**
 * Lazily walk all tracked-series pages from the host. Yields entries one
 * series at a time so the caller can interleave per-series fetches without
 * buffering the whole list (relevant for users tracking hundreds of series).
 */
async function* iterateTrackedSeries(
  rpc: HostRpcClient,
  sourceId: string,
): AsyncGenerator<TrackedSeriesEntry> {
  const pageSize = 200;
  let offset = 0;
  while (true) {
    const page = await listTracked(rpc, sourceId, offset, pageSize);
    for (const entry of page.tracked) {
      yield entry;
    }
    if (page.nextOffset === undefined || page.tracked.length === 0) return;
    offset = page.nextOffset;
  }
}

/**
 * Per-series effective language list. We use the host's `latestKnown*`
 * exposure plus the `externalIds` map to scope the fetch, but the
 * languages config is owned by the host (set on `series_tracking.languages`
 * with fallback to the server-wide default).
 *
 * However, the current `releases/list_tracked` response shape doesn't
 * expose per-series `languages` — see plan doc for this design choice.
 * For Phase 6 the plugin reads its admin-level group blocklist and emits
 * candidates with the language tag from the parsed entry; the host's
 * `latest_known_*` advance gate enforces the per-series language list
 * authoritatively (see `services/release/languages.rs`).
 *
 * We *also* want to drop out-of-language candidates client-side to keep the
 * ledger small and the inbox clean. Without per-series languages on the
 * tracked-series payload, the client-side filter degrades to a no-op
 * pass-everything for known languages — leaving it to the host's gate. The
 * group blocklist still applies.
 *
 * If a future protocol revision exposes `effectiveLanguages` on the
 * tracked-series entry, swap this stub for the real list and the existing
 * `passesFilters` will do the right thing.
 */
function effectiveLanguagesForSeries(_entry: TrackedSeriesEntry): string[] {
  return []; // empty = no client-side language gate; host gate is authoritative
}

/**
 * Map a `ParsedRssItem` to a `ReleaseCandidate`. Confidence is 1.0 because
 * the match is keyed by external ID — there's no fuzzy matching.
 */
function toCandidate(entry: TrackedSeriesEntry, item: ParsedRssItem): ReleaseCandidate {
  const candidate: ReleaseCandidate = {
    seriesMatch: {
      codexSeriesId: entry.seriesId,
      confidence: 1.0,
      reason: `mangaupdates_id:${entry.externalIds?.[EXTERNAL_ID_SOURCE_MANGAUPDATES] ?? ""}`,
    },
    externalReleaseId: item.externalReleaseId,
    chapter: item.chapter,
    volume: item.volume,
    language: item.language,
    groupOrUploader: item.group,
    payloadUrl: item.link.length > 0 ? item.link : `urn:mu:${item.externalReleaseId}`,
    observedAt: item.observedAt,
  };
  return candidate;
}

// =============================================================================
// Per-series poll
// =============================================================================

/** Outcome of a single per-series fetch+record cycle. */
export interface SeriesPollOutcome {
  seriesId: string;
  fetched: boolean;
  notModified: boolean;
  parsed: number;
  /** Of those parsed, how many passed client-side filters and were sent to record. */
  matched: number;
  recorded: number;
  /** Of those sent to record, how many the host deduped onto an existing row. */
  deduped: number;
  upstreamStatus: number;
  /** New ETag returned by upstream (only set when fetched=true). */
  etag: string | null;
  /** Error string if the per-series fetch failed; empty otherwise. */
  error: string;
}

/**
 * Poll a single series. Internal — exposed for testing.
 *
 * Aggregates the worst (highest) upstream status across the per-series
 * fetches at the call site so the host's per-host backoff layer sees real
 * 429/5xx signals.
 */
export async function pollSeries(
  rpc: HostRpcClient,
  sourceId: string,
  entry: TrackedSeriesEntry,
  options: {
    blockedGroups: string[];
    timeoutMs: number;
    fetchImpl?: typeof fetch;
  },
): Promise<SeriesPollOutcome> {
  const muId = entry.externalIds?.[EXTERNAL_ID_SOURCE_MANGAUPDATES];
  if (!muId) {
    return {
      seriesId: entry.seriesId,
      fetched: false,
      notModified: false,
      parsed: 0,
      matched: 0,
      recorded: 0,
      deduped: 0,
      upstreamStatus: 0,
      etag: null,
      error: "missing mangaupdates external ID",
    };
  }

  // We don't have per-series ETag here — that lives on the source row, not
  // the series. For a per-source feed (rss-uploader) ETags align cleanly;
  // for per-series feeds (this plugin) we'd need per-(source, series) state
  // to do conditional GETs per series. That's a future optimization; for
  // now we always do an unconditional GET. Daily polls + small per-series
  // bodies keep the bandwidth cost negligible.
  const result = await fetchSeriesFeed(muId, null, {
    fetchImpl: options.fetchImpl,
    timeoutMs: options.timeoutMs,
  });

  if (result.kind === "notModified") {
    return {
      seriesId: entry.seriesId,
      fetched: true,
      notModified: true,
      parsed: 0,
      matched: 0,
      recorded: 0,
      deduped: 0,
      upstreamStatus: 304,
      etag: null,
      error: "",
    };
  }

  if (result.kind === "error") {
    return {
      seriesId: entry.seriesId,
      fetched: false,
      notModified: false,
      parsed: 0,
      matched: 0,
      recorded: 0,
      deduped: 0,
      upstreamStatus: result.status,
      etag: null,
      error: result.message,
    };
  }

  // result.kind === "ok"
  const items = parseFeed(result.body);
  const filters = resolveFilters({
    languages: effectiveLanguagesForSeries(entry),
    blockedGroups: options.blockedGroups,
  });
  let matched = 0;
  let recorded = 0;
  let deduped = 0;
  for (const item of items) {
    if (!passesFilters(item, filters)) continue;
    matched++;
    const candidate = toCandidate(entry, item);
    const outcome = await recordCandidate(rpc, sourceId, candidate);
    if (!outcome) continue;
    if (outcome.deduped) {
      deduped++;
    } else {
      recorded++;
    }
  }
  return {
    seriesId: entry.seriesId,
    fetched: true,
    notModified: false,
    parsed: items.length,
    matched,
    recorded,
    deduped,
    upstreamStatus: 200,
    etag: result.etag,
    error: "",
  };
}

// =============================================================================
// Top-level poll handler
// =============================================================================

async function poll(params: ReleasePollRequest, rpc: HostRpcClient): Promise<ReleasePollResponse> {
  const sourceId = params.sourceId;
  const blockedGroups = parseCommaList(state.blockedGroupsCsv);

  let parsed = 0;
  let matched = 0;
  let recorded = 0;
  let deduped = 0;
  let worstStatus = 200;
  let lastEtag: string | null = null;
  let seenSeries = 0;
  // Series the host returned that lack a MangaUpdates external ID. A high
  // count here is the most common cause of an "empty" poll: the plugin
  // can't fetch a feed without an MU ID, so the user needs to populate
  // those (manual paste or metadata refresh from MangaBaka).
  let skippedNoMuId = 0;

  for await (const entry of iterateTrackedSeries(rpc, sourceId)) {
    seenSeries++;
    const outcome = await pollSeries(rpc, sourceId, entry, {
      blockedGroups,
      timeoutMs: state.requestTimeoutMs,
    });
    parsed += outcome.parsed;
    matched += outcome.matched;
    recorded += outcome.recorded;
    deduped += outcome.deduped;
    if (outcome.upstreamStatus > worstStatus) {
      worstStatus = outcome.upstreamStatus;
    }
    if (outcome.etag) lastEtag = outcome.etag;

    if (outcome.error === "missing mangaupdates external ID") {
      skippedNoMuId++;
    } else if (outcome.error) {
      logger.warn(`series ${entry.seriesId}: ${outcome.error} (status ${outcome.upstreamStatus})`);
    }
  }

  if (skippedNoMuId > 0) {
    logger.info(
      `skipped ${skippedNoMuId} of ${seenSeries} tracked series for source=${sourceId}: no mangaupdates external ID. Add one in the Tracking panel or run a metadata refresh.`,
    );
  }

  logger.info(
    `poll complete: source=${sourceId} series=${seenSeries} skipped=${skippedNoMuId} parsed=${parsed} matched=${matched} recorded=${recorded} deduped=${deduped} worst_status=${worstStatus}`,
  );

  // Report counters back to the host so the source's `last_summary` is
  // accurate. Without these the host only sees the (empty) `candidates`
  // payload — we record via reverse-RPC mid-poll — and the badge reads
  // "Fetched 0 items" no matter what actually happened.
  // Per-series ETags don't align with the per-source state slot, so we
  // intentionally leave `etag` undefined unless we actually saw one
  // (which today we won't, since we don't pass If-None-Match per series).
  return {
    notModified: false,
    upstreamStatus: worstStatus,
    parsed,
    matched,
    recorded,
    deduped,
    ...(lastEtag !== null ? { etag: lastEtag } : {}),
  };
}

// =============================================================================
// Plugin Initialization
// =============================================================================

/**
 * Register a single static source row representing the MangaUpdates batch
 * feed. Unlike Nyaa (one row per uploader), MangaUpdates polls all tracked
 * series under one logical feed, so we always declare exactly one row keyed
 * `default`. Retries on `METHOD_NOT_FOUND` to handle the brief race where
 * the host has not yet installed the releases reverse-RPC handler.
 */
export async function registerSources(
  rpc: HostRpcClient,
): Promise<{ registered: number; pruned: number } | null> {
  const sources = [
    {
      sourceKey: "default",
      displayName: "MangaUpdates Releases",
      kind: "rss-series" as const,
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

createReleaseSourcePlugin({
  manifest,
  provider: {
    async poll(params: ReleasePollRequest): Promise<ReleasePollResponse> {
      if (!state.hostRpc) {
        throw new Error("Plugin not initialized: hostRpc client missing");
      }
      return poll(params, state.hostRpc);
    },
  },
  logLevel: "info",
  async onInitialize(params: InitializeParams) {
    state.hostRpc = params.hostRpc;
    const ac = params.adminConfig ?? {};
    if (typeof ac.blockedGroups === "string") {
      state.blockedGroupsCsv = ac.blockedGroups;
    }
    if (typeof ac.requestTimeoutMs === "number" && Number.isFinite(ac.requestTimeoutMs)) {
      state.requestTimeoutMs = Math.max(1_000, Math.min(ac.requestTimeoutMs, 60_000));
    }
    logger.info(
      `initialized: blockedGroups=${state.blockedGroupsCsv ? "set" : "empty"} timeoutMs=${state.requestTimeoutMs}`,
    );

    // Materialize the single static source row. Deferred to a microtask so
    // we run *after* the host installs the releases reverse-RPC handler.
    queueMicrotask(() => {
      void registerSources(params.hostRpc).then((result) => {
        if (result) {
          logger.info(`register_sources: registered=${result.registered} pruned=${result.pruned}`);
        }
      });
    });
  },
});

logger.info("MangaUpdates release-source plugin started");
