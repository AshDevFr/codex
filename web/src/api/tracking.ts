import type { components } from "@/types/api.generated";
import { api } from "./client";

export type SeriesTracking = components["schemas"]["SeriesTrackingDto"];
export type UpdateSeriesTrackingRequest =
  components["schemas"]["UpdateSeriesTrackingRequest"];
export type SeriesAlias = components["schemas"]["SeriesAliasDto"];
export type CreateSeriesAliasRequest =
  components["schemas"]["CreateSeriesAliasRequest"];

export const trackingApi = {
  getTracking: async (seriesId: string): Promise<SeriesTracking> => {
    const response = await api.get<SeriesTracking>(
      `/series/${seriesId}/tracking`,
    );
    return response.data;
  },

  updateTracking: async (
    seriesId: string,
    update: UpdateSeriesTrackingRequest,
  ): Promise<SeriesTracking> => {
    const response = await api.patch<SeriesTracking>(
      `/series/${seriesId}/tracking`,
      update,
    );
    return response.data;
  },

  listAliases: async (seriesId: string): Promise<SeriesAlias[]> => {
    const response = await api.get<{ aliases: SeriesAlias[] }>(
      `/series/${seriesId}/aliases`,
    );
    return response.data.aliases;
  },

  createAlias: async (
    seriesId: string,
    request: CreateSeriesAliasRequest,
  ): Promise<SeriesAlias> => {
    const response = await api.post<SeriesAlias>(
      `/series/${seriesId}/aliases`,
      request,
    );
    return response.data;
  },

  deleteAlias: async (seriesId: string, aliasId: string): Promise<void> => {
    await api.delete(`/series/${seriesId}/aliases/${aliasId}`);
  },
};
