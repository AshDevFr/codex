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
 * Source-row model:
 *   - On `onInitialize` (which the host re-runs after every config save),
 *     the plugin parses the admin's `uploaders` CSV and calls
 *     `releases/register_sources` with one entry per subscription. The host
 *     materializes one `release_sources` row per uploader, keyed on
 *     `(plugin_id, sourceKey)` where `sourceKey` is `kind:identifier`
 *     (e.g. `user:tsuna69`, `query:luminousscans`, `params:c=3_1&q=berserk`).
 *   - The host scheduler fires one `releases/poll` task per source row, so
 *     each uploader has its own poll cadence, ETag, and last-error status.
 *
 * Flow per `releases/poll`:
 *   1. Recover the subscription from `params.config.subscription` (or fall
 *      back to parsing `params.sourceKey`).
 *   2. Pull tracked-series + aliases from the host
 *      (`releases/list_tracked`).
 *   3. Conditional GET the RSS feed using `params.etag`.
 *   4. Parse each item; match against tracked aliases; emit a candidate via
 *      `releases/record`.
 *   5. Return the new ETag and upstream status for the host's per-host
 *      backoff layer.
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
  sourceKeyToSubscription,
  subscriptionToSourceKey,
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
  /** Of those sent to record, how many the host deduped onto an existing row. */
  deduped: number;
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
    payloadUrl:
      item.pageUrl ?? (item.link.length > 0 ? item.link : `urn:nyaa:${item.externalReleaseId}`),
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
      deduped: 0,
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
      deduped: 0,
      upstreamStatus: result.status,
      etag: null,
      error: result.message,
    };
  }

  // result.kind === "ok"
  const items = parseFeed(result.body);
  let matched = 0;
  let recorded = 0;
  let deduped = 0;
  for (const item of items) {
    const m = matchSeries(item.seriesGuess, candidates, {
      fuzzyFloor: options.minConfidence,
    });
    if (m === null) continue;
    matched++;
    const candidate = toCandidate(m, item, subscription);
    const outcome = await recordCandidate(rpc, sourceId, candidate);
    if (!outcome) continue;
    if (outcome.deduped) {
      deduped++;
    } else {
      recorded++;
    }
  }
  return {
    subscription,
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

/**
 * Resolve the subscription this poll request is for. The host stamps every
 * `release_sources` row with its plugin-defined `config` (set at register
 * time), so the preferred path is `params.config.subscription`. If a row
 * pre-dates the config field (e.g. created in a previous plugin version),
 * fall back to parsing `params.sourceKey`.
 */
function resolveSubscription(params: ReleasePollRequest): UploaderSubscription | null {
  const cfg = params.config as { subscription?: unknown } | undefined | null;
  const fromConfig = cfg?.subscription;
  if (fromConfig && typeof fromConfig === "object") {
    const obj = fromConfig as Record<string, unknown>;
    const kind = obj.kind;
    const identifier = obj.identifier;
    if (
      typeof identifier === "string" &&
      identifier.length > 0 &&
      (kind === "user" || kind === "query" || kind === "params")
    ) {
      return { kind, identifier };
    }
  }
  if (typeof params.sourceKey === "string" && params.sourceKey.length > 0) {
    return sourceKeyToSubscription(params.sourceKey);
  }
  return null;
}

async function poll(params: ReleasePollRequest, rpc: HostRpcClient): Promise<ReleasePollResponse> {
  const sourceId = params.sourceId;
  const subscription = resolveSubscription(params);
  if (subscription === null) {
    logger.warn(`source=${sourceId} no resolvable subscription on poll request; skipping`);
    return { notModified: false, upstreamStatus: 200 };
  }

  // 1. Pull tracked-series + aliases.
  const tracked = await fetchAllTracked(rpc, sourceId);
  if (tracked.length === 0) {
    logger.info(`no tracked series with aliases for source=${sourceId}`);
    return { notModified: false, upstreamStatus: 200 };
  }

  // 2. Conditional GET against this subscription's feed.
  const outcome = await pollSubscription(rpc, sourceId, subscription, tracked, {
    previousEtag: params.etag ?? null,
    timeoutMs: state.requestTimeoutMs,
    minConfidence: state.minConfidence,
    ...(state.baseUrl ? { baseUrl: state.baseUrl } : {}),
  });
  if (outcome.error) {
    logger.warn(
      `source=${sourceId} ${subscription.kind}:${subscription.identifier}: ${outcome.error} (status ${outcome.upstreamStatus})`,
    );
  }

  logger.info(
    `poll complete: source=${sourceId} subscription=${subscription.kind}:${subscription.identifier} tracked=${tracked.length} parsed=${outcome.parsed} matched=${outcome.matched} recorded=${outcome.recorded} deduped=${outcome.deduped} status=${outcome.upstreamStatus}${outcome.notModified ? " (304)" : ""}`,
  );

  // Report counters back to the host so it can build a meaningful
  // `last_summary` for the source. Without these, the host only sees the
  // (empty) `candidates` payload — we record via reverse-RPC mid-poll —
  // and the status badge reads "Fetched 0 items" even on a busy poll.
  return {
    notModified: outcome.notModified,
    upstreamStatus: outcome.upstreamStatus,
    parsed: outcome.parsed,
    matched: outcome.matched,
    recorded: outcome.recorded,
    deduped: outcome.deduped,
    ...(outcome.etag !== null ? { etag: outcome.etag } : {}),
  };
}

// =============================================================================
// Plugin Initialization
// =============================================================================

/**
 * Send the desired-state list of source rows to the host. Called from
 * `onInitialize` (after the host has installed the releases reverse-RPC
 * handler) so the plugin's source rows are materialized whenever the
 * config changes.
 *
 * Retries on `METHOD_NOT_FOUND` with linear backoff: the host installs the
 * releases handler shortly after `initialize` returns, and there is a small
 * race window where the plugin's first reverse-RPC call may land before the
 * handler is in place.
 */
export async function registerSources(
  rpc: HostRpcClient,
  subscriptions: UploaderSubscription[],
): Promise<{ registered: number; pruned: number } | null> {
  const sources = subscriptions.map((sub) => ({
    sourceKey: subscriptionToSourceKey(sub),
    displayName: displayNameFor(sub),
    kind: "rss-uploader" as const,
    config: { subscription: { kind: sub.kind, identifier: sub.identifier } },
  }));

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
        // Wait for the host to finish installing the releases reverse-RPC
        // handler. Linear backoff: 50ms, 100ms, 150ms, 200ms.
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

/** Human-readable label shown in the Release tracking settings table. */
function displayNameFor(sub: UploaderSubscription): string {
  if (sub.kind === "user") return `Nyaa: ${sub.identifier}`;
  if (sub.kind === "query") return `Nyaa search: ${sub.identifier}`;
  return `Nyaa params: ${sub.identifier}`;
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

    // Materialize source rows. Deferred to a microtask + retry on
    // METHOD_NOT_FOUND so we run *after* the host installs the releases
    // reverse-RPC handler (it does so right after `initialize` returns).
    queueMicrotask(() => {
      void registerSources(params.hostRpc, state.subscriptions).then((result) => {
        if (result) {
          logger.info(`register_sources: registered=${result.registered} pruned=${result.pruned}`);
        }
      });
    });
  },
});

logger.info("Nyaa release-source plugin started");
