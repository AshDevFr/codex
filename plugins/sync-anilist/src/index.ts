/**
 * AniList Sync Plugin for Codex
 *
 * Syncs manga reading progress between Codex and AniList.
 * Communicates via JSON-RPC over stdio using the Codex plugin SDK.
 *
 * Capabilities:
 * - Push reading progress from Codex to AniList
 * - Pull reading progress from AniList to Codex
 * - Get user info from AniList
 * - Status reporting for sync state
 */

import {
  createLogger,
  createSyncPlugin,
  type ExternalUserInfo,
  type InitializeParams,
  type SyncEntry,
  type SyncEntryResult,
  type SyncProvider,
  type SyncPullRequest,
  type SyncPullResponse,
  type SyncPushRequest,
  type SyncPushResponse,
  type SyncStatusResponse,
} from "@ashdev/codex-plugin-sdk";
import {
  AniListClient,
  type AniListFuzzyDate,
  anilistStatusToSync,
  convertScoreFromAnilist,
  convertScoreToAnilist,
  fuzzyDateToIso,
  isoToFuzzyDate,
  syncStatusToAnilist,
} from "./anilist.js";
import { manifest } from "./manifest.js";

const logger = createLogger({ name: "sync-anilist", level: "debug" });

// Plugin state (set during initialization)
let client: AniListClient | null = null;
let viewerId: number | null = null;
let scoreFormat = "POINT_10";

// =============================================================================
// Sync Provider Implementation
// =============================================================================

const provider: SyncProvider = {
  async getUserInfo(): Promise<ExternalUserInfo> {
    if (!client) {
      throw new Error("Plugin not initialized - no AniList client");
    }

    const viewer = await client.getViewer();
    viewerId = viewer.id;
    scoreFormat = viewer.mediaListOptions.scoreFormat;

    logger.info(`Authenticated as ${viewer.name} (id: ${viewer.id}, scoreFormat: ${scoreFormat})`);

    return {
      externalId: String(viewer.id),
      username: viewer.name,
      avatarUrl: viewer.avatar.large || viewer.avatar.medium,
      profileUrl: viewer.siteUrl,
    };
  },

  async pushProgress(params: SyncPushRequest): Promise<SyncPushResponse> {
    if (!client) {
      throw new Error("Plugin not initialized - no AniList client");
    }

    const success: SyncEntryResult[] = [];
    const failed: SyncEntryResult[] = [];

    for (const entry of params.entries) {
      try {
        const mediaId = Number.parseInt(entry.externalId, 10);
        if (Number.isNaN(mediaId)) {
          failed.push({
            externalId: entry.externalId,
            status: "failed",
            error: `Invalid media ID: ${entry.externalId}`,
          });
          continue;
        }

        const saveParams: {
          mediaId: number;
          status?: string;
          score?: number;
          progress?: number;
          progressVolumes?: number;
          startedAt?: AniListFuzzyDate;
          completedAt?: AniListFuzzyDate;
          notes?: string;
        } = {
          mediaId,
          status: syncStatusToAnilist(entry.status),
        };

        // Map progress
        if (entry.progress?.chapters !== undefined) {
          saveParams.progress = entry.progress.chapters;
        }
        if (entry.progress?.volumes !== undefined) {
          saveParams.progressVolumes = entry.progress.volumes;
        }

        // Map score (convert from 1-100 scale to AniList format)
        if (entry.score !== undefined) {
          saveParams.score = convertScoreToAnilist(entry.score, scoreFormat);
        }

        // Map dates
        if (entry.startedAt) {
          saveParams.startedAt = isoToFuzzyDate(entry.startedAt);
        }
        if (entry.completedAt) {
          saveParams.completedAt = isoToFuzzyDate(entry.completedAt);
        }

        // Map notes
        if (entry.notes !== undefined) {
          saveParams.notes = entry.notes;
        }

        const result = await client.saveEntry(saveParams);
        logger.debug(`Pushed entry ${entry.externalId}: status=${result.status}`);

        success.push({
          externalId: entry.externalId,
          status: "updated",
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : "Unknown error";
        logger.error(`Failed to push entry ${entry.externalId}: ${message}`);
        failed.push({
          externalId: entry.externalId,
          status: "failed",
          error: message,
        });
      }
    }

    return { success, failed };
  },

  async pullProgress(params: SyncPullRequest): Promise<SyncPullResponse> {
    if (!client || viewerId === null) {
      throw new Error("Plugin not initialized - call getUserInfo first");
    }

    // Parse pagination cursor (page number)
    const page = params.cursor ? Number.parseInt(params.cursor, 10) : 1;
    const perPage = params.limit ? Math.min(params.limit, 50) : 50;

    const result = await client.getMangaList(viewerId, page, perPage);

    const entries: SyncEntry[] = result.entries.map((entry) => ({
      externalId: String(entry.mediaId),
      status: anilistStatusToSync(entry.status),
      progress: {
        chapters: entry.progress || undefined,
        volumes: entry.progressVolumes || undefined,
      },
      score: entry.score > 0 ? convertScoreFromAnilist(entry.score, scoreFormat) : undefined,
      startedAt: fuzzyDateToIso(entry.startedAt),
      completedAt: fuzzyDateToIso(entry.completedAt),
      notes: entry.notes || undefined,
    }));

    logger.info(
      `Pulled ${entries.length} entries (page ${result.pageInfo.currentPage}/${result.pageInfo.lastPage})`,
    );

    return {
      entries,
      nextCursor: result.pageInfo.hasNextPage ? String(result.pageInfo.currentPage + 1) : undefined,
      hasMore: result.pageInfo.hasNextPage,
    };
  },

  async status(): Promise<SyncStatusResponse> {
    if (!client || viewerId === null) {
      return {
        pendingPush: 0,
        pendingPull: 0,
        conflicts: 0,
      };
    }

    // Get total count from AniList
    const result = await client.getMangaList(viewerId, 1, 1);

    return {
      externalCount: result.pageInfo.total,
      pendingPush: 0,
      pendingPull: 0,
      conflicts: 0,
    };
  },
};

// =============================================================================
// Plugin Initialization
// =============================================================================

createSyncPlugin({
  manifest,
  provider,
  logLevel: "debug",
  onInitialize(params: InitializeParams) {
    // Get access token from credentials
    const accessToken = params.credentials?.access_token;
    if (accessToken) {
      client = new AniListClient(accessToken);
      logger.info("AniList client initialized with access token");
    } else {
      logger.warn("No access token provided - sync operations will fail");
    }

  },
});

logger.info("AniList sync plugin started");
