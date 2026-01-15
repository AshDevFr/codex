import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type SettingDto = components["schemas"]["SettingDto"];
export type SettingHistoryDto = components["schemas"]["SettingHistoryDto"];
export type UpdateSettingRequest =
	components["schemas"]["UpdateSettingRequest"];
export type BulkUpdateSettingsRequest =
	components["schemas"]["BulkUpdateSettingsRequest"];
// Bulk update returns an array of SettingDto
export type BulkUpdateSettingsResponse = SettingDto[];

// Public setting type (simplified, for non-admin users)
export interface PublicSettingDto {
	key: string;
	value: string;
}

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
};
