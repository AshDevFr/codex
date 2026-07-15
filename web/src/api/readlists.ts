import type { Book } from "@/types";
import type { components } from "@/types/api.generated";
import { api } from "./client";

export type ReadList = components["schemas"]["ReadListDto"];
export type CreateReadListRequest =
  components["schemas"]["CreateReadListRequest"];
export type UpdateReadListRequest =
  components["schemas"]["UpdateReadListRequest"];

type ReadListListResponse = components["schemas"]["ReadListListResponse"];

/**
 * Sort for a read list's members. An explicit sort always wins; when omitted,
 * the read list's `ordered` flag picks the default (`manual` when set,
 * `release` otherwise). Matches the API's `sort` query param.
 */
export type ReadListBookSort = "release" | "title" | "added" | "manual";

/** Direction for a chosen sort; the server ignores it for `manual`. */
export type SortDirection = "asc" | "desc";

export const readListsApi = {
  /** All read lists (with each read list's visible book count). */
  list: async (): Promise<ReadList[]> => {
    const response = await api.get<ReadListListResponse>("/readlists");
    return response.data.items;
  },

  get: async (id: string): Promise<ReadList> => {
    const response = await api.get<ReadList>(`/readlists/${id}`);
    return response.data;
  },

  /**
   * Member books, filtered by the user's visibility. An explicit sort always
   * wins; otherwise the `ordered` flag picks the default order.
   */
  getBooks: async (
    id: string,
    sort?: ReadListBookSort,
    direction?: SortDirection,
  ): Promise<Book[]> => {
    const params = new URLSearchParams();
    if (sort) params.set("sort", sort);
    if (direction) params.set("direction", direction);
    const query = params.size > 0 ? `?${params}` : "";
    const response = await api.get<Book[]>(`/readlists/${id}/books${query}`);
    return response.data;
  },

  create: async (body: CreateReadListRequest): Promise<ReadList> => {
    const response = await api.post<ReadList>("/readlists", body);
    return response.data;
  },

  update: async (
    id: string,
    body: UpdateReadListRequest,
  ): Promise<ReadList> => {
    const response = await api.patch<ReadList>(`/readlists/${id}`, body);
    return response.data;
  },

  delete: async (id: string): Promise<void> => {
    await api.delete(`/readlists/${id}`);
  },

  addBooks: async (id: string, bookIds: string[]): Promise<ReadList> => {
    const response = await api.post<ReadList>(`/readlists/${id}/books`, {
      bookIds,
    });
    return response.data;
  },

  removeBook: async (id: string, bookId: string): Promise<void> => {
    await api.delete(`/readlists/${id}/books/${bookId}`);
  },

  /** Set the full manual reading order of a read list's books. */
  reorder: async (id: string, bookIds: string[]): Promise<void> => {
    await api.put(`/readlists/${id}/books`, { bookIds });
  },

  /** Read lists that contain a given book. */
  forBook: async (bookId: string): Promise<ReadList[]> => {
    const response = await api.get<ReadListListResponse>(
      `/books/${bookId}/readlists`,
    );
    return response.data.items;
  },
};
