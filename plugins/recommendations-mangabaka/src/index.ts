/**
 * MangaBaka Recommendations Plugin for Codex
 *
 * Generates manga recommendations from the user's library by combining two
 * signals from MangaBaka's public API:
 *
 * 1. Content similarity, via a single `/v1/series/mix` call that folds every
 *    seed's tag vector into one probe.
 * 2. Collaborative filtering, via `/v1/series/{id}/readers-also-like` on the
 *    highest-rated seeds. (Not wired up yet.)
 *
 * Neither endpoint requires authentication, so the plugin works with no account
 * and no API key.
 *
 * This module is deliberately thin. Importing it starts a stdio JSON-RPC
 * server, so the logic it delegates to lives in sibling modules that can be
 * tested without that side effect.
 *
 * Communicates via JSON-RPC over stdio using the Codex plugin SDK.
 */

import {
  createRecommendationPlugin,
  type InitializeParams,
  type RecommendationClearResponse,
  type RecommendationDismissRequest,
  type RecommendationDismissResponse,
  type RecommendationProvider,
  type RecommendationRequest,
  type RecommendationResponse,
} from "@ashdev/codex-plugin-sdk";
import { MangaBakaRecommendationClient } from "./api.js";
import {
  describeSettings,
  parseAdminConfig,
  parseUserConfig,
  type UserSettings,
} from "./config.js";
import { DismissalStore } from "./dismissals.js";
import { logger } from "./logger.js";
import { manifest } from "./manifest.js";
import { type GenerateOptions, generateRecommendations } from "./recommend.js";
import { TagResolver } from "./tags.js";

// Plugin state (set during initialization)
let client: MangaBakaRecommendationClient | null = null;
let tagResolver: TagResolver | null = null;
let settings: UserSettings = parseUserConfig(undefined);
const dismissals = new DismissalStore();

/** Set the MangaBaka client (exported for testing) */
export function setClient(c: MangaBakaRecommendationClient | null): void {
  client = c;
}

/**
 * Turn the parsed settings into generation options, resolving tag names to the
 * numeric IDs `tag_not` requires.
 *
 * Resolution is deferred to the first run rather than done during
 * initialization so that startup involves no network I/O: a slow or
 * unreachable tag catalogue would otherwise delay the plugin coming up, for a
 * filter that most users never set. The resolver caches in memory and in
 * storage, so this costs at most one request per process.
 */
async function buildOptions(): Promise<GenerateOptions> {
  const excludedTagIds =
    settings.excludedTagNames.length > 0 && tagResolver
      ? await tagResolver.resolve(settings.excludedTagNames)
      : [];

  return {
    contentWeight: settings.contentWeight,
    collaborativeSeeds: settings.collaborativeSeeds,
    excludeSameAuthor: settings.excludeSameAuthor,
    filters: {
      contentRating: settings.contentRating.length > 0 ? settings.contentRating : undefined,
      includedTypes: settings.includedTypes.length > 0 ? settings.includedTypes : undefined,
      excludedTypes: settings.excludedTypes.length > 0 ? settings.excludedTypes : undefined,
      excludedGenres: settings.excludedGenres.length > 0 ? settings.excludedGenres : undefined,
      excludedTagIds: excludedTagIds.length > 0 ? excludedTagIds : undefined,
      minimumRating: settings.minimumRating,
    },
  };
}

const provider: RecommendationProvider = {
  async get(params: RecommendationRequest): Promise<RecommendationResponse> {
    if (!client) {
      throw new Error("Plugin not initialized - no MangaBaka client");
    }

    return generateRecommendations(client, params, dismissals, await buildOptions());
  },

  async dismiss(params: RecommendationDismissRequest): Promise<RecommendationDismissResponse> {
    await dismissals.add(params.externalId);
    logger.debug(
      `Dismissed recommendation: ${params.externalId} (reason: ${params.reason ?? "none"})`,
    );
    return { dismissed: true };
  },

  async clear(): Promise<RecommendationClearResponse> {
    const count = await dismissals.clear();
    logger.info(`Cleared ${count} dismissed recommendations`);
    return { cleared: true };
  },
};

createRecommendationPlugin({
  manifest,
  provider,
  logLevel: "info",
  async onInitialize(params: InitializeParams) {
    // Honor the host-supplied log level (Codex `plugins.log_level` config).
    if (params.logLevel) logger.setLevel(params.logLevel);

    client = new MangaBakaRecommendationClient(parseAdminConfig(params.adminConfig));
    tagResolver = new TagResolver(client, params.storage);
    settings = parseUserConfig(params.userConfig);

    await dismissals.hydrate(params.storage);

    logger.info(
      `MangaBaka recommendations ready (${client.baseUrl}, ${dismissals.size} dismissed)`,
    );
    logger.info(`Settings: ${describeSettings(settings)}`);
  },
});

logger.info("MangaBaka recommendations plugin started");
