import type { components } from "@/types/api.generated";
import type { BookCondition, SeriesCondition } from "@/types/filters";
import { api } from "./client";

export type FilterPresetScope = "list" | "search";
export type FilterPresetTarget = "series" | "books";

export type FilterPresetDto = components["schemas"]["FilterPresetDto"];
export type FilterPresetListResponse =
  components["schemas"]["FilterPresetListResponse"];
export type CreateFilterPresetRequest =
  components["schemas"]["CreateFilterPresetRequest"];
export type UpdateFilterPresetRequest =
  components["schemas"]["UpdateFilterPresetRequest"];

export interface ListFilterPresetsParams {
  scope?: FilterPresetScope;
  target?: FilterPresetTarget;
  libraryId?: string;
}

export interface CreateFilterPresetInput {
  name: string;
  scope: FilterPresetScope;
  target: FilterPresetTarget;
  condition: SeriesCondition | BookCondition;
  query?: string | null;
  sort?: string | null;
  libraryId?: string | null;
}

export interface UpdateFilterPresetInput {
  name: string;
  condition: SeriesCondition | BookCondition;
  query?: string | null;
  sort?: string | null;
  libraryId?: string | null;
}

export const filterPresetsApi = {
  list: async (
    params?: ListFilterPresetsParams,
  ): Promise<FilterPresetDto[]> => {
    const search = new URLSearchParams();
    if (params?.scope) search.set("scope", params.scope);
    if (params?.target) search.set("target", params.target);
    if (params?.libraryId) search.set("libraryId", params.libraryId);

    const qs = search.toString();
    const url = qs ? `/filter-presets?${qs}` : "/filter-presets";
    const response = await api.get<FilterPresetListResponse>(url);
    return response.data.presets;
  },

  get: async (id: string): Promise<FilterPresetDto> => {
    const response = await api.get<FilterPresetDto>(`/filter-presets/${id}`);
    return response.data;
  },

  create: async (input: CreateFilterPresetInput): Promise<FilterPresetDto> => {
    const body: CreateFilterPresetRequest = {
      name: input.name,
      scope: input.scope,
      target: input.target,
      condition: input.condition as unknown as Record<string, never>,
      query: input.query ?? null,
      sort: input.sort ?? null,
      libraryId: input.libraryId ?? null,
    };
    const response = await api.post<FilterPresetDto>("/filter-presets", body);
    return response.data;
  },

  update: async (
    id: string,
    input: UpdateFilterPresetInput,
  ): Promise<FilterPresetDto> => {
    const body: UpdateFilterPresetRequest = {
      name: input.name,
      condition: input.condition as unknown as Record<string, never>,
      query: input.query ?? null,
      sort: input.sort ?? null,
      libraryId: input.libraryId ?? null,
    };
    const response = await api.put<FilterPresetDto>(
      `/filter-presets/${id}`,
      body,
    );
    return response.data;
  },

  delete: async (id: string): Promise<void> => {
    await api.delete(`/filter-presets/${id}`);
  },
};
