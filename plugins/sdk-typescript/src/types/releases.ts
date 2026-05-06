/**
 * Release-source protocol types - MUST match the Rust protocol exactly.
 *
 * Plugins implementing the `release_source` capability poll external sources
 * for new chapter/volume releases and emit `ReleaseCandidate` rows. The host
 * threshold-gates and dedups them through the `release_ledger` table.
 *
 * @see src/services/plugin/protocol.rs (`ReleasePollRequest`, `ReleasePollResponse`)
 * @see src/services/release/candidate.rs (`ReleaseCandidate`, `SeriesMatch`)
 * @see src/services/plugin/releases_handler.rs (reverse-RPC handlers)
 */

// =============================================================================
// Reverse-RPC method names (plugin -> host)
// =============================================================================

/**
 * Method names for the `releases/*` reverse-RPC namespace. Plugins call these
 * over the open RPC channel during `releases/poll` (or any other time).
 */
export const RELEASES_METHODS = {
  /** List tracked series, scoped to what the plugin's manifest declared. */
  LIST_TRACKED: "releases/list_tracked",
  /** Submit a candidate to the host's release ledger. */
  RECORD: "releases/record",
  /** Get persisted per-source state (etag, last_polled_at, last_error). */
  SOURCE_STATE_GET: "releases/source_state/get",
  /** Set persisted per-source state (etag only — other fields are host-owned). */
  SOURCE_STATE_SET: "releases/source_state/set",
  /**
   * Replace the set of `release_sources` rows owned by this plugin.
   *
   * Plugins call this from `onInitialize` (and after any config change, which
   * triggers a process restart that re-runs `onInitialize`). Each call carries
   * the plugin's full desired-state list; the host upserts every entry on
   * `(plugin_id, source_key)` and prunes rows whose `source_key` is not in
   * the request. User-managed fields (`enabled`, `pollIntervalS`) are
   * preserved across re-registrations so an admin's overrides aren't
   * trampled by a plugin restart.
   */
  REGISTER_SOURCES: "releases/register_sources",
} as const;

// =============================================================================
// ReleaseCandidate (the wire shape plugins emit)
// =============================================================================

/**
 * Per-series match metadata attached to every candidate.
 *
 * - `codexSeriesId` is the host's UUID for the series. Plugins resolve this
 *   from `releases/list_tracked` (don't invent series IDs).
 * - `confidence` (0.0..=1.0) tells the host how sure the plugin is about the
 *   match. The host drops below-threshold candidates (default 0.7).
 * - `reason` is a short opaque string used for debugging/UI, e.g.
 *   `"mangaupdates_id"`, `"alias-exact"`, `"alias-fuzzy"`.
 */
export interface SeriesMatch {
  codexSeriesId: string;
  confidence: number;
  reason: string;
}

/**
 * Release candidate emitted by a plugin.
 *
 * **Field semantics:**
 * - `externalReleaseId`: Stable per-source ID. The first dedup key.
 *   `(sourceId, externalReleaseId)` is `UNIQUE` in `release_ledger`.
 * - `chapter` / `volume`: At least one should be set; both is fine for a
 *   "vol 15 covers ch 126-142" case (the volume axis advances; the chapter
 *   axis advances to the volume's last chapter only if the candidate
 *   carries it). Decimals supported on `chapter` (e.g. 47.5).
 * - `language`: ISO 639-1 code, lowercase. Must be non-empty. The host's
 *   `latest_known_*` advance gate uses this against the per-series
 *   effective language list.
 * - `groupOrUploader`: Scanlation group (MangaUpdates) or torrent uploader
 *   handle (Nyaa). Optional but strongly recommended.
 * - `payloadUrl`: The link the user follows to actually consume / acquire
 *   the release. Must be non-empty. Conventionally a human-readable landing
 *   page (Nyaa view page, MangaUpdates release page).
 * - `mediaUrl` / `mediaUrlKind`: Optional second URL describing how to
 *   actually fetch the bits — a `.torrent` file, a magnet link, or a direct
 *   download. Set both together; leave both unset for sources that only
 *   surface a landing page. The UI renders a kind-specific icon next to
 *   the standard external-link icon when these are present.
 * - `infoHash`: Torrent info_hash if applicable. Cross-source dedup key.
 * - `metadata` / `formatHints`: Free-form JSON for plugin-specific data
 *   (Nyaa size in bytes, MangaUpdates "is volume bundle" flag, etc.).
 * - `observedAt`: When the plugin saw this entry. Used for ordering;
 *   bounded by `MAX_FUTURE_SKEW_S` (1h) on the host side.
 */
export interface ReleaseCandidate {
  seriesMatch: SeriesMatch;
  externalReleaseId: string;
  chapter?: number | null;
  volume?: number | null;
  language: string;
  formatHints?: Record<string, unknown> | null;
  groupOrUploader?: string | null;
  payloadUrl: string;
  mediaUrl?: string | null;
  mediaUrlKind?: MediaUrlKind | null;
  infoHash?: string | null;
  metadata?: Record<string, unknown> | null;
  /** ISO-8601 timestamp. */
  observedAt: string;
}

/**
 * Classifies what `mediaUrl` points at so the UI can pick an appropriate
 * icon and the host can label it consistently across sources.
 *
 * - `torrent`: HTTP(S) URL to a `.torrent` file.
 * - `magnet`:  `magnet:` URI.
 * - `direct`:  HTTP(S) URL to the file itself (DDL host, CDN, etc.).
 * - `other`:   anything else; render a generic download icon.
 */
