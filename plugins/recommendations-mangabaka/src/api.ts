/**
 * MangaBaka recommendation API client.
 *
 * Covers the three endpoints this plugin needs. None of them require
 * authentication, so unlike `metadata-mangabaka` no API key header is sent.
 *
 * Stability: `/v1/series/mix` and `/v1/series/{id}/readers-also-like` are both
 * marked `x-api-stability: beta` upstream and may change without notice.
 * `/v1/tags` is stable. Every response is therefore parsed defensively: a
 * missing or renamed field degrades the result rather than throwing, so a
 * partial upstream change costs recommendation quality instead of breaking the
 * task outright.
 *
 * API docs: https://mangabaka.org/api
 */

import { ApiError, NotFoundError, RateLimitError } from "@ashdev/codex-plugin-sdk";
import { logger } from "./logger.js";
import type {
  MbCatalogueTag,
  MbMixParams,
  MbMixResponse,
  MbReadersAlsoLikeParams,
  MbReadersAlsoLikeResponse,
  MbRecommendationEntry,
  MbTagsResponse,
} from "./types.js";

export const DEFAULT_BASE_URL = "https://api.mangabaka.org";
const DEFAULT_TIMEOUT_SECONDS = 30;
const DEFAULT_RETRY_AFTER_SECONDS = 60;

/** Upstream caps, from the OpenAPI spec. Requests are clamped rather than rejected. */
const MIX_LIMIT_MAX = 50;
const READERS_ALSO_LIKE_LIMIT_MAX = 24;

export interface MangaBakaRecommendationClientOptions {
  /** Request timeout in seconds (default: 30). */
  timeout?: number;
  /**
   * Override the API base URL (default: {@link DEFAULT_BASE_URL}).
   *
   * The escape hatch if the beta endpoints move hosts, or for pointing at a
   * caching proxy.
   */
  baseUrl?: string;
}

/** Clamp a value into an inclusive range, rounding to an integer. */
function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(Math.round(value), max));
}

/**
 * Append an array parameter as repeated keys (`k=a&k=b`), which is the form the
 * MangaBaka API expects. Empty and absent arrays append nothing, so an unset
 * filter stays absent from the URL rather than becoming an empty-string value
 * that upstream would try to parse.
 */
function appendAll(params: URLSearchParams, key: string, values?: readonly unknown[]): void {
  if (!values || values.length === 0) return;
  for (const value of values) {
    params.append(key, String(value));
  }
}

export class MangaBakaRecommendationClient {
  private readonly timeoutMs: number;
  /** Resolved API base URL, normalized without a trailing slash. */
  readonly baseUrl: string;

  constructor(options?: MangaBakaRecommendationClientOptions) {
    this.timeoutMs = (options?.timeout ?? DEFAULT_TIMEOUT_SECONDS) * 1000;
    // Request paths are absolute, so a trailing slash would produce `//v1/...`.
    this.baseUrl = (options?.baseUrl?.trim() || DEFAULT_BASE_URL).replace(/\/+$/, "");
    logger.debug(
      `MangaBakaRecommendationClient initialized with baseUrl: ${this.baseUrl}, timeout: ${this.timeoutMs}ms`,
    );
  }

  /**
   * Multi-seed content similarity.
   *
   * Combines every seed's tag vector into a single probe and returns
   * MMR-diversified results annotated with `matched_seed_ids`, so one request
   * covers the whole seed set.
   */
  async mix(params: MbMixParams): Promise<MbRecommendationEntry[]> {
    if (params.series.length === 0) {
      // Upstream requires at least one seed or include-tag and answers 400.
      // Fail here instead of spending a round trip to be told so.
      throw new ApiError("mix requires at least one seed series");
    }

    const query = new URLSearchParams();
    appendAll(query, "series", params.series);

    if (params.limit !== undefined) {
      query.set("limit", String(clamp(params.limit, 1, MIX_LIMIT_MAX)));
    }
    if (params.strict !== undefined) {
      query.set("strict", String(params.strict));
    }
    if (params.ratingLower !== undefined) {
      query.set("rating_lower", String(clamp(params.ratingLower, 0, 100)));
    }
    if (params.ratingUpper !== undefined) {
      query.set("rating_upper", String(clamp(params.ratingUpper, 0, 100)));
    }

    appendAll(query, "content_rating", params.contentRating);
    appendAll(query, "tag_not", params.tagNot);
    appendAll(query, "type", params.type);
    appendAll(query, "type_not", params.typeNot);
    appendAll(query, "status", params.status);
    appendAll(query, "status_not", params.statusNot);
    appendAll(query, "genre_not", params.genreNot);

    logger.debug(`Mix request for ${params.series.length} seeds`);
    const response = await this.request<MbMixResponse>(`/v1/series/mix?${query.toString()}`);

    return sanitizeEntries(response.data, "mix");
  }

