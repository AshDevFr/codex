/**
 * Sync Provider Protocol Types
 *
 * Defines the types for sync provider operations. These types MUST match
 * the Rust protocol exactly (see src/services/plugin/sync.rs in the Codex backend).
 *
 * Sync providers push and pull reading progress between Codex and external
 * services like AniList and MyAnimeList.
 *
 * ## Architecture
 *
 * Sync operations are initiated by the host (Codex) and sent to the plugin.
 * The plugin communicates with the external service using user credentials
 * provided during initialization.
 *
 * ## Methods
 *
 * - `sync/getUserInfo` - Get user info from external service
 * - `sync/pushProgress` - Push reading progress to external service
 * - `sync/pullProgress` - Pull reading progress from external service
 * - `sync/status` - Get sync status/diff between Codex and external
 *
 * @see src/services/plugin/sync.rs in the Codex backend
 */

// =============================================================================
// Reading Status
// =============================================================================

/**
 * Reading status for sync entries.
 *
 * Uses snake_case values to match Rust's `#[serde(rename_all = "snake_case")]`.
 */
export type SyncReadingStatus = "reading" | "completed" | "on_hold" | "dropped" | "plan_to_read";

// =============================================================================
// Sync Entry Result Status
// =============================================================================

/**
 * Status of a single sync entry operation result.
 *
 * Uses snake_case values to match Rust's `#[serde(rename_all = "snake_case")]`.
 */
export type SyncEntryResultStatus = "created" | "updated" | "unchanged" | "failed";

// =============================================================================
// User Info
// =============================================================================

/**
 * Response from `sync/getUserInfo` method.
 *
 * Returns the user's identity on the external service.
 * Used to display the connected account in the UI.
 */
export interface ExternalUserInfo {
  /** User ID on the external service */
  externalId: string;
  /** Display name / username */
  username: string;
  /** Avatar/profile image URL */
  avatarUrl?: string;
  /** Profile URL on the external service */
  profileUrl?: string;
}

// =============================================================================
// Sync Progress
// =============================================================================

/**
 * Reading progress details.
 *
 * All fields are optional to support different tracking granularities
 * (e.g., chapter-based for manga, page-based for single volumes).
 */
export interface SyncProgress {
  /** Number of chapters read */
  chapters?: number;
  /** Number of volumes read */
  volumes?: number;
  /** Number of pages read (for single-volume works) */
  pages?: number;
}

// =============================================================================
// Sync Entry (shared between push and pull)
// =============================================================================

/**
 * A single reading progress entry for sync.
 *
 * Represents one series/book's reading state that can be pushed to
 * or pulled from an external service.
 */
export interface SyncEntry {
  /** External ID on the target service (e.g., AniList media ID) */
  externalId: string;
  /** Reading status */
  status: SyncReadingStatus;
  /** Reading progress */
  progress?: SyncProgress;
  /** User's score/rating (service-specific scale, e.g., 1-10 or 1-100) */
  score?: number;
  /** When the user started reading (ISO 8601) */
  startedAt?: string;
  /** When the user completed reading (ISO 8601) */
  completedAt?: string;
  /** User notes */
  notes?: string;
}

// =============================================================================
// Push Progress
// =============================================================================

/**
 * Parameters for `sync/pushProgress` method.
 *
 * Sends reading progress from Codex to the external service.
 */
export interface SyncPushRequest {
  /** Entries to push to the external service */
  entries: SyncEntry[];
}

/**
 * Result for a single sync entry (push or pull).
 */
export interface SyncEntryResult {
  /** External ID of the entry */
  externalId: string;
  /** Result status */
  status: SyncEntryResultStatus;
  /** Error message if failed */
  error?: string;
}

/**
 * Response from `sync/pushProgress` method.
 */
export interface SyncPushResponse {
  /** Successfully synced entries */
  success: SyncEntryResult[];
  /** Failed entries */
  failed: SyncEntryResult[];
}

// =============================================================================
// Pull Progress
// =============================================================================

/**
 * Parameters for `sync/pullProgress` method.
 *
 * Requests reading progress from the external service.
 */
export interface SyncPullRequest {
  /** Only pull entries updated after this timestamp (ISO 8601). If not set, pulls all entries. */
  since?: string;
  /** Maximum number of entries to pull */
  limit?: number;
  /** Pagination cursor for continuing a previous pull */
  cursor?: string;
}

/**
 * Response from `sync/pullProgress` method.
 */
export interface SyncPullResponse {
  /** Entries pulled from the external service */
  entries: SyncEntry[];
  /** Cursor for next page (if more entries available) */
  nextCursor?: string;
  /** Whether there are more entries to pull */
  hasMore: boolean;
}

// =============================================================================
// Sync Status
// =============================================================================

/**
 * Response from `sync/status` method.
 *
 * Provides an overview of the sync state between Codex and the external service.
 */
export interface SyncStatusResponse {
  /** Last successful sync timestamp (ISO 8601) */
  lastSyncAt?: string;
  /** Number of entries on the external service */
  externalCount?: number;
  /** Number of entries that need to be pushed */
  pendingPush: number;
  /** Number of entries that need to be pulled */
  pendingPull: number;
  /** Entries with conflicts (different on both sides) */
  conflicts: number;
}

// =============================================================================
// Sync Provider Interface
// =============================================================================

/**
 * Interface for plugins that sync reading progress.
 *
 * Plugins implementing this capability can push and pull reading progress
 * between Codex and external services (e.g., AniList, MyAnimeList).
 *
 * Declare this capability in the plugin manifest with `syncProvider: true`.
 *
 * @example
 * ```typescript
 * const provider: SyncProvider = {
 *   async getUserInfo() {
 *     return {
 *       externalId: "12345",
 *       username: "manga_reader",
 *       avatarUrl: "https://anilist.co/img/avatar.jpg",
 *       profileUrl: "https://anilist.co/user/manga_reader",
 *     };
 *   },
 *   async pushProgress(params) {
 *     // Push entries to external service
 *     return { success: [], failed: [] };
 *   },
 *   async pullProgress(params) {
 *     // Pull entries from external service
 *     return { entries: [], hasMore: false };
 *   },
 * };
 * ```
 */
export interface SyncProvider {
  /**
   * Get user info from the external service.
   *
   * Returns the user's identity on the external service.
   * Used to display the connected account in the UI.
   *
   * @returns External user information
   */
  getUserInfo(): Promise<ExternalUserInfo>;

  /**
   * Push reading progress to the external service.
   *
   * Sends one or more reading progress entries from Codex to the
   * external service. Returns results indicating which entries
   * were created, updated, unchanged, or failed.
   *
   * @param params - Push request with entries to sync
   * @returns Push results with success and failure details
   */
  pushProgress(params: SyncPushRequest): Promise<SyncPushResponse>;

  /**
   * Pull reading progress from the external service.
   *
   * Retrieves reading progress entries from the external service.
   * Supports pagination via cursor and incremental sync via `since`.
   *
   * @param params - Pull request with optional filters and pagination
   * @returns Pull results with entries and pagination info
   */
  pullProgress(params: SyncPullRequest): Promise<SyncPullResponse>;

  /**
   * Get sync status overview (optional).
   *
   * Provides a summary of the sync state between Codex and the
   * external service, including pending operations and conflicts.
   *
   * @returns Sync status information
   */
  status?(): Promise<SyncStatusResponse>;
}