export type MediaUrlKind = "torrent" | "magnet" | "direct" | "other";

// =============================================================================
// releases/list_tracked
// =============================================================================

export interface ListTrackedRequest {
  sourceId: string;
  limit?: number;
  offset?: number;
}

/**
 * One tracked-series row scoped to what the plugin's manifest asked for.
 * Aliases are present only when `requiresAliases: true`; external IDs are
 * present only for sources the plugin listed in `requiresExternalIds`.
 */
export interface TrackedSeriesEntry {
  seriesId: string;
  aliases?: string[];
  /** Map keyed by external-ID source name (e.g. `{ mangaupdates: "12345" }`). */
  externalIds?: Record<string, string>;
  latestKnownChapter?: number | null;
  latestKnownVolume?: number | null;
}

export interface ListTrackedResponse {
  tracked: TrackedSeriesEntry[];
  nextOffset?: number;
}

// =============================================================================
// releases/record
// =============================================================================

export interface RecordRequest {
  sourceId: string;
  candidate: ReleaseCandidate;
}

export interface RecordResponse {
  ledgerId: string;
  /** True if the row deduped onto an existing ledger entry. */
  deduped: boolean;
}

// =============================================================================
// releases/source_state
// =============================================================================

export interface SourceStateGetRequest {
  sourceId: string;
}

export interface SourceStateView {
  etag?: string;
  lastPolledAt?: string;
  lastError?: string;
  lastErrorAt?: string;
}

export interface SourceStateSetRequest {
  sourceId: string;
  /** Only `etag` is plugin-writable; other fields are host-owned. */
  etag?: string;
}

// =============================================================================
// releases/register_sources
// =============================================================================

/**
 * One source the plugin wants the host to materialize as a `release_sources`
 * row. The plugin owns the `sourceKey` namespace; the host treats it as an
 * opaque string for dedup keyed on `(pluginId, sourceKey)`.
 */
export interface RegisteredSourceInput {
  /**
   * Stable per-plugin identifier. Reuse the same key across calls so user
   * overrides (enabled, pollIntervalS) survive plugin restarts.
   */
  sourceKey: string;
  /** Human-readable label shown in the Release tracking settings UI. */
  displayName: string;
  /**
   * Must be one of the kinds the plugin declared in its
   * `releaseSource.kinds` capability — the host rejects anything else.
   */
  kind: "rss-uploader" | "rss-series" | "api-feed" | "metadata-feed";
  /**
   * Optional opaque per-source config snapshot persisted on the row. The
   * host doesn't interpret it; the plugin reads its own admin config
   * directly. Useful for surfacing "what did this source originate from?"
   * in the UI / logs.
   */
  config?: Record<string, unknown> | null;
}

export interface RegisterSourcesRequest {
  sources: RegisteredSourceInput[];
}

export interface RegisterSourcesResponse {
  /** Number of sources upserted (created or refreshed). */
  registered: number;
  /** Number of sources removed because they were not in the request. */
  pruned: number;
}

// =============================================================================
// releases/poll (host -> plugin)
// =============================================================================

/**
 * Parameters for the host's call into a release-source plugin's
 * `releases/poll` handler. Carries the source row to poll plus any ETag the
 * plugin recorded on its previous poll, plus the plugin-defined source key
 * and per-source config snapshot so the plugin can dispatch directly without
 * a reverse-RPC roundtrip.
 */
export interface ReleasePollRequest {
  sourceId: string;
  /**
   * The same `sourceKey` the plugin passed to `releases/register_sources`.
   * Useful when one plugin process owns multiple source rows (e.g., one per
   * Nyaa uploader) and needs to know which one to poll.
   */
  sourceKey?: string;
  /**
   * Snapshot of `release_sources.config` for this row. Plugins that stash
   * per-source config on register can read it back here.
   */
  config?: Record<string, unknown> | null;
  etag?: string;
}

/**
 * Response from a `releases/poll` call.
 *
 * Plugins may also stream candidates over `releases/record` mid-poll; the
 * host treats both styles identically. Use `candidates` for plugins that
 * prefer to return everything at once.
 *
 * Plugins that stream via `releases/record` should also populate the
 * counter fields (`parsed`, `matched`, `recorded`, `deduped`). Without
 * them, the host can only see what came back in `candidates` and the
 * source's status badge will read "Fetched 0 items" no matter what
 * actually happened.
 */
export interface ReleasePollResponse {
  /** Optional batch of candidates the host should evaluate and ledger. */
  candidates?: ReleaseCandidate[];
  /** New ETag observed (e.g., from the upstream feed's `ETag` header). */
  etag?: string;
  /** Whether the upstream returned `304 Not Modified` (or equivalent). */
  notModified?: boolean;
  /** HTTP status code observed (used by host's per-host backoff). */
  upstreamStatus?: number;
  /**
   * Items the plugin parsed from the upstream feed before any matching
   * or threshold filtering. Streaming plugins should set this so the
   * host's `last_summary` reflects upstream activity, not just the shape
   * of the response payload.
   */
  parsed?: number;
  /**
   * Of those parsed, the count that matched a tracked-series alias (i.e.
   * became candidates the plugin then evaluated/streamed).
   */
  matched?: number;
  /**
   * Of those matched, the count actually inserted into the ledger
   * (excludes dedupes). For plugins that stream via `releases/record`,
   * this is the count of non-deduped record outcomes.
   */
  recorded?: number;
  /**
   * Of those matched, the count the host deduped onto an existing ledger
   * row. Optional; when omitted the host infers `matched - recorded`.
   */
  deduped?: number;
}
