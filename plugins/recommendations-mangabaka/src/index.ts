/**
 * MangaBaka Recommendations Plugin for Codex
 *
 * Generates manga recommendations from the user's library by combining two
 * signals from MangaBaka's public API:
 *
 * 1. Content similarity, via a single `/v1/series/mix` call that folds every
 *    seed's tag vector into one probe.
 * 2. Collaborative filtering, via `/v1/series/{id}/readers-also-like` on the
 *    highest-rated seeds.
 *
 * Neither endpoint requires authentication, so the plugin works with no account
 * and no API key.
 *
 * Communicates via JSON-RPC over stdio using the Codex plugin SDK.
 */

import {
  createRecommendationPlugin,
  type InitializeParams,
  type PluginStorage,
  type RecommendationProvider,
  type RecommendationRequest,
  type RecommendationResponse,
} from "@ashdev/codex-plugin-sdk";
import { MangaBakaRecommendationClient } from "./api.js";
import { logger } from "./logger.js";
import { manifest } from "./manifest.js";

// Plugin state (set during initialization)
let client: MangaBakaRecommendationClient | null = null;
let storage: PluginStorage | null = null;

/** Set the MangaBaka client (exported for testing) */
export function setClient(c: MangaBakaRecommendationClient | null): void {
  client = c;
}

/** Access the captured storage handle (exported for testing) */
export function getStorage(): PluginStorage | null {
  return storage;
}

const provider: RecommendationProvider = {
  async get(params: RecommendationRequest): Promise<RecommendationResponse> {
    if (!client) {
      throw new Error("Plugin not initialized - no MangaBaka client");
    }

    // Seed resolution, the mix and readers-also-like calls, filtering, and
    // scoring are not implemented yet. Returning an empty set keeps the
    // JSON-RPC contract honest in the meantime rather than emitting
    // placeholder recommendations that would look real to the host.
    logger.warn(
      `Recommendation generation not yet implemented; ignoring ${params.library?.length ?? 0} seeds`,
    );

    return { recommendations: [], generatedAt: new Date().toISOString(), cached: false };
  },
};

createRecommendationPlugin({
  manifest,
  provider,
  logLevel: "info",
  async onInitialize(params: InitializeParams) {
    // Honor the host-supplied log level (Codex `plugins.log_level` config).
    if (params.logLevel) logger.setLevel(params.logLevel);

    client = new MangaBakaRecommendationClient();
    storage = params.storage;

    logger.info(`MangaBaka recommendations client ready (${client.baseUrl})`);
  },
});

logger.info("MangaBaka recommendations plugin started");
