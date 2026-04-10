import { api } from "./client";

// Types (manually defined since not yet in OpenAPI)

export interface SeriesExportDto {
  id: string;
  format: string;
  status: string;
  libraryIds: string[];
  fields: string[];
  fileSizeBytes: number | null;
  rowCount: number | null;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
  expiresAt: string;
}

export interface CreateSeriesExportRequest {
  format: string;
  libraryIds: string[];
  fields: string[];
}

export interface SeriesExportListResponse {
  exports: SeriesExportDto[];
}

export interface ExportFieldDto {
  key: string;
  label: string;
  multiValue: boolean;
  userSpecific: boolean;
}

export interface ExportFieldCatalogResponse {
  fields: ExportFieldDto[];
}

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

  /** Get the download URL for an export (uses auth from client interceptor) */
  download: async (id: string): Promise<Blob> => {
    const response = await api.get(`/user/exports/series/${id}/download`, {
      responseType: "blob",
    });
    return response.data as Blob;
  },
};
