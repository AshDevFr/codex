/**
 * MangaBaka API client
 * API docs: https://mangabaka.org/api
 */

import {
  ApiError,
  AuthError,
  createLogger,
  NotFoundError,
  RateLimitError,
} from "@ashdev/codex-plugin-sdk";
import type { MbGetSeriesResponse, MbSearchResponse, MbSeries } from "./types.js";

const BASE_URL = "https://api.mangabaka.dev";
const DEFAULT_TIMEOUT_SECONDS = 60;
const logger = createLogger({ name: "mangabaka-api", level: "debug" });

export interface MangaBakaClientOptions {
  /** Request timeout in seconds (default: 60) */
  timeout?: number;
  /** Sort order for search results (passed as sort_by to the API) */
  sortBy?: string;
}

export class MangaBakaClient {
  private readonly apiKey: string;
  private readonly timeoutMs: number;
  private readonly sortBy: string | undefined;

  constructor(apiKey: string, options?: MangaBakaClientOptions) {
    if (!apiKey) {
      throw new AuthError("API key is required");
    }
    this.apiKey = apiKey;
    this.timeoutMs = (options?.timeout ?? DEFAULT_TIMEOUT_SECONDS) * 1000;
    this.sortBy = options?.sortBy;
    logger.debug(
      `MangaBakaClient initialized with timeout: ${this.timeoutMs}ms, sortBy: ${this.sortBy ?? "default"}`,
    );
  }

  /**
   * Search for series by query
   */
  async search(
    query: string,
    page = 1,
    perPage = 20,
  ): Promise<{ data: MbSeries[]; total: number; page: number; totalPages: number }> {
    logger.debug(`Searching for: "${query}" (page ${page})`);

    const params = new URLSearchParams({
      q: query,
      page: String(page),
      limit: String(perPage),
    });

    if (this.sortBy) {
      params.set("sort_by", this.sortBy);
    }

    const response = await this.request<MbSearchResponse>(`/v1/series/search?${params.toString()}`);

    return {
      data: response.data,
      total: response.pagination?.total ?? response.data.length,
      page: response.pagination?.page ?? page,
      totalPages: response.pagination?.total_pages ?? 1,
    };
  }

  /**
   * Get full series details by ID
   */
  async getSeries(id: number): Promise<MbSeries> {
    logger.debug(`Getting series: ${id}`);

    const response = await this.request<MbGetSeriesResponse>(`/v1/series/${id}`);

    return response.data;
  }

  /**
   * Make an authenticated request to the MangaBaka API
   */
  private async request<T>(path: string): Promise<T> {
    const url = `${BASE_URL}${path}`;
    const headers: Record<string, string> = {
      "x-api-key": this.apiKey,
      Accept: "application/json",
    };

    // Set up timeout using AbortController
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      logger.debug(`Request: ${path} (timeout: ${this.timeoutMs}ms)`);
      const response = await fetch(url, {
        method: "GET",
        headers,
        signal: controller.signal,
      });

      // Handle rate limiting
      if (response.status === 429) {
        const retryAfter = response.headers.get("Retry-After");
        const seconds = retryAfter ? Number.parseInt(retryAfter, 10) : 60;
        throw new RateLimitError(seconds);
      }

      // Handle auth errors
      if (response.status === 401 || response.status === 403) {
        throw new AuthError("Invalid API key");
      }

      // Handle not found
      if (response.status === 404) {
        throw new NotFoundError(`Resource not found: ${path}`);
      }

      // Handle other errors
      if (!response.ok) {
        const text = await response.text();
        logger.error(`API error: ${response.status}`, { body: text });
        throw new ApiError(`API error: ${response.status} ${response.statusText}`, response.status);
      }

      return response.json() as Promise<T>;
    } catch (error) {
      // Handle timeout (abort)
      if (error instanceof Error && error.name === "AbortError") {
        logger.error(`Request timed out after ${this.timeoutMs}ms: ${path}`);
        throw new ApiError(`Request timed out after ${this.timeoutMs / 1000}s`);
      }

      // Re-throw plugin errors
      if (
        error instanceof RateLimitError ||
        error instanceof AuthError ||
        error instanceof NotFoundError ||
        error instanceof ApiError
      ) {
        throw error;
      }

      // Wrap other errors
      const message = error instanceof Error ? error.message : "Unknown error";
      logger.error("Request failed", error);
      throw new ApiError(`Request failed: ${message}`);
    } finally {
      clearTimeout(timeoutId);
    }
  }
}
