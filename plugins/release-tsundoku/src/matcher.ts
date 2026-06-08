/**
 * Exact external-ID matching between the Tsundoku feed and Codex's tracked
 * series.
 *
 * Codex's `releases/record` path keys on a `codexSeriesId` (UUID), not on
 * external IDs — so matching happens here, plugin-side, with zero fuzzy
 * logic. The host returns each tracked series' provider IDs via
 * `releases/list_tracked` (scoped by the manifest's `requiresExternalIds`,
 * prefix-stripped to bare provider names). We index those into
 * `"provider:id" -> codexSeriesId`, then resolve each feed item by looking
 * up its own provider IDs in priority order. The first provider that hits
 * wins; the match is exact, so the candidate's confidence is always 1.0.
 */

import type { TrackedSeriesEntry } from "@ashdev/codex-plugin-sdk";
import type { FeedItem } from "./fetcher.js";
import { TSUNDOKU_EXTERNAL_ID_SOURCES } from "./manifest.js";

/** Result of resolving a feed item to a tracked Codex series. */
export interface MatchResult {
  /** The Codex series UUID the candidate should be recorded against. */
  codexSeriesId: string;
  /** The provider whose ID produced the match (for the candidate `reason`). */
  provider: string;
  /** The external ID value that matched. */
  externalId: string;
}

/** Compose the index key for a `(provider, externalId)` pair. */
function indexKey(provider: string, externalId: string): string {
  return `${provider}:${externalId}`;
}

/**
 * Build a reverse index `"provider:id" -> codexSeriesId` from the host's
 * tracked-series rows. Entries without external IDs contribute nothing.
 *
 * If two tracked series somehow share the same `(provider, id)` (shouldn't
 * happen — provider IDs are unique per series), the later entry wins. That's
 * an arbitrary-but-deterministic tie-break for a degenerate input.
 */
export function buildIndex(entries: TrackedSeriesEntry[]): Map<string, string> {
  const index = new Map<string, string>();
  for (const entry of entries) {
    const ids = entry.externalIds;
    if (!ids) continue;
    for (const [provider, externalId] of Object.entries(ids)) {
      if (!externalId) continue;
      index.set(indexKey(provider, externalId), entry.seriesId);
    }
  }
  return index;
}

/**
 * Resolve a feed item against the reverse index. Providers are tried in the
 * manifest's declared priority order (`TSUNDOKU_EXTERNAL_ID_SOURCES`, most
 * canonical first) so the `reason` is stable when an item carries several
 * matchable IDs. Returns `null` when no provider ID hits the index — the
 * common case, since the feed spans the whole Tsundoku catalog and the user
 * only tracks a slice of it.
 */
export function matchItem(item: FeedItem, index: Map<string, string>): MatchResult | null {
  // Collapse the item's external-ID array into a provider -> id lookup so the
  // priority sweep below is O(providers) rather than O(providers * ids).
  const byProvider = new Map<string, string>();
  for (const ext of item.externalIds) {
    if (ext.externalId) {
      byProvider.set(ext.provider, ext.externalId);
    }
  }

  for (const provider of TSUNDOKU_EXTERNAL_ID_SOURCES) {
    const externalId = byProvider.get(provider);
    if (externalId === undefined) continue;
    const codexSeriesId = index.get(indexKey(provider, externalId));
    if (codexSeriesId !== undefined) {
      return { codexSeriesId, provider, externalId };
    }
  }
  return null;
}
