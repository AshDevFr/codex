/**
 * Persisted per-user dismissals.
 *
 * The host has no dismissal state of its own for recommendation providers: it
 * forwards `recommendations/dismiss` to the plugin and expects the plugin to
 * stop returning that entry. State lives in `PluginStorage`, which is already
 * scoped per user and per plugin, mirrored in memory so filtering stays
 * synchronous.
 *
 * Storage failures are logged and swallowed throughout. A lost dismissal makes
 * one unwanted recommendation reappear; a thrown error fails the user's entire
 * recommendation run.
 */

import type { PluginStorage } from "@ashdev/codex-plugin-sdk";
import { logger } from "./logger.js";

/** Storage key holding the array of dismissed external IDs. */
export const DISMISSED_STORAGE_KEY = "dismissed_ids";

export class DismissalStore {
  private ids = new Set<string>();
  private storage: PluginStorage | null = null;

  /** Number of dismissals currently held. */
  get size(): number {
    return this.ids.size;
  }

  /** Whether this external ID has been dismissed. */
  has(externalId: string): boolean {
    return this.ids.has(externalId);
  }

  /**
   * Attach storage and load persisted dismissals, replacing any in-memory
   * state. Called once during plugin initialization.
   */
  async hydrate(storage: PluginStorage): Promise<void> {
    this.storage = storage;
    this.ids = new Set();

    try {
      const result = await storage.get(DISMISSED_STORAGE_KEY);
      if (!Array.isArray(result?.data)) {
        // Absent (never dismissed anything) or unrecognised. Either way there
        // is nothing to restore.
        return;
      }

      for (const id of result.data) {
        if (typeof id === "string" && id.length > 0) {
          this.ids.add(id);
        }
      }
      logger.debug(`Loaded ${this.ids.size} dismissed IDs from storage`);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      logger.warn(`Failed to load dismissed IDs from storage: ${message}`);
    }
  }

  /** Record a dismissal and persist it. */
  async add(externalId: string): Promise<void> {
    this.ids.add(externalId);
    await this.persist();
  }

  /** Drop every dismissal, returning how many were removed. */
  async clear(): Promise<number> {
    const count = this.ids.size;
    this.ids.clear();
    await this.persist();
    return count;
  }

  private async persist(): Promise<void> {
    if (!this.storage) return;

    try {
      await this.storage.set(DISMISSED_STORAGE_KEY, [...this.ids]);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      // The in-memory set is deliberately left as-is: the dismissal should
      // still hold for the rest of this process even if it cannot be saved.
      logger.warn(`Failed to save dismissed IDs to storage: ${message}`);
    }
  }
}
