import type { components } from "@/types/api.generated";
import { api } from "./client";

// Re-export generated types for convenience
export type SharingTagDto = components["schemas"]["SharingTagDto"];
export type SharingTagSummaryDto = components["schemas"]["SharingTagSummaryDto"];
export type SharingTagListResponse =
	components["schemas"]["SharingTagListResponse"];
export type CreateSharingTagRequest =
	components["schemas"]["CreateSharingTagRequest"];
export type UpdateSharingTagRequest =
	components["schemas"]["UpdateSharingTagRequest"];
export type SetSeriesSharingTagsRequest =
	components["schemas"]["SetSeriesSharingTagsRequest"];
export type ModifySeriesSharingTagRequest =
	components["schemas"]["ModifySeriesSharingTagRequest"];
export type SetUserSharingTagGrantRequest =
	components["schemas"]["SetUserSharingTagGrantRequest"];
export type UserSharingTagGrantDto =
	components["schemas"]["UserSharingTagGrantDto"];
export type UserSharingTagGrantsResponse =
	components["schemas"]["UserSharingTagGrantsResponse"];
export type AccessMode = components["schemas"]["AccessMode"];

export const sharingTagsApi = {
	// ============================================
	// Admin CRUD operations (admin only)
	// ============================================

	/**
	 * List all sharing tags (admin only)
	 */
	list: async (): Promise<SharingTagDto[]> => {
		const response = await api.get<SharingTagListResponse>(
			"/admin/sharing-tags",
		);
		return response.data.items;
	},

	/**
	 * Get a single sharing tag by ID (admin only)
	 */
	get: async (tagId: string): Promise<SharingTagDto> => {
		const response = await api.get<SharingTagDto>(
			`/admin/sharing-tags/${tagId}`,
		);
		return response.data;
	},

	/**
	 * Create a new sharing tag (admin only)
	 */
	create: async (request: CreateSharingTagRequest): Promise<SharingTagDto> => {
		const response = await api.post<SharingTagDto>(
			"/admin/sharing-tags",
			request,
		);
		return response.data;
	},

	/**
	 * Update a sharing tag (admin only)
	 */
	update: async (
		tagId: string,
		request: UpdateSharingTagRequest,
	): Promise<SharingTagDto> => {
		const response = await api.patch<SharingTagDto>(
			`/admin/sharing-tags/${tagId}`,
			request,
		);
		return response.data;
	},

	/**
	 * Delete a sharing tag (admin only)
	 */
	delete: async (tagId: string): Promise<void> => {
		await api.delete(`/admin/sharing-tags/${tagId}`);
	},

	// ============================================
	// Series sharing tag operations
	// ============================================

	/**
	 * Get sharing tags for a series (admins only, maintainers can view but not modify)
	 */
	getForSeries: async (seriesId: string): Promise<SharingTagSummaryDto[]> => {
		const response = await api.get<SharingTagSummaryDto[]>(
			`/series/${seriesId}/sharing-tags`,
		);
		return response.data;
	},

	/**
	 * Set all sharing tags for a series (replaces existing)
	 */
	setForSeries: async (
		seriesId: string,
		sharingTagIds: string[],
	): Promise<SharingTagSummaryDto[]> => {
		const response = await api.put<SharingTagSummaryDto[]>(
			`/series/${seriesId}/sharing-tags`,
			{ sharingTagIds } satisfies SetSeriesSharingTagsRequest,
		);
		return response.data;
	},

	/**
	 * Add a single sharing tag to a series
	 */
	addToSeries: async (
		seriesId: string,
		sharingTagId: string,
	): Promise<SharingTagSummaryDto[]> => {
		const response = await api.post<SharingTagSummaryDto[]>(
			`/series/${seriesId}/sharing-tags`,
			{ sharingTagId } satisfies ModifySeriesSharingTagRequest,
		);
		return response.data;
	},

	/**
	 * Remove a sharing tag from a series
	 */
	removeFromSeries: async (
		seriesId: string,
		sharingTagId: string,
	): Promise<void> => {
		await api.delete(`/series/${seriesId}/sharing-tags/${sharingTagId}`);
	},

	// ============================================
	// User sharing tag grant operations
	// ============================================

	/**
	 * Get sharing tag grants for a user (admin only)
	 */
	getGrantsForUser: async (
		userId: string,
	): Promise<UserSharingTagGrantsResponse> => {
		const response = await api.get<UserSharingTagGrantsResponse>(
			`/users/${userId}/sharing-tags`,
		);
		return response.data;
	},

	/**
	 * Set a sharing tag grant for a user (creates or updates)
	 */
	setGrantForUser: async (
		userId: string,
		sharingTagId: string,
		accessMode: AccessMode,
	): Promise<UserSharingTagGrantDto> => {
		const response = await api.put<UserSharingTagGrantDto>(
			`/users/${userId}/sharing-tags`,
			{ sharingTagId, accessMode } satisfies SetUserSharingTagGrantRequest,
		);
		return response.data;
	},

	/**
	 * Remove a sharing tag grant from a user
	 */
	removeGrantFromUser: async (
		userId: string,
		sharingTagId: string,
	): Promise<void> => {
		await api.delete(`/users/${userId}/sharing-tags/${sharingTagId}`);
	},

	/**
	 * Get current user's sharing tag grants
	 */
	getMyGrants: async (): Promise<UserSharingTagGrantsResponse> => {
		const response = await api.get<UserSharingTagGrantsResponse>(
			"/user/sharing-tags",
		);
		return response.data;
	},
};