  /**
   * Collaborative filtering for a single series, based on shared library
   * activity. This is the taste signal that tag-vector similarity cannot
   * provide on its own.
   */
  async readersAlsoLike(
    seriesId: number,
    params?: MbReadersAlsoLikeParams,
  ): Promise<MbRecommendationEntry[]> {
    const query = new URLSearchParams();

    if (params?.limit !== undefined) {
      query.set("limit", String(clamp(params.limit, 1, READERS_ALSO_LIKE_LIMIT_MAX)));
    }
    appendAll(query, "content_rating", params?.contentRating);
    appendAll(query, "tag_not", params?.tagNot);

    const suffix = query.toString();
    logger.debug(`Readers-also-like request for series ${seriesId}`);
    const response = await this.request<MbReadersAlsoLikeResponse>(
      `/v1/series/${seriesId}/readers-also-like${suffix ? `?${suffix}` : ""}`,
    );

    return sanitizeEntries(response.data, "readers-also-like");
  }

  /**
   * The global tag catalogue.
   *
   * Needed because `tag_not` accepts numeric tag IDs only; passing a tag name
   * returns HTTP 400. Callers resolve user-supplied names through this list.
   */
  async tags(): Promise<MbCatalogueTag[]> {
    const response = await this.request<MbTagsResponse>("/v1/tags");

    if (!Array.isArray(response.data)) {
      logger.warn("Tag catalogue response had no data array");
      return [];
    }

    return response.data.filter(
      (tag): tag is MbCatalogueTag =>
        typeof tag?.id === "number" && typeof tag?.name === "string" && tag.name.length > 0,
    );
  }

  /** Issue an unauthenticated GET and classify upstream failures. */
  private async request<T>(path: string): Promise<T> {
    const url = `${this.baseUrl}${path}`;

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      logger.debug(`Request: ${path} (timeout: ${this.timeoutMs}ms)`);
      const response = await fetch(url, {
        method: "GET",
        // No x-api-key: every endpoint used here is public.
        headers: { Accept: "application/json" },
        signal: controller.signal,
      });

      if (response.status === 429) {
        const retryAfter = response.headers.get("Retry-After");
        const parsed = retryAfter ? Number.parseInt(retryAfter, 10) : Number.NaN;
        const seconds = Number.isNaN(parsed) ? DEFAULT_RETRY_AFTER_SECONDS : parsed;
        throw new RateLimitError(seconds);
      }

      if (response.status === 404) {
        throw new NotFoundError(`Resource not found: ${path}`);
      }

      if (!response.ok) {
        const text = await response.text();
        logger.error(`API error: ${response.status}`, { body: text });
        throw new ApiError(`API error: ${response.status} ${response.statusText}`, response.status);
      }

      return (await response.json()) as T;
    } catch (error) {
      if (error instanceof Error && error.name === "AbortError") {
        logger.error(`Request timed out after ${this.timeoutMs}ms: ${path}`);
        throw new ApiError(`Request timed out after ${this.timeoutMs / 1000}s`);
      }

      if (
        error instanceof RateLimitError ||
        error instanceof NotFoundError ||
        error instanceof ApiError
      ) {
        throw error;
      }

      const message = error instanceof Error ? error.message : "Unknown error";
      logger.error("Request failed", error);
      throw new ApiError(`Request failed: ${message}`);
    } finally {
      clearTimeout(timeoutId);
    }
  }
}

/**
 * Drop entries that carry no identifiable series.
 *
 * `series.id` is the one field everything downstream depends on: it keys
 * deduplication, exclusion, and the external ID handed back to Codex. An entry
 * without it cannot be used for anything, so it is discarded here rather than
 * guarded against at every later stage.
 */
function sanitizeEntries(data: unknown, source: string): MbRecommendationEntry[] {
  if (!Array.isArray(data)) {
    logger.warn(`${source} response had no data array`);
    return [];
  }

  const usable = data.filter(
    (entry): entry is MbRecommendationEntry => typeof entry?.series?.id === "number",
  );

  const dropped = data.length - usable.length;
  if (dropped > 0) {
    logger.warn(`Dropped ${dropped} ${source} entries with no series id`);
  }

  return usable;
}
