/**
 * AniList GraphQL API client
 *
 * Provides typed access to AniList's GraphQL API for reading progress sync.
 * See: https://anilist.gitbook.io/anilist-apiv2-docs/
 */

import { ApiError, AuthError, RateLimitError } from "@ashdev/codex-plugin-sdk";

const ANILIST_API_URL = "https://graphql.anilist.co";

// =============================================================================
// GraphQL Queries
// =============================================================================

const VIEWER_QUERY = `
  query {
    Viewer {
      id
      name
      avatar {
        large
        medium
      }
      siteUrl
      options {
        displayAdultContent
      }
      mediaListOptions {
        scoreFormat
      }
    }
  }
`;

const MANGA_LIST_QUERY = `
  query ($userId: Int!, $page: Int, $perPage: Int) {
    Page(page: $page, perPage: $perPage) {
      pageInfo {
        total
        currentPage
        lastPage
        hasNextPage
      }
      mediaList(userId: $userId, type: MANGA, sort: UPDATED_TIME_DESC) {
        id
        mediaId
        status
        score
        progress
        progressVolumes
        startedAt {
          year
          month
          day
        }
        completedAt {
          year
          month
          day
        }
        notes
        updatedAt
        media {
          id
          title {
            romaji
            english
            native
          }
          siteUrl
        }
      }
    }
  }
`;

const UPDATE_ENTRY_MUTATION = `
  mutation (
    $mediaId: Int!,
    $status: MediaListStatus,
    $score: Float,
    $progress: Int,
    $progressVolumes: Int,
    $startedAt: FuzzyDateInput,
    $completedAt: FuzzyDateInput,
    $notes: String
  ) {
    SaveMediaListEntry(
      mediaId: $mediaId,
      status: $status,
      score: $score,
      progress: $progress,
      progressVolumes: $progressVolumes,
      startedAt: $startedAt,
      completedAt: $completedAt,
      notes: $notes
    ) {
      id
      mediaId
      status
      score
      progress
      progressVolumes
    }
  }
`;

// =============================================================================
// Types
// =============================================================================

export interface AniListViewer {
  id: number;
  name: string;
  avatar: { large?: string; medium?: string };
  siteUrl: string;
  options: { displayAdultContent: boolean };
  mediaListOptions: { scoreFormat: string };
}

export interface AniListFuzzyDate {
  year?: number | null;
  month?: number | null;
  day?: number | null;
}

export interface AniListMediaListEntry {
  id: number;
  mediaId: number;
  status: string;
  score: number;
  progress: number;
  progressVolumes: number;
  startedAt: AniListFuzzyDate;
  completedAt: AniListFuzzyDate;
  notes: string | null;
  updatedAt: number;
  media: {
    id: number;
    title: { romaji?: string; english?: string; native?: string };
    siteUrl: string;
  };
}

export interface AniListPageInfo {
  total: number;
  currentPage: number;
  lastPage: number;
  hasNextPage: boolean;
}

export interface AniListSaveResult {
  id: number;
  mediaId: number;
  status: string;
  score: number;
  progress: number;
  progressVolumes: number;
}

// =============================================================================
// Client
// =============================================================================

export class AniListClient {
  private accessToken: string;

  constructor(accessToken: string) {
    this.accessToken = accessToken;
  }

  /**
   * Execute a GraphQL query against the AniList API.
   * On rate limit (429), waits the requested duration and retries once.
   */
  private async query<T>(queryStr: string, variables?: Record<string, unknown>): Promise<T> {
    return this.executeQuery<T>(queryStr, variables, true);
  }

  private async executeQuery<T>(
    queryStr: string,
    variables: Record<string, unknown> | undefined,
    allowRetry: boolean,
  ): Promise<T> {
    let response: Response;
    try {
      response = await fetch(ANILIST_API_URL, {
        method: "POST",
        signal: AbortSignal.timeout(30_000),
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
          Authorization: `Bearer ${this.accessToken}`,
        },
        body: JSON.stringify({ query: queryStr, variables }),
      });
    } catch (error) {
      if (error instanceof DOMException && error.name === "TimeoutError") {
        throw new ApiError("AniList API request timed out after 30 seconds");
      }
      throw error;
    }

    if (response.status === 401) {
      throw new AuthError("AniList access token is invalid or expired");
    }

    if (response.status === 429) {
      const retryAfter = response.headers.get("Retry-After");
      const retrySeconds = retryAfter ? Number.parseInt(retryAfter, 10) : 60;
      const waitSeconds = Number.isNaN(retrySeconds) ? 60 : retrySeconds;

      if (allowRetry) {
        await new Promise((resolve) => setTimeout(resolve, waitSeconds * 1000));
        return this.executeQuery<T>(queryStr, variables, false);
      }

      throw new RateLimitError(waitSeconds, "AniList rate limit exceeded");
    }

