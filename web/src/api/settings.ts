import axios from "axios";
import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type SettingDto = components["schemas"]["SettingDto"];
export type SettingHistoryDto = components["schemas"]["SettingHistoryDto"];
export type UpdateSettingRequest =
  components["schemas"]["UpdateSettingRequest"];
export type BulkUpdateSettingsRequest =
  components["schemas"]["BulkUpdateSettingsRequest"];
export type BrandingSettingsDto = components["schemas"]["BrandingSettingsDto"];
// Bulk update returns an array of SettingDto
export type BulkUpdateSettingsResponse = SettingDto[];

// Re-export generated public setting type
export type PublicSettingDto = components["schemas"]["PublicSettingDto"];

// Map of setting key to public setting
export type PublicSettingsMap = Record<string, PublicSettingDto>;

export const settingsApi = {
  /**
   * List all settings (admin only)
   */
  list: async (): Promise<SettingDto[]> => {
    const response = await api.get<SettingDto[]>("/admin/settings");
    return response.data;
  },

  /**
   * Get a single setting by key (admin only)
   */
  get: async (key: string): Promise<SettingDto> => {
    const response = await api.get<SettingDto>(
      `/admin/settings/${encodeURIComponent(key)}`,
    );
    return response.data;
  },

  /**
   * Update a single setting (admin only)
   */
  update: async (
    key: string,
    request: UpdateSettingRequest,
  ): Promise<SettingDto> => {
    const response = await api.put<SettingDto>(
      `/admin/settings/${encodeURIComponent(key)}`,
      request,
    );
    return response.data;
  },

  /**
   * Bulk update multiple settings (admin only)
   */
  bulkUpdate: async (
    request: BulkUpdateSettingsRequest,
  ): Promise<BulkUpdateSettingsResponse> => {
    const response = await api.post<BulkUpdateSettingsResponse>(
      "/admin/settings/bulk",
      request,
    );
    return response.data;
  },

  /**
   * Reset a setting to its default value (admin only)
   */
  reset: async (key: string): Promise<SettingDto> => {
    const response = await api.post<SettingDto>(
      `/admin/settings/${encodeURIComponent(key)}/reset`,
    );
    return response.data;
  },

  /**
   * Get setting history (admin only)
   */
  getHistory: async (key: string): Promise<SettingHistoryDto[]> => {
    const response = await api.get<SettingHistoryDto[]>(
      `/admin/settings/${encodeURIComponent(key)}/history`,
    );
    return response.data;
  },

  /**
   * Get public display settings (all authenticated users)
   * Returns non-sensitive settings that affect UI/display behavior
   */
  getPublicSettings: async (): Promise<PublicSettingsMap> => {
    const response = await api.get<PublicSettingsMap>("/settings/public");
    return response.data;
  },

  /**
   * Get branding settings (unauthenticated)
   * Returns branding-related settings needed on login page and other
   * unauthenticated UI surfaces.
   */
  getBranding: async (): Promise<BrandingSettingsDto> => {
    // Use axios directly without auth interceptor since this is unauthenticated
    const response = await axios.get<BrandingSettingsDto>(
      "/api/v1/settings/branding",
    );
    return response.data;
  },
};
