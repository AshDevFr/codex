import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type SeriesExportDto = components["schemas"]["SeriesExportDto"];
export type CreateSeriesExportRequest =
  components["schemas"]["CreateSeriesExportRequest"];
export type SeriesExportListResponse =
  components["schemas"]["SeriesExportListResponse"];
export type ExportFieldDto = components["schemas"]["ExportFieldDto"];
export type ExportFieldCatalogResponse =
  components["schemas"]["ExportFieldCatalogResponse"];

export const seriesExportsApi = {
  /** Create a new series export job */
  create: async (
    request: CreateSeriesExportRequest,
  ): Promise<SeriesExportDto> => {
    const response = await api.post<SeriesExportDto>(
      "/user/exports/series",
      request,
    );
    return response.data;
  },

  /** List current user's exports */
  list: async (): Promise<SeriesExportDto[]> => {
    const response = await api.get<SeriesExportListResponse>(
      "/user/exports/series",
    );
    return response.data.exports;
  },

  /** Get a single export by ID */
  get: async (id: string): Promise<SeriesExportDto> => {
    const response = await api.get<SeriesExportDto>(
      `/user/exports/series/${id}`,
    );
    return response.data;
  },

  /** Delete an export */
  delete: async (id: string): Promise<void> => {
    await api.delete(`/user/exports/series/${id}`);
  },

  /** Get the field catalog */
  getFieldCatalog: async (): Promise<ExportFieldDto[]> => {
    const response = await api.get<ExportFieldCatalogResponse>(
      "/user/exports/series/fields",
    );
    return response.data.fields;
  },

  /** Download an export file as blob (auth handled by client interceptor) */
  download: async (id: string): Promise<Blob> => {
    const response = await api.get(`/user/exports/series/${id}/download`, {
      responseType: "blob",
    });
    return response.data as Blob;
  },
};
