/**
 * MangaBaka Plugin - Fetch manga metadata from MangaBaka
 *
 * MangaBaka aggregates metadata from multiple sources (AniList, MAL, MangaDex, etc.)
 * and provides a unified API for manga/novel metadata.
 *
 * API docs: https://mangabaka.org/api
 *
 * Credentials are provided by Codex via the initialize message.
 * Required credential: api_key (get one at https://mangabaka.org/settings/api)
 */

import {
  ConfigError,
  createLogger,
  createMetadataPlugin,
  type InitializeParams,
  type MetadataProvider,
} from "@ashdev/codex-plugin-sdk";
import { MangaBakaClient } from "./api.js";
import { handleGet } from "./handlers/get.js";
import { handleMatch } from "./handlers/match.js";
import { handleSearch } from "./handlers/search.js";
import { manifest } from "./manifest.js";

const logger = createLogger({ name: "mangabaka", level: "info" });

// Client is initialized when we receive credentials from Codex
let client: MangaBakaClient | null = null;

function getClient(): MangaBakaClient {
  if (!client) {
    throw new ConfigError("Plugin not initialized - missing API key");
  }
  return client;
}

// Create the MetadataProvider implementation
const provider: MetadataProvider = {
  async search(params) {
    return handleSearch(params, getClient());
  },

  async get(params) {
    return handleGet(params, getClient());
  },

  async match(params) {
    return handleMatch(params, getClient());
  },
};

// Start the plugin server
createMetadataPlugin({
  manifest,
  provider,
  logLevel: "info",
  onInitialize(params: InitializeParams) {
    const apiKey = params.credentials?.api_key;
    if (!apiKey) {
      throw new ConfigError("api_key credential is required");
    }
    client = new MangaBakaClient(apiKey);
    logger.info("MangaBaka client initialized");
  },
});

logger.info("MangaBaka plugin started");
