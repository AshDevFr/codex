import type { components } from "@/types/api.generated";
import { api } from "./client";

export type FullSeriesMetadata =
	components["schemas"]["FullSeriesMetadataResponse"];
export type SeriesMetadataResponse =
	components["schemas"]["SeriesMetadataResponse"];
export type MetadataLocks = components["schemas"]["MetadataLocks"];
export type UpdateMetadataLocksRequest =
	components["schemas"]["UpdateMetadataLocksRequest"];
export type AlternateTitle = components["schemas"]["AlternateTitleDto"];
export type ExternalRating = components["schemas"]["ExternalRatingDto"];
export type ExternalLink = components["schemas"]["ExternalLinkDto"];
export type SeriesCover = components["schemas"]["SeriesCoverDto"];

export const seriesMetadataApi = {
	/**
	 * Get full metadata for a series including all related data
	 * (genres, tags, alternate titles, external ratings, external links, locks)
	 */
	getFullMetadata: async (seriesId: string): Promise<FullSeriesMetadata> => {
		const response = await api.get<FullSeriesMetadata>(
			`/series/${seriesId}/metadata/full`,
		);
		return response.data;
	},

	/**
	 * Get metadata lock states for a series
	 */
	getLocks: async (seriesId: string): Promise<MetadataLocks> => {
		const response = await api.get<MetadataLocks>(
			`/series/${seriesId}/metadata/locks`,
		);
		return response.data;
	},

	/**
	 * Update metadata lock states for a series
	 */
	updateLocks: async (
		seriesId: string,
		locks: Partial<MetadataLocks>,
	): Promise<MetadataLocks> => {
		const response = await api.put<MetadataLocks>(
			`/series/${seriesId}/metadata/locks`,
			locks,
		);
		return response.data;
	},

	/**
	 * Replace all metadata for a series (PUT)
	 */
	replaceMetadata: async (
		seriesId: string,
		metadata: components["schemas"]["ReplaceSeriesMetadataRequest"],
	): Promise<SeriesMetadataResponse> => {
		const response = await api.put<SeriesMetadataResponse>(
			`/series/${seriesId}/metadata`,
			metadata,
		);
		return response.data;
	},

	/**
	 * Partially update metadata for a series (PATCH)
	 */
	patchMetadata: async (
		seriesId: string,
		metadata: components["schemas"]["PatchSeriesMetadataRequest"],
	): Promise<SeriesMetadataResponse> => {
		const response = await api.patch<SeriesMetadataResponse>(
			`/series/${seriesId}/metadata`,
			metadata,
		);
		return response.data;
	},

	// Alternate titles
	getAlternateTitles: async (seriesId: string): Promise<AlternateTitle[]> => {
		const response =
			await api.get<components["schemas"]["AlternateTitleListResponse"]>(
				`/series/${seriesId}/alternate-titles`,
			);
		return response.data.titles;
	},

	createAlternateTitle: async (
		seriesId: string,
		title: string,
		label: string,
	): Promise<AlternateTitle> => {
		const response = await api.post<AlternateTitle>(
			`/series/${seriesId}/alternate-titles`,
			{ title, label },
		);
		return response.data;
	},

	updateAlternateTitle: async (
		seriesId: string,
		titleId: string,
		title?: string,
		label?: string,
	): Promise<AlternateTitle> => {
		const response = await api.put<AlternateTitle>(
			`/series/${seriesId}/alternate-titles/${titleId}`,
			{ title, label },
		);
		return response.data;
	},

	deleteAlternateTitle: async (
		seriesId: string,
		titleId: string,
	): Promise<void> => {
		await api.delete(`/series/${seriesId}/alternate-titles/${titleId}`);
	},

	// External ratings
	getExternalRatings: async (seriesId: string): Promise<ExternalRating[]> => {
		const response =
			await api.get<components["schemas"]["ExternalRatingListResponse"]>(
				`/series/${seriesId}/external-ratings`,
			);
		return response.data.ratings;
	},

	createExternalRating: async (
		seriesId: string,
		sourceName: string,
		rating: number,
		voteCount?: number,
	): Promise<ExternalRating> => {
		const response = await api.post<ExternalRating>(
			`/series/${seriesId}/external-ratings`,
			{ source_name: sourceName, rating, vote_count: voteCount },
		);
		return response.data;
	},

	deleteExternalRating: async (
		seriesId: string,
		ratingId: string,
	): Promise<void> => {
		await api.delete(`/series/${seriesId}/external-ratings/${ratingId}`);
	},

	// External links
	getExternalLinks: async (seriesId: string): Promise<ExternalLink[]> => {
		const response =
			await api.get<components["schemas"]["ExternalLinkListResponse"]>(
				`/series/${seriesId}/external-links`,
			);
		return response.data.links;
	},

	createExternalLink: async (
		seriesId: string,
		sourceName: string,
		url: string,
		externalId?: string,
	): Promise<ExternalLink> => {
		const response = await api.post<ExternalLink>(
			`/series/${seriesId}/external-links`,
			{ source_name: sourceName, url, external_id: externalId },
		);
		return response.data;
	},

	deleteExternalLink: async (
		seriesId: string,
		linkId: string,
	): Promise<void> => {
		await api.delete(`/series/${seriesId}/external-links/${linkId}`);
	},

	// Cover management
	listCovers: async (seriesId: string): Promise<SeriesCover[]> => {
		const response =
			await api.get<components["schemas"]["SeriesCoverListResponse"]>(
				`/series/${seriesId}/covers`,
			);
		return response.data.covers;
	},

	selectCover: async (seriesId: string, coverId: string): Promise<void> => {
		await api.put(`/series/${seriesId}/covers/${coverId}/select`);
	},

	deleteCover: async (seriesId: string, coverId: string): Promise<void> => {
		await api.delete(`/series/${seriesId}/covers/${coverId}`);
	},
};
