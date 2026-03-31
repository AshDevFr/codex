/**
 * AniList GraphQL API client for recommendations
 *
 * Uses AniList's recommendations and user list data to generate
 * personalized manga suggestions.
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
    }
  }
`;

/** Get recommendations for a specific manga */
const MEDIA_RECOMMENDATIONS_QUERY = `
  query ($mediaId: Int!, $page: Int, $perPage: Int) {
    Media(id: $mediaId, type: MANGA) {
      id
      title {
        romaji
        english
      }
      recommendations(page: $page, perPage: $perPage, sort: RATING_DESC) {
        pageInfo {
          hasNextPage
        }
        nodes {
          rating
          mediaRecommendation {
            id
            title {
              romaji
              english
            }
            coverImage {
              large
            }
            description(asHtml: false)
            genres
            tags {
              name
              rank
              category
            }
            averageScore
            popularity
            siteUrl
            status
            format
            countryOfOrigin
            startDate {
              year
            }
            volumes
          }
        }
      }
    }
  }
`;

/** Search for a manga by title to find its AniList ID */
const SEARCH_MANGA_QUERY = `
  query ($search: String!) {
    Media(search: $search, type: MANGA) {
      id
      title {
        romaji
        english
      }
    }
  }
`;

/** Get the user's manga list to know what they've already seen */
const USER_MANGA_IDS_QUERY = `
  query ($userId: Int!, $page: Int, $perPage: Int) {
    Page(page: $page, perPage: $perPage) {
      pageInfo {
        hasNextPage
        currentPage
      }
      mediaList(userId: $userId, type: MANGA) {
        mediaId
      }
    }
  }
`;

// =============================================================================
// Types
// =============================================================================

/** AniList media status values */
export type AniListMediaStatus =
  | "FINISHED"
  | "RELEASING"
  | "NOT_YET_RELEASED"
  | "CANCELLED"
  | "HIATUS";

/** AniList media format values (manga-relevant subset) */
export type AniListMediaFormat = "MANGA" | "NOVEL" | "ONE_SHOT";

/** AniList tag on a media entry */
export interface AniListTag {
  name: string;
  rank: number;
  category: string;
}

export interface AniListRecommendationNode {
  rating: number;
  mediaRecommendation: {
    id: number;
    title: { romaji?: string; english?: string };
    coverImage: { large?: string };
    description: string | null;
    genres: string[];
    tags: AniListTag[];
    averageScore: number | null;
    popularity: number | null;
    siteUrl: string;
    status: AniListMediaStatus | null;
    format: AniListMediaFormat | null;
    countryOfOrigin: string | null;
    startDate: { year: number | null } | null;
    volumes: number | null;
  } | null;
}

interface SearchResult {
  id: number;
  title: { romaji?: string; english?: string };
}

// =============================================================================
// Client
// =============================================================================

export class AniListRecommendationClient {
  private accessToken: string;

  constructor(accessToken: string) {
    this.accessToken = accessToken;
  }

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

    const json = (await response.json()) as {
      data?: T;
      errors?: Array<{ message: string }>;
    };

    if (json.errors?.length) {
      const message = json.errors.map((e) => e.message).join("; ");
      throw new ApiError(`AniList GraphQL error: ${message}`);
    }

    if (!json.data) {
      throw new ApiError("AniList returned empty data");
    }

    return json.data;
  }

  /** Get the authenticated viewer's ID */
  async getViewerId(): Promise<number> {
    const data = await this.query<{ Viewer: { id: number; name: string } }>(VIEWER_QUERY);
    return data.Viewer.id;
  }

  /** Search for a manga by title and return its AniList ID */
  async searchManga(title: string): Promise<SearchResult | null> {
    try {
      const data = await this.query<{ Media: SearchResult | null }>(SEARCH_MANGA_QUERY, {
        search: title,
      });
      return data.Media;
    } catch {
      return null;
    }
  }

  /** Get community recommendations for a specific manga (up to maxPages pages) */
  async getRecommendationsForMedia(
    mediaId: number,
    perPage = 10,
    maxPages = 5,
  ): Promise<AniListRecommendationNode[]> {
    const allNodes: AniListRecommendationNode[] = [];
    let page = 1;
    let hasMore = true;

    while (hasMore && page <= maxPages) {
      const data = await this.query<{
        Media: {
          id: number;
          title: { romaji?: string; english?: string };
          recommendations: {
            pageInfo: { hasNextPage: boolean };
            nodes: AniListRecommendationNode[];
          };
        };
      }>(MEDIA_RECOMMENDATIONS_QUERY, { mediaId, page, perPage });

      allNodes.push(...data.Media.recommendations.nodes);
      hasMore = data.Media.recommendations.pageInfo.hasNextPage;
      page++;
    }

    return allNodes;
  }

  /** Get all manga IDs in the user's list (for deduplication) */
  async getUserMangaIds(userId: number): Promise<Set<number>> {
    const ids = new Set<number>();
    let page = 1;
    let hasMore = true;

    while (hasMore) {
      const data = await this.query<{
        Page: {
          pageInfo: { hasNextPage: boolean; currentPage: number };
          mediaList: Array<{ mediaId: number }>;
        };
      }>(USER_MANGA_IDS_QUERY, { userId, page, perPage: 50 });

      for (const entry of data.Page.mediaList) {
        ids.add(entry.mediaId);
      }

      hasMore = data.Page.pageInfo.hasNextPage;
      page++;
    }

    return ids;
  }
}

// =============================================================================
// Helpers
// =============================================================================

/** Get the best title from an AniList title object */
export function getBestTitle(title: { romaji?: string; english?: string }): string {
  return title.english || title.romaji || "Unknown";
}

/** Common HTML entities to decode */
const HTML_ENTITIES: Record<string, string> = {
  "&amp;": "&",
  "&lt;": "<",
  "&gt;": ">",
  "&quot;": '"',
  "&#39;": "'",
  "&apos;": "'",
  "&nbsp;": " ",
  "&mdash;": "\u2014",
  "&ndash;": "\u2013",
  "&hellip;": "\u2026",
};

const ENTITY_PATTERN = /&(?:#(\d+)|#x([0-9a-fA-F]+)|[a-zA-Z]+);/g;

/** Strip HTML tags and decode HTML entities */
export function stripHtml(html: string | null): string | undefined {
  if (!html) return undefined;
  return html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]*>/g, "")
    .replace(ENTITY_PATTERN, (match, decimal, hex) => {
      if (decimal) return String.fromCharCode(Number.parseInt(decimal, 10));
      if (hex) return String.fromCharCode(Number.parseInt(hex, 16));
      return HTML_ENTITIES[match] ?? match;
    })
    .trim();
}
