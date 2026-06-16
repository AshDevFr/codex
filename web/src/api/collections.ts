import type { Series } from "@/types";
import type { components } from "@/types/api.generated";
import { api } from "./client";

export type Collection = components["schemas"]["CollectionDto"];
export type CreateCollectionRequest =
  components["schemas"]["CreateCollectionRequest"];
export type UpdateCollectionRequest =
  components["schemas"]["UpdateCollectionRequest"];

type CollectionListResponse = components["schemas"]["CollectionListResponse"];

export const collectionsApi = {
  /** All collections (with each collection's visible series count). */
  list: async (): Promise<Collection[]> => {
    const response = await api.get<CollectionListResponse>("/collections");
    return response.data.items;
  },

  get: async (id: string): Promise<Collection> => {
    const response = await api.get<Collection>(`/collections/${id}`);
    return response.data;
  },

  /** Member series, in stored order, filtered by the user's visibility. */
  getSeries: async (id: string): Promise<Series[]> => {
    const response = await api.get<Series[]>(`/collections/${id}/series`);
    return response.data;
  },

  create: async (body: CreateCollectionRequest): Promise<Collection> => {
    const response = await api.post<Collection>("/collections", body);
    return response.data;
  },

  update: async (
    id: string,
    body: UpdateCollectionRequest,
  ): Promise<Collection> => {
    const response = await api.patch<Collection>(`/collections/${id}`, body);
    return response.data;
  },

  delete: async (id: string): Promise<void> => {
    await api.delete(`/collections/${id}`);
  },

  addSeries: async (id: string, seriesIds: string[]): Promise<Collection> => {
    const response = await api.post<Collection>(`/collections/${id}/series`, {
      seriesIds,
    });
    return response.data;
  },

  removeSeries: async (id: string, seriesId: string): Promise<void> => {
    await api.delete(`/collections/${id}/series/${seriesId}`);
  },

  /** Set the full manual order of a collection's series. */
  reorder: async (id: string, seriesIds: string[]): Promise<void> => {
    await api.put(`/collections/${id}/series`, { seriesIds });
  },

  /** Collections that contain a given series. */
  forSeries: async (seriesId: string): Promise<Collection[]> => {
    const response = await api.get<CollectionListResponse>(
      `/series/${seriesId}/collections`,
    );
    return response.data.items;
  },
};
