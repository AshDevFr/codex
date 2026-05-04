/**
 * Nyaa.si Release-Source Plugin for Codex.
 *
 * Polls Nyaa user / search RSS feeds for an admin-configured uploader
 * allowlist and announces new releases for tracked series. Matching is
 * alias-based: each parsed Nyaa title is normalized and compared to every
 * tracked series' alias list. Confidence is 0.95 on exact normalized match,
 * dropping to a fuzzy floor of 0.7 for near-matches; below that, the
 * candidate is silently dropped (the host's threshold would reject it
 * anyway).
 *
 * Flow per `releases/poll`:
 *   1. Read uploader subscriptions from admin config.
 *   2. Pull tracked-series + aliases from the host
 *      (`releases/list_tracked`).
 *   3. For each subscription, conditional GET the RSS feed (ETag stored on
 *      the source row; we don't have per-subscription state slots).
 *   4. Parse each item; match against tracked aliases; emit a candidate via
 *      `releases/record`.
 *   5. Aggregate the worst upstream status across all subscriptions for the
 *      host's per-host backoff layer.
 *
 * Design notes:
 *   - **One source row, many uploaders.** The plan calls for "one source
 *     row per uploader", but the host has no admin endpoint for creating
 *     `release_sources` rows; admins create one row when enabling the
 *     plugin and the plugin walks all subscriptions during a single poll.
 *     Mirrors how MangaUpdates polls all tracked series within one source
 *     row's `poll(sourceId)` call.
 *   - **ETag is a single bucket.** The source row stores one ETag — we use
 *     it on the *first* uploader fetched and rotate fresh ETags out of the
 *     response on subsequent polls. Daily polls + small RSS bodies make
 *     this acceptable; per-subscription ETags would need per-(source,
 *     subscription) state, deferred.
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
import {
  fetchSubscriptionFeed,
  parseSubscriptionList,
  type UploaderSubscription,
} from "./fetcher.js";
import {
  DEFAULT_MIN_CONFIDENCE,
  DEFAULT_POLL_INTERVAL_S,
  DEFAULT_REQUEST_TIMEOUT_MS,
  manifest,
} from "./manifest.js";
import { type AliasCandidate, type AliasMatch, matchSeries } from "./matcher.js";
import { type ParsedRssItem, parseFeed } from "./parser.js";

const logger = createLogger({ name: manifest.name, level: "info" });

// =============================================================================
// Plugin-level state (set during initialize)
// =============================================================================

interface PluginState {
  hostRpc: HostRpcClient | null;
  /** Parsed admin uploader subscription list. */
  subscriptions: UploaderSubscription[];
  /** Hard timeout for upstream fetches. */
  requestTimeoutMs: number;
  /** Minimum confidence floor — passed to the matcher's `fuzzyFloor`. */
  minConfidence: number;
  /** Override base URL (for tests / mirrors). */
  baseUrl: string | null;
}

const state: PluginState = {
  hostRpc: null,
  subscriptions: [],
  requestTimeoutMs: DEFAULT_REQUEST_TIMEOUT_MS,
  minConfidence: DEFAULT_MIN_CONFIDENCE,
  baseUrl: null,
};

