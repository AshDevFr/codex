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
 * Sort for an unordered read list's members (ignored by the server when the
 * read list is manually ordered). Matches the API's `sort` query param.
 */
export type ReadListBookSort = "release" | "title" | "added";

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
   * Member books, filtered by the user's visibility. Ordered read lists
   * return manual reading order; unordered ones honor `sort` (release date
   * by default).
   */
  getBooks: async (id: string, sort?: ReadListBookSort): Promise<Book[]> => {
    const query = sort ? `?sort=${sort}` : "";
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
