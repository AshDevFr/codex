import type { components } from "@/types/api.generated";
import { api } from "./client";

export type NewsResponse = components["schemas"]["NewsResponse"];
export type NewsEntryDto = components["schemas"]["NewsEntryDto"];
export type NewsUpdateAvailableDto =
  components["schemas"]["NewsUpdateAvailableDto"];

export const newsApi = {
  /**
   * Codex release and Codex Reader news for the calling admin (admin only).
   */
  get: async (): Promise<NewsResponse> => {
    const response = await api.get<NewsResponse>("/news");
    return response.data;
  },

  /**
   * Mark everything published so far as seen by the calling admin.
   */
  markSeen: async (): Promise<void> => {
    await api.post("/news/seen");
  },
};
