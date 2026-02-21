import type { components } from "@/types/api.generated";
import { api } from "./client";

export type BulkMetadataUpdateResponse =
  components["schemas"]["BulkMetadataUpdateResponse"];

export type BulkPatchSeriesMetadataRequest =
  components["schemas"]["BulkPatchSeriesMetadataRequest"];

export type BulkPatchBookMetadataRequest =
  components["schemas"]["BulkPatchBookMetadataRequest"];

export type BulkModifySeriesTagsRequest =
  components["schemas"]["BulkModifySeriesTagsRequest"];

export type BulkModifySeriesGenresRequest =
  components["schemas"]["BulkModifySeriesGenresRequest"];

export type BulkModifyBookTagsRequest =
  components["schemas"]["BulkModifyBookTagsRequest"];

export type BulkModifyBookGenresRequest =
  components["schemas"]["BulkModifyBookGenresRequest"];

export type BulkUpdateSeriesLocksRequest =
  components["schemas"]["BulkUpdateSeriesLocksRequest"];

export type BulkUpdateBookLocksRequest =
  components["schemas"]["BulkUpdateBookLocksRequest"];

export const bulkMetadataApi = {
  // ==================== Series Bulk Operations ====================

  patchSeriesMetadata: async (
    data: BulkPatchSeriesMetadataRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.patch<BulkMetadataUpdateResponse>(
      "/series/bulk/metadata",
      data,
    );
    return response.data;
  },

  modifySeriesTags: async (
    data: BulkModifySeriesTagsRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.post<BulkMetadataUpdateResponse>(
      "/series/bulk/tags",
      data,
    );
    return response.data;
  },

  modifySeriesGenres: async (
    data: BulkModifySeriesGenresRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.post<BulkMetadataUpdateResponse>(
      "/series/bulk/genres",
      data,
    );
    return response.data;
  },

  updateSeriesLocks: async (
    data: BulkUpdateSeriesLocksRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.put<BulkMetadataUpdateResponse>(
      "/series/bulk/metadata/locks",
      data,
    );
    return response.data;
  },

  // ==================== Book Bulk Operations ====================

  patchBookMetadata: async (
    data: BulkPatchBookMetadataRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.patch<BulkMetadataUpdateResponse>(
      "/books/bulk/metadata",
      data,
    );
    return response.data;
  },

  modifyBookTags: async (
    data: BulkModifyBookTagsRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.post<BulkMetadataUpdateResponse>(
      "/books/bulk/tags",
      data,
    );
    return response.data;
  },

  modifyBookGenres: async (
    data: BulkModifyBookGenresRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.post<BulkMetadataUpdateResponse>(
      "/books/bulk/genres",
      data,
    );
    return response.data;
  },

  updateBookLocks: async (
    data: BulkUpdateBookLocksRequest,
  ): Promise<BulkMetadataUpdateResponse> => {
    const response = await api.put<BulkMetadataUpdateResponse>(
      "/books/bulk/metadata/locks",
      data,
    );
    return response.data;
  },
};
