import type { FullSeries, FullSeriesMetadata } from "@/types";

/**
 * Simplified metadata type for use in templates.
 *
 * This type provides a flattened, template-friendly view of series metadata.
 * Complex nested DTOs are simplified to make template authoring easier:
 * - genres/tags are arrays of strings (just names, not full objects)
 * - externalRatings/externalLinks are simplified objects
 * - alternateTitles are simplified objects
 */
export interface MetadataForTemplate {
	/** Series title */
	title: string;
	/** Series summary/description */
	summary: string | null;
	/** Publisher name */
	publisher: string | null;
	/** Imprint (sub-publisher) */
	imprint: string | null;
	/** Publication year */
	year: number | null;
	/** Age rating (e.g., 13, 16, 18) */
	ageRating: number | null;
	/** Language code (BCP47 format) */
	language: string | null;
	/** Series status (ongoing, ended, hiatus, abandoned, unknown) */
	status: string | null;
	/** Reading direction (ltr, rtl, ttb, webtoon) */
	readingDirection: string | null;
	/** Expected total book count */
	totalBookCount: number | null;
	/** Custom sort name */
	titleSort: string | null;
	/** Genre names as a simple array of strings */
	genres: string[];
	/** Tag names as a simple array of strings */
	tags: string[];
	/** External ratings from various sources */
	externalRatings: Array<{ source: string; rating: number; votes?: number }>;
	/** External links to other sites */
	externalLinks: Array<{ source: string; url: string; externalId?: string }>;
	/** Alternate titles for this series */
	alternateTitles: Array<{ title: string; label: string }>;
}

/**
 * Sample metadata for template testing in the editor.
 * This provides realistic mock data matching the MetadataForTemplate structure.
 */
export const SAMPLE_METADATA_FOR_TEMPLATE: MetadataForTemplate = {
	title: "Attack on Titan",
	summary:
		"Humanity lives inside cities surrounded by enormous walls due to the Titans, gigantic humanoid creatures who devour humans seemingly without reason.",
	publisher: "Kodansha",
	imprint: "Bessatsu Shōnen Magazine",
	year: 2009,
	ageRating: 16,
	language: "ja",
	status: "ended",
	readingDirection: "rtl",
	totalBookCount: 34,
	titleSort: "Attack on Titan",
	genres: ["Action", "Dark Fantasy", "Post-Apocalyptic"],
	tags: ["manga", "titans", "survival", "military"],
	externalRatings: [
		{ source: "MyAnimeList", rating: 8.54, votes: 1250000 },
		{ source: "AniList", rating: 84, votes: 890000 },
	],
	externalLinks: [
		{ source: "MyAnimeList", url: "https://myanimelist.net/manga/23390" },
		{
			source: "AniList",
			url: "https://anilist.co/manga/53390",
			externalId: "53390",
		},
	],
	alternateTitles: [
		{ title: "Shingeki no Kyojin", label: "Romaji" },
		{ title: "進撃の巨人", label: "Native" },
	],
};

/**
 * Transforms a FullSeriesMetadata response into a simplified MetadataForTemplate object.
 *
 * This transformation:
 * - Extracts scalar fields directly
 * - Simplifies genres and tags to arrays of names
 * - Simplifies external ratings, links, and alternate titles to clean objects
 * - Omits internal fields like IDs, timestamps, and locks
 *
 * @param metadata - The full series metadata from the API
 * @returns A simplified metadata object for template rendering
 */
export function transformToMetadataForTemplate(
	metadata: FullSeriesMetadata,
): MetadataForTemplate {
	return {
		// Scalar fields - pass through directly
		title: metadata.title,
		summary: metadata.summary ?? null,
		publisher: metadata.publisher ?? null,
		imprint: metadata.imprint ?? null,
		year: metadata.year ?? null,
		ageRating: metadata.ageRating ?? null,
		language: metadata.language ?? null,
		status: metadata.status ?? null,
		readingDirection: metadata.readingDirection ?? null,
		totalBookCount: metadata.totalBookCount ?? null,
		titleSort: metadata.titleSort ?? null,

		// Simplify genres to just names
		genres: metadata.genres.map((genre) => genre.name),

		// Simplify tags to just names
		tags: metadata.tags.map((tag) => tag.name),

		// Simplify external ratings
		externalRatings: metadata.externalRatings.map((rating) => ({
			source: rating.sourceName,
			rating: rating.rating,
			...(rating.voteCount !== undefined &&
				rating.voteCount !== null && { votes: rating.voteCount }),
		})),

		// Simplify external links
		externalLinks: metadata.externalLinks.map((link) => ({
			source: link.sourceName,
			url: link.url,
			...(link.externalId && { externalId: link.externalId }),
		})),

		// Simplify alternate titles
		alternateTitles: metadata.alternateTitles.map((alt) => ({
			title: alt.title,
			label: alt.label,
		})),
	};
}

/**
 * Transforms a FullSeries response (FullSeriesResponse) into a MetadataForTemplate object.
 *
 * This handles the nested structure of FullSeriesResponse where:
 * - Scalar metadata fields are in `series.metadata`
 * - Arrays (genres, tags, etc.) are at the top level of the response
 *
 * @param series - The full series response from the API
 * @returns A simplified metadata object for template rendering
 */
export function transformFullSeriesToMetadataForTemplate(
	series: FullSeries,
): MetadataForTemplate {
	const metadata = series.metadata;

	return {
		// Scalar fields from nested metadata
		title: metadata.title,
		summary: metadata.summary ?? null,
		publisher: metadata.publisher ?? null,
		imprint: metadata.imprint ?? null,
		year: metadata.year ?? null,
		ageRating: metadata.ageRating ?? null,
		language: metadata.language ?? null,
		status: metadata.status ?? null,
		readingDirection: metadata.readingDirection ?? null,
		totalBookCount: metadata.totalBookCount ?? null,
		titleSort: metadata.titleSort ?? null,

		// Arrays from top-level of FullSeriesResponse
		genres: series.genres.map((genre) => genre.name),
		tags: series.tags.map((tag) => tag.name),

		externalRatings: series.externalRatings.map((rating) => ({
			source: rating.sourceName,
			rating: rating.rating,
			...(rating.voteCount !== undefined &&
				rating.voteCount !== null && { votes: rating.voteCount }),
		})),

		externalLinks: series.externalLinks.map((link) => ({
			source: link.sourceName,
			url: link.url,
			...(link.externalId && { externalId: link.externalId }),
		})),

		alternateTitles: series.alternateTitles.map((alt) => ({
			title: alt.title,
			label: alt.label,
		})),
	};
}
