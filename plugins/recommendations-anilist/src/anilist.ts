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
            averageScore
            siteUrl
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

export interface AniListRecommendationNode {
  rating: number;
  mediaRecommendation: {
    id: number;
    title: { romaji?: string; english?: string };
    coverImage: { large?: string };
    description: string | null;
    genres: string[];
    averageScore: number | null;
    siteUrl: string;
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
    const response = await fetch(ANILIST_API_URL, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        Authorization: `Bearer ${this.accessToken}`,
      },
      body: JSON.stringify({ query: queryStr, variables }),
    });

    if (response.status === 401) {
      throw new AuthError("AniList access token is invalid or expired");
    }

    if (response.status === 429) {
      const retryAfter = response.headers.get("Retry-After");
      const retrySeconds = retryAfter ? Number.parseInt(retryAfter, 10) : 60;
      throw new RateLimitError(
        Number.isNaN(retrySeconds) ? 60 : retrySeconds,
        "AniList rate limit exceeded",
      );
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

  /** Get community recommendations for a specific manga */
  async getRecommendationsForMedia(
    mediaId: number,
    perPage = 10,
  ): Promise<AniListRecommendationNode[]> {
    const data = await this.query<{
      Media: {
        id: number;
        title: { romaji?: string; english?: string };
        recommendations: { nodes: AniListRecommendationNode[] };
      };
    }>(MEDIA_RECOMMENDATIONS_QUERY, { mediaId, page: 1, perPage });

    return data.Media.recommendations.nodes;
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

/** Strip HTML tags from a string */
export function stripHtml(html: string | null): string | undefined {
  if (!html) return undefined;
  return html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]*>/g, "")
    .trim();
}
