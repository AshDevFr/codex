import type { Series } from "@/types";
import type { components } from "@/types/api.generated";
import type { SeriesCondition } from "@/types/filters";
import { api } from "./client";

export type Collection = components["schemas"]["CollectionDto"];
export type CreateCollectionRequest =
  components["schemas"]["CreateCollectionRequest"];
export type UpdateCollectionRequest =
  components["schemas"]["UpdateCollectionRequest"];

/**
 * The generated schema types model a collection's `condition` as an opaque
 * object, because the API documents it as a free-form `Object`. Callers work
 * with the real `SeriesCondition` union instead and the cast happens here, at
 * the one boundary, rather than in every component.
 */
type WireCondition = Record<string, never>;

const encodeCondition = (condition: SeriesCondition | null | undefined) =>
  condition as unknown as WireCondition | null | undefined;

/** Create payload, with a typed membership rule. */
export type CreateCollectionInput = Omit<
  CreateCollectionRequest,
  "condition"
> & {
  condition?: SeriesCondition;
};

/**
 * Update payload, with a typed membership rule.
 *
 * `condition` is a tri-state: absent leaves the rule alone, `null` clears it
 * (converting the collection to hand-picked), and a value replaces it.
 */
export type UpdateCollectionInput = Omit<
  UpdateCollectionRequest,
  "condition"
> & {
  condition?: SeriesCondition | null;
};

type CollectionListResponse = components["schemas"]["CollectionListResponse"];

/**
 * Sort for a collection's members. An explicit sort always wins; when
 * omitted, the collection's `ordered` flag picks the default (`manual` when
 * set, `title` otherwise). Matches the API's `sort` query param.
 */
export type CollectionSeriesSort = "title" | "added" | "year" | "manual";

/** Direction for a chosen sort; the server ignores it for `manual`. */
export type SortDirection = "asc" | "desc";

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

  /**
   * Member series, filtered by the user's visibility. An explicit sort always
   * wins; otherwise the `ordered` flag picks the default order.
   */
  getSeries: async (
    id: string,
    sort?: CollectionSeriesSort,
    direction?: SortDirection,
  ): Promise<Series[]> => {
    const params = new URLSearchParams();
    if (sort) params.set("sort", sort);
    if (direction) params.set("direction", direction);
    const query = params.size > 0 ? `?${params}` : "";
    const response = await api.get<Series[]>(
      `/collections/${id}/series${query}`,
    );
    return response.data;
  },

  create: async (input: CreateCollectionInput): Promise<Collection> => {
    const body: CreateCollectionRequest = {
      ...input,
      condition: encodeCondition(input.condition) ?? undefined,
    };
    const response = await api.post<Collection>("/collections", body);
    return response.data;
  },

  update: async (
    id: string,
    input: UpdateCollectionInput,
  ): Promise<Collection> => {
    // Spread first so an absent `condition` stays absent on the wire; only
    // overwrite the key when the caller actually supplied one.
    const body: UpdateCollectionRequest = { ...input, condition: undefined };
    if ("condition" in input) {
      body.condition = encodeCondition(input.condition);
    }
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