/** Reset state. Exported for tests; not part of the plugin contract. */
export function _resetState(): void {
  state.hostRpc = null;
  state.subscriptions = [];
  state.requestTimeoutMs = DEFAULT_REQUEST_TIMEOUT_MS;
  state.minConfidence = DEFAULT_MIN_CONFIDENCE;
  state.baseUrl = null;
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
 * Pull every tracked-series page from the host. We can't stream
 * subscription-by-subscription because each Nyaa item has to be matched
 * against the *full* alias set; partial pages would leak misses.
 */
export async function fetchAllTracked(
  rpc: HostRpcClient,
  sourceId: string,
): Promise<AliasCandidate[]> {
  const out: AliasCandidate[] = [];
  const pageSize = 200;
  let offset = 0;
  while (true) {
    const page = await listTracked(rpc, sourceId, offset, pageSize);
    for (const entry of page.tracked) {
      const aliases = entry.aliases ?? [];
      // Drop entries with no aliases — Nyaa matching is alias-only.
      if (aliases.length === 0) continue;
      out.push({ seriesId: entry.seriesId, aliases });
    }
    if (page.nextOffset === undefined || page.tracked.length === 0) return out;
    offset = page.nextOffset;
  }
}

// =============================================================================
// Per-subscription poll
// =============================================================================

/** Outcome of a single per-subscription fetch+parse cycle. */
export interface SubscriptionPollOutcome {
  subscription: UploaderSubscription;
  fetched: boolean;
  notModified: boolean;
  parsed: number;
  matched: number;
  recorded: number;
  upstreamStatus: number;
  /** New ETag returned by upstream (only set when fetched=true). */
  etag: string | null;
  error: string;
}

/**
 * Build a `ReleaseCandidate` from a parsed RSS item + the matcher's verdict.
 *
 * Language is hardcoded to `"en"` — Nyaa releases don't carry a language tag
 * in the title or RSS metadata. English-only is the right default for the
 * uploader allowlist this plugin is designed around (`1r0n`, etc.); admins
 * who add non-English uploaders should configure tracked series' languages
 * accordingly. The host's `latest_known_*` advance gate enforces the
 * per-series language list.
 */
function toCandidate(
  match: AliasMatch,
  item: ParsedRssItem,
  subscription: UploaderSubscription,
): ReleaseCandidate {
  const formatHints: Record<string, unknown> = { ...item.formatHints };
  if (item.chapterRangeEnd !== null) {
    formatHints.chapterRangeEnd = item.chapterRangeEnd;
  }
  if (item.volumeRangeEnd !== null) {
    formatHints.volumeRangeEnd = item.volumeRangeEnd;
  }
  formatHints.subscription = `${subscription.kind}:${subscription.identifier}`;

  return {
    seriesMatch: {
      codexSeriesId: match.seriesId,
      confidence: match.confidence,
      reason: match.reason,
    },
    externalReleaseId: item.externalReleaseId,
    chapter: item.chapter,
    volume: item.volume,
    language: "en",
    groupOrUploader: item.group ?? (subscription.kind === "user" ? subscription.identifier : null),
    payloadUrl: item.link.length > 0 ? item.link : `urn:nyaa:${item.externalReleaseId}`,
    infoHash: item.infoHash,
    formatHints,
    observedAt: item.observedAt,
  };
}

/**
 * Poll a single uploader subscription. Internal — exposed for testing.
 */
export async function pollSubscription(
  rpc: HostRpcClient,
  sourceId: string,
  subscription: UploaderSubscription,
  candidates: AliasCandidate[],
  options: {
    previousEtag: string | null;
    timeoutMs: number;
    minConfidence: number;
    baseUrl?: string | null;
    fetchImpl?: typeof fetch;
  },
): Promise<SubscriptionPollOutcome> {
  const result = await fetchSubscriptionFeed(subscription, options.previousEtag, null, {
    fetchImpl: options.fetchImpl,
    timeoutMs: options.timeoutMs,
    ...(options.baseUrl ? { baseUrl: options.baseUrl } : {}),
  });

  if (result.kind === "notModified") {
    return {
      subscription,
      fetched: true,
      notModified: true,
      parsed: 0,
      matched: 0,
      recorded: 0,
      upstreamStatus: 304,
      etag: null,
      error: "",
    };
  }

  if (result.kind === "error") {
    return {
      subscription,
      fetched: false,
      notModified: false,
      parsed: 0,
      matched: 0,
      recorded: 0,
      upstreamStatus: result.status,
      etag: null,
      error: result.message,
    };
  }

  // result.kind === "ok"
  const items = parseFeed(result.body);
  let matched = 0;
  let recorded = 0;
  for (const item of items) {
    const m = matchSeries(item.seriesGuess, candidates, {
      fuzzyFloor: options.minConfidence,
    });
    if (m === null) continue;
    matched++;
    const candidate = toCandidate(m, item, subscription);
    const outcome = await recordCandidate(rpc, sourceId, candidate);
    if (outcome && !outcome.deduped) recorded++;
  }
  return {
    subscription,
    fetched: true,
    notModified: false,
    parsed: items.length,
    matched,
    recorded,
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

  if (state.subscriptions.length === 0) {
    logger.warn("no uploader subscriptions configured; nothing to poll");
    return { notModified: false, upstreamStatus: 200 };
  }

  // 1. Pull tracked-series + aliases.
  const tracked = await fetchAllTracked(rpc, sourceId);
  if (tracked.length === 0) {
    logger.info(`no tracked series with aliases for source=${sourceId}`);
    return { notModified: false, upstreamStatus: 200 };
  }

  let parsed = 0;
  let matched = 0;
  let recorded = 0;
  let worstStatus = 200;
  let lastEtag: string | null = null;

  // 2. Walk subscriptions in declaration order. We use the ETag stored on
  //    the source row (passed as `params.etag`) for the *first* fetch;
  //    subsequent fetches start fresh because the ETag belongs to whichever
  //    subscription was polled last, not this one.
  let firstFetch = true;
  for (const sub of state.subscriptions) {
    const outcome = await pollSubscription(rpc, sourceId, sub, tracked, {
      previousEtag: firstFetch ? (params.etag ?? null) : null,
      timeoutMs: state.requestTimeoutMs,
      minConfidence: state.minConfidence,
      ...(state.baseUrl ? { baseUrl: state.baseUrl } : {}),
    });
    firstFetch = false;
    parsed += outcome.parsed;
    matched += outcome.matched;
    recorded += outcome.recorded;
    if (outcome.upstreamStatus > worstStatus) worstStatus = outcome.upstreamStatus;
    if (outcome.etag) lastEtag = outcome.etag;
    if (outcome.error) {
      logger.warn(
        `subscription ${sub.kind}:${sub.identifier}: ${outcome.error} (status ${outcome.upstreamStatus})`,
      );
    }
  }

  logger.info(
    `poll complete: source=${sourceId} subscriptions=${state.subscriptions.length} tracked=${tracked.length} parsed=${parsed} matched=${matched} recorded=${recorded} worst_status=${worstStatus}`,
  );

  return {
    notModified: false,
    upstreamStatus: worstStatus,
    ...(lastEtag !== null ? { etag: lastEtag } : {}),
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
      return poll(params, state.hostRpc);
    },
  },
  logLevel: "info",
  onInitialize(params: InitializeParams) {
    state.hostRpc = params.hostRpc;
    const ac = params.adminConfig ?? {};
    if (typeof ac.uploaders === "string") {
      state.subscriptions = parseSubscriptionList(ac.uploaders);
    }
    if (typeof ac.requestTimeoutMs === "number" && Number.isFinite(ac.requestTimeoutMs)) {
      state.requestTimeoutMs = Math.max(1_000, Math.min(ac.requestTimeoutMs, 60_000));
    }
    if (typeof ac.baseUrl === "string" && ac.baseUrl.trim().length > 0) {
      state.baseUrl = ac.baseUrl.trim();
    }
    logger.info(
      `initialized: subscriptions=${state.subscriptions.length} timeoutMs=${state.requestTimeoutMs} minConfidence=${state.minConfidence} defaultPoll=${DEFAULT_POLL_INTERVAL_S}s`,
    );
  },
});

logger.info("Nyaa release-source plugin started");