    if (!response.ok) {
      const body = await response.text().catch(() => "");
      throw new ApiError(
        `AniList API error: ${response.status} ${response.statusText}${body ? ` - ${body}` : ""}`,
      );
    }

    const json = (await response.json()) as { data?: T; errors?: Array<{ message: string }> };

    if (json.errors?.length) {
      const message = json.errors.map((e) => e.message).join("; ");
      throw new ApiError(`AniList GraphQL error: ${message}`);
    }

    if (!json.data) {
      throw new ApiError("AniList returned empty data");
    }

    return json.data;
  }

  /**
   * Get the authenticated user's info
   */
  async getViewer(): Promise<AniListViewer> {
    const data = await this.query<{ Viewer: AniListViewer }>(VIEWER_QUERY);
    return data.Viewer;
  }

  /**
   * Get the user's manga list (paginated)
   */
  async getMangaList(
    userId: number,
    page = 1,
    perPage = 50,
  ): Promise<{ pageInfo: AniListPageInfo; entries: AniListMediaListEntry[] }> {
    const variables: Record<string, unknown> = { userId, page, perPage };

    const data = await this.query<{
      Page: {
        pageInfo: AniListPageInfo;
        mediaList: AniListMediaListEntry[];
      };
    }>(MANGA_LIST_QUERY, variables);

    return {
      pageInfo: data.Page.pageInfo,
      entries: data.Page.mediaList,
    };
  }

  /**
   * Update or create a manga list entry
   */
  async saveEntry(variables: {
    mediaId: number;
    status?: string;
    score?: number;
    progress?: number;
    progressVolumes?: number;
    startedAt?: AniListFuzzyDate;
    completedAt?: AniListFuzzyDate;
    notes?: string;
  }): Promise<AniListSaveResult> {
    const data = await this.query<{ SaveMediaListEntry: AniListSaveResult }>(
      UPDATE_ENTRY_MUTATION,
      variables,
    );
    return data.SaveMediaListEntry;
  }
}

// =============================================================================
// Status Mapping
// =============================================================================

/**
 * Map AniList MediaListStatus to Codex SyncReadingStatus
 */
export function anilistStatusToSync(
  status: string,
): "reading" | "completed" | "on_hold" | "dropped" | "plan_to_read" {
  switch (status) {
    case "CURRENT":
    case "REPEATING":
      return "reading";
    case "COMPLETED":
      return "completed";
    case "PAUSED":
      return "on_hold";
    case "DROPPED":
      return "dropped";
    case "PLANNING":
      return "plan_to_read";
    default:
      return "reading";
  }
}

/**
 * Map Codex SyncReadingStatus to AniList MediaListStatus
 */
export function syncStatusToAnilist(
  status: string,
): "CURRENT" | "COMPLETED" | "PAUSED" | "DROPPED" | "PLANNING" {
  switch (status) {
    case "reading":
      return "CURRENT";
    case "completed":
      return "COMPLETED";
    case "on_hold":
      return "PAUSED";
    case "dropped":
      return "DROPPED";
    case "plan_to_read":
      return "PLANNING";
    default:
      return "CURRENT";
  }
}

/**
 * Convert AniList FuzzyDate to ISO 8601 string
 */
export function fuzzyDateToIso(date: AniListFuzzyDate | null | undefined): string | undefined {
  if (!date?.year) return undefined;
  const month = date.month ? String(date.month).padStart(2, "0") : "01";
  const day = date.day ? String(date.day).padStart(2, "0") : "01";
  return `${date.year}-${month}-${day}T00:00:00Z`;
}

/**
 * Convert ISO 8601 string to AniList FuzzyDate
 */
export function isoToFuzzyDate(iso: string | undefined): AniListFuzzyDate | undefined {
  if (!iso) return undefined;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return undefined;
  return {
    year: d.getUTCFullYear(),
    month: d.getUTCMonth() + 1,
    day: d.getUTCDate(),
  };
}

// =============================================================================
// Score Conversion
// =============================================================================

/**
 * Convert a score from Codex's 1-100 scale to AniList's format
 */
export function convertScoreToAnilist(score: number, format: string): number {
  switch (format) {
    case "POINT_100":
      return Math.round(score);
    case "POINT_10_DECIMAL":
      return score / 10;
    case "POINT_10":
      return Math.round(score / 10);
    case "POINT_5":
      return Math.round(score / 20);
    case "POINT_3":
      if (score >= 70) return 3;
      if (score >= 40) return 2;
      return 1;
    default:
      return Math.round(score / 10);
  }
}

/**
 * Convert a score from AniList's format to Codex's 1-100 scale
 */
export function convertScoreFromAnilist(score: number, format: string): number {
  switch (format) {
    case "POINT_100":
      return score;
    case "POINT_10_DECIMAL":
      return score * 10;
    case "POINT_10":
      return score * 10;
    case "POINT_5":
      return score * 20;
    case "POINT_3":
      return Math.round(score * 33.3);
    default:
      return score * 10;
  }
}
