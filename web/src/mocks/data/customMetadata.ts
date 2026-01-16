/**
 * Mock custom metadata fixtures for development and testing
 *
 * These fixtures provide realistic sample data that can be used with
 * the custom metadata editor and template display system.
 */

/**
 * Example custom metadata for a manga series with reading progress
 */
export const MANGA_READING_PROGRESS = {
	status: "In Progress",
	priority: 8,
	rating: 9,
	started_date: "2024-01-15",
	current_volume: 5,
	notes: "Currently on the tournament arc. Great action sequences!",
	tags: ["action", "shonen", "favorite"],
};

/**
 * Example custom metadata for a completed comic series
 */
export const COMPLETED_COMIC = {
	status: "Completed",
	rating: 10,
	started_date: "2023-06-01",
	completed_date: "2023-12-20",
	review:
		"An absolute masterpiece. The storytelling and art are top-tier. This series redefined the genre.",
	times_read: 3,
	tags: ["favorite", "classic", "recommended"],
};

/**
 * Example custom metadata with external links
 */
export const EXTERNAL_LINKS_METADATA = {
	links: [
		{ name: "MyAnimeList", url: "https://myanimelist.net/manga/2" },
		{ name: "AniList", url: "https://anilist.co/manga/30002" },
		{
			name: "MangaUpdates",
			url: "https://www.mangaupdates.com/series/example",
		},
		{ name: "Official Site", url: "https://example.com/series" },
	],
	ids: {
		mal_id: "2",
		anilist_id: "30002",
		mangaupdates_id: "example123",
		isbn: "978-1-23456-789-0",
	},
};

/**
 * Example custom metadata for a physical collection
 */
export const PHYSICAL_COLLECTION = {
	format: "Hardcover Omnibus",
	edition: "Deluxe Edition",
	condition: "Near Mint",
	purchase_date: "2024-03-15",
	purchase_price: 49.99,
	location: "Bookshelf A, Row 3",
	signed: true,
	notes: "Signed by the author at NYCC 2024. Includes exclusive cover.",
};

/**
 * Example custom metadata with nested structure
 */
export const NESTED_METADATA = {
	acquisition: {
		source: "Digital Purchase",
		platform: "Amazon Kindle",
		date: "2024-02-28",
		price: 12.99,
	},
	reading: {
		status: "Reading",
		progress: 45,
		started: "2024-03-01",
		sessions: [
			{ date: "2024-03-01", chapters: "1-5" },
			{ date: "2024-03-05", chapters: "6-10" },
		],
	},
	notes: {
		general: "Great artwork and pacing",
		highlights: [
			"Amazing cliffhanger in chapter 8",
			"Character development in chapter 10",
		],
	},
};

/**
 * Example custom metadata for tracking borrowed items
 */
export const BORROWED_ITEM = {
	status: "Borrowed",
	borrowed_from: "City Library",
	borrowed_date: "2024-04-01",
	due_date: "2024-04-22",
	renewal_count: 0,
	notes: "Check for digital availability",
};

/**
 * Example custom metadata for wishlist items
 */
export const WISHLIST_METADATA = {
	status: "Wishlist",
	priority: 7,
	estimated_price: 35.0,
	preferred_format: "Paperback",
	available_at: ["Amazon", "Barnes & Noble", "Local Comic Shop"],
	notes: "Wait for sale or use rewards points",
};

/**
 * Minimal custom metadata example
 */
export const MINIMAL_METADATA = {
	rating: 8,
	status: "Completed",
};

/**
 * Empty/null custom metadata for testing edge cases
 */
export const EMPTY_METADATA = {};

/**
 * All mock custom metadata fixtures mapped by name
 */
export const CUSTOM_METADATA_FIXTURES: Record<
	string,
	Record<string, unknown>
> = {
	manga_reading_progress: MANGA_READING_PROGRESS,
	completed_comic: COMPLETED_COMIC,
	external_links: EXTERNAL_LINKS_METADATA,
	physical_collection: PHYSICAL_COLLECTION,
	nested_structure: NESTED_METADATA,
	borrowed_item: BORROWED_ITEM,
	wishlist: WISHLIST_METADATA,
	minimal: MINIMAL_METADATA,
	empty: EMPTY_METADATA,
};

/**
 * Get a random custom metadata fixture (for mock data generation)
 */
export function getRandomCustomMetadata(): Record<string, unknown> | null {
	const fixtures = Object.values(CUSTOM_METADATA_FIXTURES).filter(
		(f) => Object.keys(f).length > 0,
	);
	const randomIndex = Math.floor(Math.random() * fixtures.length);
	// 30% chance of returning null to simulate series without custom metadata
	if (Math.random() < 0.3) {
		return null;
	}
	return fixtures[randomIndex];
}

/**
 * Example metadata that works well with the default template
 */
export const DEFAULT_TEMPLATE_EXAMPLE: Record<string, unknown> = {
	source: "Scanned from physical copy",
	edition: "First Edition",
	condition: "Excellent",
	notes: "Part of the original print run",
};

/**
 * Example metadata that works well with the reading list template
 */
export const READING_LIST_EXAMPLE: Record<string, unknown> = {
	status: "In Progress",
	priority: 9,
	rating: 8,
	started_date: "2024-01-15",
	notes: "Loving the character development in this series!",
};

/**
 * Example metadata that works well with the external links template
 */
export const EXTERNAL_LINKS_EXAMPLE: Record<string, unknown> = {
	links: [
		{ name: "MyAnimeList", url: "https://myanimelist.net/" },
		{ name: "AniList", url: "https://anilist.co/" },
		{ name: "Wikipedia", url: "https://wikipedia.org/" },
	],
	ids: {
		mal_id: "12345",
		isbn: "978-0-123456-78-9",
	},
};
