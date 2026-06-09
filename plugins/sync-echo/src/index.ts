/**
 * Echo Sync Plugin for Codex
 *
 * A minimal test/debug sync plugin. It does not talk to any external service:
 * it accepts any push (echoing every entry back as created/updated), returns
 * deterministic, fully-populated entries on pull, and reports canned status.
 *
 * Its main purpose is debugging the host -> plugin sync protocol: it declares
 * `wantsDetailedProgress` so it receives the per-book detailed progress payload,
 * and records every request and its response to JSON files under the plugin's
 * data directory (see `recorder.ts`).
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
import { DEFAULT_MAX_PAYLOAD_FILES, DEFAULT_PULL_COUNT, manifest } from "./manifest.js";
import { PayloadRecorder, redactConfig } from "./recorder.js";

const logger = createLogger({ name: "sync-echo", level: "debug" });

// Plugin configuration (set during initialization)
const config = {
  pullCount: DEFAULT_PULL_COUNT,
};

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

/** Set the pull entry count (exported for testing) */
export function setPullCount(count: number): void {
  config.pullCount = count;
}

const STATUSES: SyncEntry["status"][] = [
  "reading",
  "completed",
  "on_hold",
  "plan_to_read",
  "dropped",
];

/**
 * Build a deterministic, fully-populated sync entry for the given index. Every
 * optional field is set so the host's pull-ingest path is exercised end to end.
 */
function makeEntry(i: number): SyncEntry {
  const day = String((i % 28) + 1).padStart(2, "0");
  return {
    externalId: String(1000 + i),
    status: STATUSES[i % STATUSES.length],
    progress: {
      chapters: i + 1,
      volumes: i + 1,
      pages: (i + 1) * 20,
      totalChapters: 100,
      totalVolumes: 12,
      maxVolume: i + 1,
      maxChapter: i + 1.5,
      readBooks: [
        {
          volume: i + 1,
          chapter: i + 1.5,
          completed: i % 2 === 0,
          currentPage: (i + 1) * 10,
          progressPercentage: 0.5,
        },
      ],
    },
    score: 70 + (i % 30),
    startedAt: `2026-01-${day}T00:00:00.000Z`,
    completedAt: `2026-02-${day}T00:00:00.000Z`,
    notes: `Echo note for entry ${i}`,
    latestUpdatedAt: `2026-03-${day}T00:00:00.000Z`,
    title: `Echo Series ${i + 1}`,
  };
}

/** Exported for testing */
export const provider: SyncProvider = {
  async getUserInfo(): Promise<ExternalUserInfo> {
    return rec<ExternalUserInfo>("sync/getUserInfo", null, {
      externalId: "echo-user-1",
      username: "echo_user",
      avatarUrl: "https://picsum.photos/100/100",
      profileUrl: "https://echo.example.com/user/echo_user",
    });
  },

  async pushProgress(params: SyncPushRequest): Promise<SyncPushResponse> {
    logger.info(`Push received: ${params.entries.length} entries`);

    // Echo every entry back as a success, alternating created/updated so both
    // result statuses are exercised. Nothing is ever rejected.
    const success: SyncEntryResult[] = params.entries.map((entry, i) => ({
      externalId: entry.externalId,
      status: i % 2 === 0 ? "created" : "updated",
    }));

    return rec<SyncPushResponse>("sync/pushProgress", params, {
      success,
      failed: [],
    });
  },

  async pullProgress(params: SyncPullRequest): Promise<SyncPullResponse> {
    const requested = params.limit ?? config.pullCount;
    const count = Math.min(Math.max(0, requested), config.pullCount);
    const entries = Array.from({ length: count }, (_, i) => makeEntry(i));

    logger.info(`Pull returning ${entries.length} deterministic entries`);

    return rec<SyncPullResponse>("sync/pullProgress", params, {
      entries,
      hasMore: false,
    });
  },

  async status(): Promise<SyncStatusResponse> {
    return rec<SyncStatusResponse>("sync/status", null, {
      lastSyncAt: "2026-03-01T00:00:00.000Z",
      externalCount: config.pullCount,
      pendingPush: 0,
      pendingPull: 0,
      conflicts: 0,
    });
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
    // Honor the host-supplied log level (Codex `plugins.log_level` config).
    if (params.logLevel) logger.setLevel(params.logLevel);
    const pullCount = params.adminConfig?.pullCount;
    if (typeof pullCount === "number") {
      config.pullCount = Math.min(Math.max(1, Math.floor(pullCount)), 50);
    }

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

    logger.info(
      `Echo sync plugin initialized (pullCount: ${config.pullCount}, recordPayloads: ${recordPayloads})`,
    );
  },
});

logger.info("Echo sync plugin started");
