/**
 * Tag name to numeric ID resolution.
 *
 * MangaBaka's `tag_not` parameter accepts tag IDs only: passing a name returns
 * HTTP 400 (`expected number, received NaN`). Nobody is going to look up that
 * "Death Game" is 1120, so the user-facing setting takes names and this resolves
 * them.
 *
 * The catalogue is large (roughly 2,700 entries) and comes from `/v1/tags`,
 * which unlike the recommendation endpoints is stable and edge-cached for an
 * hour. It is fetched at most once per process and cached in plugin storage
 * between runs.
 */

import type { PluginStorage } from "@ashdev/codex-plugin-sdk";
import type { MangaBakaRecommendationClient } from "./api.js";
import { logger } from "./logger.js";

/** Storage key holding the cached name-to-ID map. */
export const TAG_CACHE_KEY = "tag_catalogue";

/** How long a cached catalogue stays usable. Tags are added, rarely renamed. */
const TAG_CACHE_TTL_MS = 7 * 24 * 60 * 60 * 1000;

/** Normalise a tag name for lookup. */
function key(name: string): string {
  return name.trim().toLowerCase();
}

export class TagResolver {
  private catalogue: Map<string, number> | null = null;
  /**
   * Set once the catalogue has been looked up, successfully or not.
   *
   * Without it, a failed fetch would be retried on every resolve call, turning
   * one unavailable endpoint into repeated latency on every run.
   */
  private attempted = false;

  constructor(
    private readonly client: MangaBakaRecommendationClient,
    private readonly storage?: PluginStorage,
  ) {}

  /**
   * Resolve tag names to IDs, dropping any that cannot be resolved.
   *
   * Unresolvable names are logged and skipped rather than failing: a filter the
   * user typo'd should cost them that one exclusion, not their recommendations.
   */
  async resolve(names: string[]): Promise<number[]> {
    const wanted = names.map(key).filter((name) => name.length > 0);
    if (wanted.length === 0) return [];

    const catalogue = await this.load();
    const ids: number[] = [];
    const seen = new Set<number>();
    const unresolved: string[] = [];

    for (const name of wanted) {
      // A user who already knows the numeric ID should not be second-guessed.
      const asNumber = /^\d+$/.test(name) ? Number(name) : null;
      const id = asNumber ?? catalogue?.get(name);

      if (id === undefined || id === null) {
        unresolved.push(name);
        continue;
      }
      if (seen.has(id)) continue;

      seen.add(id);
      ids.push(id);
    }

    if (unresolved.length > 0) {
      logger.warn(
        `Ignoring ${unresolved.length} unrecognised tag name(s): ${unresolved.join(", ")}`,
      );
    }

    return ids;
  }

  /** Load the catalogue from memory, then storage, then upstream. */
  private async load(): Promise<Map<string, number> | null> {
    if (this.catalogue || this.attempted) return this.catalogue;
    this.attempted = true;

    const cached = await this.readCache();
    if (cached) {
      this.catalogue = cached;
      logger.debug(`Loaded ${cached.size} tags from cache`);
      return this.catalogue;
    }

    try {
      const tags = await this.client.tags();
      const catalogue = new Map<string, number>();
      for (const tag of tags) {
        // Merged tags still resolve, pointing at whatever they merged into.
        catalogue.set(key(tag.name), tag.merged_with ?? tag.id);
      }

      this.catalogue = catalogue;
      logger.debug(`Fetched ${catalogue.size} tags from MangaBaka`);
      await this.writeCache(catalogue);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      logger.warn(`Could not fetch the tag catalogue, tag filters will be ignored: ${message}`);
    }

    return this.catalogue;
  }

  private async readCache(): Promise<Map<string, number> | null> {
    if (!this.storage) return null;

    try {
      const result = await this.storage.get(TAG_CACHE_KEY);
      const data = result?.data;
      // Arrays are objects too, so the Array check matters: a stale cache in the
      // wrong shape must fall through to a fetch rather than yield an empty map.
      if (!data || typeof data !== "object" || Array.isArray(data)) return null;

      const catalogue = new Map<string, number>();
      for (const [name, id] of Object.entries(data as Record<string, unknown>)) {
        if (typeof id === "number") catalogue.set(name, id);
      }

      return catalogue.size > 0 ? catalogue : null;
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      logger.warn(`Failed to read the cached tag catalogue: ${message}`);
      return null;
    }
  }

  private async writeCache(catalogue: Map<string, number>): Promise<void> {
    if (!this.storage) return;

    try {
      const expiresAt = new Date(Date.now() + TAG_CACHE_TTL_MS).toISOString();
      await this.storage.set(TAG_CACHE_KEY, Object.fromEntries(catalogue), expiresAt);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      // The in-memory copy still serves this run.
      logger.warn(`Failed to cache the tag catalogue: ${message}`);
    }
  }
}
