import type { components } from "@/types/api.generated";
import { api } from "./client";

export type Tag = components["schemas"]["TagDto"];
export type TagListResponse = components["schemas"]["TagListResponse"];

export const tagsApi = {
	/**
	 * Get all tags across all libraries
	 */
	getAll: async (): Promise<Tag[]> => {
		const response = await api.get<TagListResponse>("/tags");
		return response.data.tags ?? [];
	},

	/**
	 * Get tags for a specific series
	 */
	getForSeries: async (seriesId: string): Promise<Tag[]> => {
		const response = await api.get<TagListResponse>(`/series/${seriesId}/tags`);
		return response.data.tags;
	},

	/**
	 * Set all tags for a series (replaces existing)
	 */
	setForSeries: async (seriesId: string, tags: string[]): Promise<Tag[]> => {
		const response = await api.put<TagListResponse>(
			`/series/${seriesId}/tags`,
			{ tags },
		);
		return response.data.tags;
	},

	/**
	 * Add a single tag to a series
	 */
	addToSeries: async (seriesId: string, name: string): Promise<Tag> => {
		const response = await api.post<Tag>(`/series/${seriesId}/tags`, { name });
		return response.data;
	},

	/**
	 * Remove a tag from a series
	 */
	removeFromSeries: async (seriesId: string, tagId: string): Promise<void> => {
		await api.delete(`/series/${seriesId}/tags/${tagId}`);
	},

	/**
	 * Delete a tag globally
	 */
	delete: async (tagId: string): Promise<void> => {
		await api.delete(`/tags/${tagId}`);
	},

	/**
	 * Clean up unused tags (admin only)
	 */
	cleanup: async (): Promise<{ deleted_count: number }> => {
		const response = await api.post<{ deleted_count: number }>("/tags/cleanup");
		return response.data;
	},
};
