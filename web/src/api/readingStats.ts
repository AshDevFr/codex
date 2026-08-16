import type { components } from "@/types/api.generated";
import { api } from "./client";

export type ReadingStatsResponse =
  components["schemas"]["ReadingStatsResponse"];
export type ReadingSummaryDto = components["schemas"]["ReadingSummaryDto"];
export type ReadingPeriodDto = components["schemas"]["ReadingPeriodDto"];
export type ReadingByDeviceDto = components["schemas"]["ReadingByDeviceDto"];
export type ReadingBySeriesDto = components["schemas"]["ReadingBySeriesDto"];
export type ReadingByFormatDto = components["schemas"]["ReadingByFormatDto"];
export type DurationBreakdownDto =
  components["schemas"]["DurationBreakdownDto"];
export type ReadingStatsGranularity =
  components["schemas"]["ReadingStatsGranularity"];
export type ReadingStatsSort = components["schemas"]["ReadingStatsSort"];
export type ReadingCoverage = components["schemas"]["ReadingCoverageDto"];

export interface ReadingStatsParams {
  from?: Date;
  to?: Date;
  granularity?: ReadingStatsGranularity;
  seriesLimit?: number;
  /**
   * Ranking key for the breakdowns. Decides which rows survive the series
   * limit, so it belongs in the request rather than in a client-side sort.
   */
  sort?: ReadingStatsSort;
}

export const readingStatsApi = {
  /**
   * Reading statistics for the signed-in user.
   *
   * Timestamps are sent with a `Z` suffix rather than a numeric offset: a bare
   * `+` in a query string decodes as a space, so `+00:00` never survives the
   * round trip.
   */
  get: async (
    params: ReadingStatsParams = {},
  ): Promise<ReadingStatsResponse> => {
    const query: Record<string, string> = {};
    if (params.from) query.from = params.from.toISOString();
    if (params.to) query.to = params.to.toISOString();
    if (params.granularity) query.granularity = params.granularity;
    if (params.seriesLimit) query.seriesLimit = String(params.seriesLimit);
    if (params.sort) query.sort = params.sort;

    const response = await api.get<ReadingStatsResponse>("/reading-stats", {
      params: query,
    });
    return response.data;
  },

  /**
   * The span the reader's history covers, ignoring any window.
   *
   * Separate from the statistics because it is window-independent: it decides
   * which years can be offered at all, and it changes at most once a day.
   */
  coverage: async (): Promise<ReadingCoverage> => {
    const response = await api.get<ReadingCoverage>("/reading-stats/coverage");
    return response.data;
  },
};
