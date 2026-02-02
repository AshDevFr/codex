import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated type for convenience
export type AppInfoDto = components["schemas"]["AppInfoDto"];

export const infoApi = {
  /**
   * Get application info (public, no authentication required)
   */
  getInfo: async (): Promise<AppInfoDto> => {
    const response = await api.get<AppInfoDto>("/info");
    return response.data;
  },
};
