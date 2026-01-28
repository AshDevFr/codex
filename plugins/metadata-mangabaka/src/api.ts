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
} from "@codex/plugin-sdk";
import type { MbGetSeriesResponse, MbSearchResponse, MbSeries } from "./types.js";

const BASE_URL = "https://api.mangabaka.dev";
const logger = createLogger({ name: "mangabaka-api", level: "debug" });

export class MangaBakaClient {
  private readonly apiKey: string;

  constructor(apiKey: string) {
    if (!apiKey) {
      throw new AuthError("API key is required");
    }
    this.apiKey = apiKey;
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

    try {
      const response = await fetch(url, {
        method: "GET",
        headers,
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
    }
  }
}
