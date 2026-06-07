/**
 * Echo Recommendations Plugin for Codex
 *
 * A minimal test/debug recommendations plugin. It does not talk to any external
 * service: it echoes the user's library seeds back as recommendations (and, when
 * the library is empty, returns a few generic ones). Every recommendation has
 * all fields populated so the host's ingest path is exercised end to end.
 *
 * Its main purpose is debugging the host -> plugin recommendations protocol: it
 * records every request and its response to JSON files under the plugin's data
 * directory (see `recorder.ts`).
 */

import {
  createLogger,
  createRecommendationPlugin,
  type InitializeParams,
  type ProfileUpdateRequest,
  type ProfileUpdateResponse,
  type Recommendation,
  type RecommendationClearResponse,
  type RecommendationDismissRequest,
  type RecommendationDismissResponse,
  type RecommendationProvider,
  type RecommendationRequest,
  type RecommendationResponse,
} from "@ashdev/codex-plugin-sdk";
import { DEFAULT_FALLBACK_COUNT, DEFAULT_MAX_PAYLOAD_FILES, manifest } from "./manifest.js";
import { PayloadRecorder, redactConfig } from "./recorder.js";

const logger = createLogger({ name: "recommendations-echo", level: "debug" });

// Payload recorder (set during initialization)
let recorder: PayloadRecorder | null = null;

/** Record a request/response pair (best-effort) and return the response. */
async function rec<T>(method: string, params: unknown, response: T): Promise<T> {
  await recorder?.record(method, params, response);
  return response;
}

/** Set the payload recorder (exported for testing) */
export function setRecorder(r: PayloadRecorder | null): void {
  recorder = r;
}

/**
 * Build a deterministic, fully-populated recommendation seeded by the given
 * title. Every optional field is set so the host's ingest path is exercised.
 */
function makeRec(basedOnTitle: string, i: number): Recommendation {
  const externalId = `echo-rec-${i + 1}`;
  return {
    externalId,
    externalUrl: `https://echo.example.com/rec/${externalId}`,
    title: `Echo Recommendation ${i + 1}`,
    coverUrl: "https://picsum.photos/300/450",
    summary: `Echo recommendation based on "${basedOnTitle}".`,
    genres: ["Action", "Echo"],
    tags: [
      { name: "echo", rank: 90, category: "Theme" },
      { name: "test", rank: 50, category: "Demographic" },
    ],
    score: Math.max(0.1, 1 - i * 0.1),
    reason: `Recommended because you read "${basedOnTitle}"`,
    basedOn: [basedOnTitle],
    inLibrary: false,
    status: "ongoing",
    format: "MANGA",
    countryOfOrigin: "JP",
    startYear: 2020 + (i % 5),
    totalVolumeCount: 12,
    totalChapterCount: 100,
    rating: 80 + (i % 20),
    popularity: 1000 + i,
  };
}

/** Exported for testing */
export const provider: RecommendationProvider = {
  async get(params: RecommendationRequest): Promise<RecommendationResponse> {
    const { library, limit, excludeIds = [] } = params;
    const exclude = new Set(excludeIds);

    logger.info(
      `Recommendations requested: ${library.length} seeds, limit=${limit ?? "none"}, exclude=${excludeIds.length}`,
    );

    // Echo each library seed back as a recommendation; if the library is empty,
    // return a few generic ones so the response is never empty.
    const seeds =
      library.length > 0
        ? library.map((e) => e.title)
        : Array.from({ length: DEFAULT_FALLBACK_COUNT }, (_, i) => `Echo Seed ${i + 1}`);

    const recommendations = seeds
      .map((title, i) => makeRec(title, i))
      .filter((r) => !exclude.has(r.externalId))
      .slice(0, limit ?? seeds.length);

    return rec<RecommendationResponse>("recommendations/get", params, {
      recommendations,
      generatedAt: new Date().toISOString(),
      cached: false,
    });
  },

  async updateProfile(params: ProfileUpdateRequest): Promise<ProfileUpdateResponse> {
    logger.info(`Profile update: ${params.entries.length} entries`);
    return rec<ProfileUpdateResponse>("recommendations/updateProfile", params, {
      updated: true,
      entriesProcessed: params.entries.length,
    });
  },

  async dismiss(params: RecommendationDismissRequest): Promise<RecommendationDismissResponse> {
    logger.info(`Dismiss ${params.externalId} (reason: ${params.reason ?? "none"})`);
    return rec<RecommendationDismissResponse>("recommendations/dismiss", params, {
      dismissed: true,
    });
  },

  async clear(): Promise<RecommendationClearResponse> {
    logger.info("Clear recommendations");
    return rec<RecommendationClearResponse>("recommendations/clear", null, { cleared: true });
  },
};

// =============================================================================
// Plugin Initialization
// =============================================================================

createRecommendationPlugin({
  manifest,
  provider,
  logLevel: "debug",
  onInitialize(params: InitializeParams) {
    // Set up payload recording (on by default for this debug plugin)
    const recordPayloads = params.adminConfig?.recordPayloads !== false;
    const maxPayloadFiles =
      typeof params.adminConfig?.maxPayloadFiles === "number"
        ? params.adminConfig.maxPayloadFiles
        : DEFAULT_MAX_PAYLOAD_FILES;
    recorder = new PayloadRecorder({
      pluginName: manifest.name,
      dataDir: params.dataDir,
      enabled: recordPayloads,
      maxFiles: maxPayloadFiles,
      configSnapshot: redactConfig({
        adminConfig: params.adminConfig,
        userConfig: params.userConfig,
      }),
      logger,
    });

    logger.info(`Echo recommendations plugin initialized (recordPayloads: ${recordPayloads})`);
  },
});

logger.info("Echo recommendations plugin started");
