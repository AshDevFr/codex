/**
 * Series API mock handlers
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import {
	COMPLETED_COMIC,
	EXTERNAL_LINKS_METADATA,
	MANGA_READING_PROGRESS,
	MINIMAL_METADATA,
} from "../data/customMetadata";
import { createPaginatedResponse, seriesSummaries } from "../data/factories";
import { getSeriesByLibrary, mockSeries } from "../data/store";
import coverSvg from "../fixtures/cover.svg?raw";

type ExternalRatingDto = components["schemas"]["ExternalRatingDto"];
type SeriesExternalIdDto = components["schemas"]["SeriesExternalIdDto"];

/**
 * Sample custom metadata for specific series (by title match)
 * These demonstrate the custom metadata feature in development mode
 */
const SERIES_CUSTOM_METADATA: Record<string, Record<string, unknown> | null> = {
	"One Piece": MANGA_READING_PROGRESS,
	"Batman: Year One": COMPLETED_COMIC,
	"Attack on Titan": EXTERNAL_LINKS_METADATA,
	Saga: MINIMAL_METADATA,
	// All other series will have null (no custom metadata)
};

function getExternalRatingsForSeries(
	seriesId: string,
	title: string,
): ExternalRatingDto[] {
	const now = new Date().toISOString();

	// Define external ratings for popular series
	const externalRatingsData: Record<
		string,
		{ sourceName: string; rating: number; voteCount: number }[]
	> = {
		"One Piece": [
			{ sourceName: "myanimelist", rating: 90, voteCount: 450000 },
			{ sourceName: "anilist", rating: 88, voteCount: 120000 },
		],
		"Attack on Titan": [
			{ sourceName: "myanimelist", rating: 85, voteCount: 380000 },
			{ sourceName: "anilist", rating: 84, voteCount: 95000 },
			{ sourceName: "kitsu", rating: 86, voteCount: 42000 },
		],
		Naruto: [
			{ sourceName: "myanimelist", rating: 82, voteCount: 320000 },
			{ sourceName: "anilist", rating: 80, voteCount: 78000 },
		],
		"My Hero Academia": [
			{ sourceName: "myanimelist", rating: 78, voteCount: 180000 },
			{ sourceName: "anilist", rating: 76, voteCount: 55000 },
		],
		"Demon Slayer": [
			{ sourceName: "myanimelist", rating: 86, voteCount: 220000 },
			{ sourceName: "anilist", rating: 85, voteCount: 68000 },
		],
	};

	const ratings = externalRatingsData[title];
	if (!ratings) return [];

	return ratings.map((r, index) => ({
		id: `ext-rating-${seriesId}-${index}`,
		seriesId,
		sourceName: r.sourceName,
		rating: r.rating,
		voteCount: r.voteCount,
		fetchedAt: now,
		createdAt: now,
		updatedAt: now,
	}));
}

/**
 * Get external IDs for a series (from metadata providers like plugins)
 */
function getExternalIdsForSeries(
	seriesId: string,
	title: string,
): SeriesExternalIdDto[] {
	const now = new Date().toISOString();

	// Define external IDs for popular series (simulating plugin matches)
	const externalIdsData: Record<
		string,
		{ source: string; externalId: string; externalUrl?: string }[]
	> = {
		"One Piece": [
			{
				source: "plugin:mangabaka",
				externalId: "13",
				externalUrl: "https://www.mangaupdates.com/series/13",
			},
		],
		"Attack on Titan": [
			{
				source: "plugin:mangabaka",
				externalId: "23390",
				externalUrl: "https://www.mangaupdates.com/series/23390",
			},
		],
		Naruto: [
			{
				source: "plugin:mangabaka",
				externalId: "11",
				externalUrl: "https://www.mangaupdates.com/series/11",
			},
		],
		"Batman: Year One": [
			{
				source: "plugin:comicvine",
				externalId: "4050-4889",
				externalUrl:
					"https://comicvine.gamespot.com/batman-year-one/4050-4889/",
			},
		],
	};

	const ids = externalIdsData[title];
	if (!ids) return [];

	return ids.map((item, index) => ({
		id: `ext-id-${seriesId}-${index}`,
		seriesId,
		source: item.source,
		externalId: item.externalId,
		externalUrl: item.externalUrl || null,
		lastSyncedAt: now,
		metadataHash: null,
		createdAt: now,
		updatedAt: now,
	}));
}

/**
 * Community average ratings (simulated user ratings)
 */
const SERIES_COMMUNITY_RATINGS: Record<
	string,
	{ average: number; count: number }
> = {
	"One Piece": { average: 92, count: 47 },
	"Attack on Titan": { average: 88, count: 35 },
	Naruto: { average: 78, count: 28 },
	"Batman: Year One": { average: 85, count: 15 },
	Watchmen: { average: 90, count: 22 },
	Saga: { average: 87, count: 18 },
	"My Hero Academia": { average: 75, count: 12 },
	"Demon Slayer": { average: 82, count: 25 },
	"Spider-Man: Blue": { average: 80, count: 8 },
};

/**
 * Get custom metadata for a series by title
 */
function getCustomMetadataForSeries(
	title: string,
): Record<string, unknown> | null {
	return SERIES_CUSTOM_METADATA[title] ?? null;
}

/**
 * Get a detailed summary for a series
 */
function getSeriesSummary(title: string): string {
	return (
		seriesSummaries[title] ||
		`${title} is a compelling series that captivates readers with its intricate plot and memorable characters. The story weaves together themes of adventure, personal growth, and the eternal struggle between good and evil, creating an unforgettable reading experience that resonates with fans across generations.`
	);
}

/**
 * Publisher mapping for series
 */
const seriesPublishers: Record<string, string> = {
	"Batman: Year One": "DC Comics",
	"Batman: The Dark Knight Returns": "DC Comics",
	"Spider-Man: Blue": "Marvel Comics",
	"Amazing Spider-Man": "Marvel Comics",
	"One Piece": "Shueisha / Viz Media",
	Naruto: "Shueisha / Viz Media",
	"Attack on Titan": "Kodansha",
	"My Hero Academia": "Shueisha / Viz Media",
	"Demon Slayer": "Shueisha / Viz Media",
	Saga: "Image Comics",
	"The Walking Dead": "Image Comics",
	Sandman: "DC Comics / Vertigo",
	Watchmen: "DC Comics",
	Dune: "Ace Books",
	"The Expanse": "Orbit Books",
	Foundation: "Gnome Press",
};

/**
 * Genre mapping for series
 */
const seriesGenres: Record<string, string[]> = {
	"Batman: Year One": ["Superhero", "Crime", "Noir"],
	"One Piece": ["Action", "Adventure", "Comedy", "Fantasy"],
	"Attack on Titan": ["Dark Fantasy", "Action", "Post-Apocalyptic"],
	Saga: ["Science Fiction", "Fantasy", "Drama", "Romance"],
	"The Walking Dead": ["Horror", "Drama", "Post-Apocalyptic"],
	Sandman: ["Fantasy", "Horror", "Mythology"],
	Naruto: ["Action", "Adventure", "Martial Arts"],
	Dune: ["Science Fiction", "Political Drama", "Adventure"],
};

/**
 * Tags for series
 */
const seriesTags: Record<string, string[]> = {
	"Batman: Year One": [
		"origin story",
		"street-level",
		"crime drama",
		"classic",
	],
	"One Piece": [
		"pirates",
		"adventure",
		"friendship",
		"world-building",
		"long-running",
	],
	"Attack on Titan": [
		"titans",
		"military",
		"mystery",
		"plot twists",
		"completed",
	],
	Saga: ["space opera", "family", "war", "mature themes"],
	"The Walking Dead": ["zombies", "survival", "community", "completed"],
	Sandman: ["mythology", "literary", "endless", "dreams"],
};

export const seriesHandlers = [
	// IMPORTANT: Specific routes MUST come before parameterized routes
	// Otherwise /api/v1/series/:id will match "started" or "search" as an ID

	// Search series (GET - legacy)
	http.get("/api/v1/series/search", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const query = url.searchParams.get("q")?.toLowerCase() || "";
		const libraryId = url.searchParams.get("libraryId");

		let results = mockSeries.filter((s) =>
			s.title.toLowerCase().includes(query),
		);

		if (libraryId) {
			results = results.filter((s) => s.libraryId === libraryId);
		}

		return HttpResponse.json(
			createPaginatedResponse(results.slice(0, 20), {
				total: results.length,
			}),
		);
	}),

	// Search series (POST - new API)
	http.post("/api/v1/series/search", async ({ request }) => {
		await delay(200);
		const body = (await request.json()) as {
			query: string;
			libraryId?: string;
		};

		const query = body.query?.toLowerCase() || "";
		let results = mockSeries.filter((s) =>
			s.title.toLowerCase().includes(query),
		);

		if (body.libraryId) {
			results = results.filter((s) => s.libraryId === body.libraryId);
		}

		// Return array directly (not paginated) - matches backend API
		return HttpResponse.json(results.slice(0, 20));
	}),

	// POST /series/list - Advanced filtering with condition tree (1-indexed)
	// Pagination params come from query string, filter criteria from body
	// Supports ?full=true for full series response with metadata
	http.post("/api/v1/series/list", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const page = Math.max(
			1,
			Number.parseInt(url.searchParams.get("page") || "1", 10),
		);
		const pageSize = Number.parseInt(
			url.searchParams.get("pageSize") || "50",
			10,
		);
		const sort = url.searchParams.get("sort");
		const full = url.searchParams.get("full") === "true";

		const body = (await request.json()) as {
			condition?: unknown;
			fullTextSearch?: string;
		};

		// For mock purposes, we'll do basic filtering
		// In a real implementation, the backend evaluates the full condition tree
		let results = [...mockSeries];

		// Apply basic library filtering if condition contains libraryId
		if (body.condition && typeof body.condition === "object") {
			const condition = body.condition as Record<string, unknown>;

			// Handle direct libraryId condition
			if ("libraryId" in condition) {
				const libOp = condition.libraryId as {
					operator: string;
					value: string;
				};
				if (libOp.operator === "is") {
					results = results.filter((s) => s.libraryId === libOp.value);
				}
			}

			// Handle allOf wrapper with libraryId
			if ("allOf" in condition && Array.isArray(condition.allOf)) {
				for (const c of condition.allOf) {
					if (c && typeof c === "object" && "libraryId" in c) {
						const libOp = (c as Record<string, unknown>).libraryId as {
							operator: string;
							value: string;
						};
						if (libOp.operator === "is") {
							results = results.filter((s) => s.libraryId === libOp.value);
						}
					}
				}
			}
		}

		// Apply full-text search
		if (body.fullTextSearch) {
			const searchLower = body.fullTextSearch.toLowerCase();
			results = results.filter((s) =>
				s.title.toLowerCase().includes(searchLower),
			);
		}

		// Apply sorting (from query params)
		if (sort) {
			const [field, direction] = sort.split(",");
			results.sort((a, b) => {
				const aVal = (a as Record<string, unknown>)[field];
				const bVal = (b as Record<string, unknown>)[field];
				if (typeof aVal === "string" && typeof bVal === "string") {
					return direction === "desc"
						? bVal.localeCompare(aVal)
						: aVal.localeCompare(bVal);
				}
				return 0;
			});
		}

		// Paginate (1-indexed)
		const start = (page - 1) * pageSize;
		const end = start + pageSize;
		const items = results.slice(start, end);

		if (full) {
			const fullItems = items.map(toFullSeriesResponse);
			return HttpResponse.json(
				createPaginatedResponse(fullItems, {
					page,
					pageSize,
					total: results.length,
					basePath: "/api/v1/series/list",
				}),
			);
		}

		return HttpResponse.json(
			createPaginatedResponse(items, {
				page,
				pageSize,
				total: results.length,
				basePath: "/api/v1/series/list",
			}),
		);
	}),

	// POST /series/list/alphabetical-groups - Get counts per first letter
	http.post("/api/v1/series/list/alphabetical-groups", async ({ request }) => {
		await delay(100);
		const body = (await request.json()) as {
			condition?: unknown;
		};

		// For mock purposes, we'll do basic filtering
		let results = [...mockSeries];

		// Apply basic library filtering if condition contains libraryId
		if (body.condition && typeof body.condition === "object") {
			const condition = body.condition as Record<string, unknown>;

			// Handle direct libraryId condition
			if ("libraryId" in condition) {
				const libOp = condition.libraryId as {
					operator: string;
					value: string;
				};
				if (libOp.operator === "is") {
					results = results.filter((s) => s.libraryId === libOp.value);
				}
			}

			// Handle allOf wrapper with libraryId
			if ("allOf" in condition && Array.isArray(condition.allOf)) {
				for (const c of condition.allOf) {
					if (c && typeof c === "object" && "libraryId" in c) {
						const libOp = (c as Record<string, unknown>).libraryId as {
							operator: string;
							value: string;
						};
						if (libOp.operator === "is") {
							results = results.filter((s) => s.libraryId === libOp.value);
						}
					}
				}
			}
		}

		// Group by first letter of title (lowercase)
		const groups = new Map<string, number>();
		for (const series of results) {
			const firstChar = series.title.charAt(0).toLowerCase();
			// Use # for digits, otherwise the letter
			const group = /\d/.test(firstChar) ? firstChar : firstChar;
			groups.set(group, (groups.get(group) || 0) + 1);
		}

		// Convert to array format
		const alphabeticalGroups = Array.from(groups.entries())
			.map(([group, count]) => ({ group, count }))
			.sort((a, b) => a.group.localeCompare(b.group));

		return HttpResponse.json(alphabeticalGroups);
	}),

	// List in-progress series
	// Supports ?libraryId= query param for library filtering
	// Supports ?full=true for full series response with metadata
	// Returns plain array (not paginated) - matches API expectation
	http.get("/api/v1/series/in-progress", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const libraryId = url.searchParams.get("libraryId");
		const full = url.searchParams.get("full") === "true";

		// Return a subset as "in-progress" series (those with reading progress)
		const baseSeries = libraryId ? getSeriesByLibrary(libraryId) : mockSeries;
		const inProgressSeries = baseSeries.slice(0, 5);

		if (full) {
			return HttpResponse.json(inProgressSeries.map(toFullSeriesResponse));
		}
		return HttpResponse.json(inProgressSeries);
	}),

	// List series with pagination (1-indexed)
	// Supports ?libraryId= query param for library filtering
	// Supports ?full=true for full series response with metadata
	http.get("/api/v1/series", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const page = Math.max(
			1,
			Number.parseInt(url.searchParams.get("page") || "1", 10),
		);
		const pageSize = Number.parseInt(
			url.searchParams.get("pageSize") || "50",
			10,
		);
		const libraryId = url.searchParams.get("libraryId");
		const full = url.searchParams.get("full") === "true";

		const filteredSeries = libraryId
			? getSeriesByLibrary(libraryId)
			: mockSeries;

		// 1-indexed pagination
		const start = (page - 1) * pageSize;
		const end = start + pageSize;
		const items = filteredSeries.slice(start, end);

		if (full) {
			const fullItems = items.map(toFullSeriesResponse);
			return HttpResponse.json(
				createPaginatedResponse(fullItems, {
					page,
					pageSize,
					total: filteredSeries.length,
					basePath: "/api/v1/series",
				}),
			);
		}

		return HttpResponse.json(
			createPaginatedResponse(items, {
				page,
				pageSize,
				total: filteredSeries.length,
				basePath: "/api/v1/series",
			}),
		);
	}),

	// List recently added series
	// Supports ?libraryId= query param for library filtering
	// Supports ?full=true for full series response with metadata
	http.get("/api/v1/series/recently-added", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const libraryId = url.searchParams.get("libraryId");
		const limit = Number.parseInt(url.searchParams.get("limit") || "50", 10);
		const full = url.searchParams.get("full") === "true";

		const baseSeries = libraryId ? getSeriesByLibrary(libraryId) : mockSeries;
		// Sort by createdAt desc and limit
		const recentSeries = [...baseSeries]
			.sort(
				(a, b) =>
					new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
			)
			.slice(0, limit);

		if (full) {
			return HttpResponse.json(recentSeries.map(toFullSeriesResponse));
		}
		return HttpResponse.json(recentSeries);
	}),

	// List recently updated series
	// Supports ?libraryId= query param for library filtering
	// Supports ?full=true for full series response with metadata
	http.get("/api/v1/series/recently-updated", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const libraryId = url.searchParams.get("libraryId");
		const limit = Number.parseInt(url.searchParams.get("limit") || "50", 10);
		const full = url.searchParams.get("full") === "true";

		const baseSeries = libraryId ? getSeriesByLibrary(libraryId) : mockSeries;
		// Sort by updatedAt desc and limit
		const recentSeries = [...baseSeries]
			.sort(
				(a, b) =>
					new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
			)
			.slice(0, limit);

		if (full) {
			return HttpResponse.json(recentSeries.map(toFullSeriesResponse));
		}
		return HttpResponse.json(recentSeries);
	}),

	// Get full series metadata (must come BEFORE generic /series/:id route)
	http.get("/api/v1/series/:id/metadata", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Get custom metadata for this series (some have sample data for demo)
		const customMetadata = getCustomMetadataForSeries(seriesItem.title);

		// Get rich metadata for this series
		const publisher =
			seriesPublishers[seriesItem.title] ||
			seriesItem.publisher ||
			"Unknown Publisher";
		const genreNames = seriesGenres[seriesItem.title] || [];
		const tagNames = seriesTags[seriesItem.title] || [];

		// Convert genre names to GenreDto objects
		const genres = genreNames.map((name, index) => ({
			id: `genre-${seriesItem.id}-${index}`,
			name,
			seriesCount: null,
			createdAt: seriesItem.createdAt,
		}));

		// Convert tag names to TagDto objects
		const tags = tagNames.map((name, index) => ({
			id: `tag-${seriesItem.id}-${index}`,
			name,
			seriesCount: null,
			createdAt: seriesItem.createdAt,
		}));

		// Determine language and reading direction based on publisher
		const isJapanese =
			publisher.includes("Viz") ||
			publisher.includes("Kodansha") ||
			publisher.includes("Shueisha");
		const language = isJapanese ? "ja" : "en";
		const readingDirection = isJapanese ? "rtl" : "ltr";

		// Determine age rating based on content
		const matureContent = [
			"The Walking Dead",
			"Attack on Titan",
			"Saga",
			"Preacher",
			"Watchmen",
		];
		const ageRating = matureContent.includes(seriesItem.title) ? 17 : 13;

		// Determine status
		const completedSeries = [
			"Batman: Year One",
			"Watchmen",
			"Attack on Titan",
			"Death Note",
			"Fullmetal Alchemist",
		];
		const status = completedSeries.includes(seriesItem.title)
			? "completed"
			: "ongoing";

		// Return FullSeriesMetadataResponse
		return HttpResponse.json({
			seriesId: seriesItem.id,
			title: seriesItem.title,
			summary: getSeriesSummary(seriesItem.title),
			publisher,
			imprint: publisher.includes("Vertigo") ? "Vertigo" : null,
			ageRating,
			language,
			status,
			readingDirection,
			titleSort: seriesItem.title.toLowerCase().replace(/^the\s+/, ""),
			totalBookCount: seriesItem.bookCount,
			year: seriesItem.year,
			customMetadata,
			createdAt: seriesItem.createdAt,
			updatedAt: seriesItem.updatedAt,
			genres,
			tags,
			alternateTitles: [],
			externalRatings: getExternalRatingsForSeries(
				seriesItem.id,
				seriesItem.title,
			),
			externalLinks: [],
			locks: {
				title: false,
				summary: false,
				publisher: false,
				imprint: false,
				ageRating: false,
				language: false,
				status: false,
				readingDirection: false,
				titleSort: false,
				totalBookCount: false,
				year: false,
				genres: false,
				tags: false,
				customMetadata: false,
			},
		});
	}),

	// Update series metadata (PATCH)
	// Request: PatchSeriesMetadataRequest, Response: SeriesMetadataResponse
	http.patch("/api/v1/series/:id/metadata", async ({ params, request }) => {
		await delay(150);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		const body = (await request.json()) as Record<string, unknown>;

		// Get custom metadata for this series
		const customMetadata = getCustomMetadataForSeries(seriesItem.title);

		// Get rich defaults
		const publisher =
			seriesPublishers[seriesItem.title] ||
			seriesItem.publisher ||
			"Unknown Publisher";
		const isJapanese =
			publisher.includes("Viz") ||
			publisher.includes("Kodansha") ||
			publisher.includes("Shueisha");
		const matureContent = [
			"The Walking Dead",
			"Attack on Titan",
			"Saga",
			"Preacher",
			"Watchmen",
		];
		const completedSeries = [
			"Batman: Year One",
			"Watchmen",
			"Attack on Titan",
			"Death Note",
			"Fullmetal Alchemist",
		];

		// Return SeriesMetadataResponse (not FullSeriesMetadataResponse)
		return HttpResponse.json({
			id: seriesItem.id,
			title: body.title ?? seriesItem.title,
			summary: body.summary ?? getSeriesSummary(seriesItem.title),
			publisher: body.publisher ?? publisher,
			imprint:
				body.imprint ?? (publisher.includes("Vertigo") ? "Vertigo" : null),
			ageRating:
				body.ageRating ?? (matureContent.includes(seriesItem.title) ? 17 : 13),
			language: body.language ?? (isJapanese ? "ja" : "en"),
			status:
				body.status ??
				(completedSeries.includes(seriesItem.title) ? "completed" : "ongoing"),
			readingDirection: body.readingDirection ?? (isJapanese ? "rtl" : "ltr"),
			titleSort:
				body.titleSort ?? seriesItem.title.toLowerCase().replace(/^the\s+/, ""),
			totalBookCount: body.totalBookCount ?? seriesItem.bookCount,
			year: body.year ?? seriesItem.year,
			customMetadata: body.customMetadata ?? customMetadata,
			updatedAt: new Date().toISOString(),
		});
	}),

	// Get series metadata locks (GET)
	// Response: MetadataLocks
	http.get("/api/v1/series/:id/metadata/locks", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Return MetadataLocks (all fields default to false)
		return HttpResponse.json({
			title: false,
			summary: false,
			publisher: false,
			imprint: false,
			ageRating: false,
			language: false,
			status: false,
			readingDirection: false,
			titleSort: false,
			totalBookCount: false,
			year: false,
			genres: false,
			tags: false,
			customMetadata: false,
		});
	}),

	// Update series metadata locks (PUT)
	// Request: UpdateMetadataLocksRequest, Response: MetadataLocks
	http.put("/api/v1/series/:id/metadata/locks", async ({ params, request }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		const body = (await request.json()) as Record<string, boolean>;

		// Return MetadataLocks (all fields required)
		return HttpResponse.json({
			title: body.title ?? false,
			summary: body.summary ?? false,
			publisher: body.publisher ?? false,
			imprint: body.imprint ?? false,
			ageRating: body.ageRating ?? false,
			language: body.language ?? false,
			status: body.status ?? false,
			readingDirection: body.readingDirection ?? false,
			titleSort: body.titleSort ?? false,
			totalBookCount: body.totalBookCount ?? false,
			year: body.year ?? false,
			genres: body.genres ?? false,
			tags: body.tags ?? false,
			customMetadata: body.customMetadata ?? false,
		});
	}),

	// Update series metadata locks (PATCH) - kept for backwards compatibility
	// Request: UpdateMetadataLocksRequest, Response: MetadataLocks
	http.patch(
		"/api/v1/series/:id/metadata/locks",
		async ({ params, request }) => {
			await delay(100);
			const seriesItem = mockSeries.find((s) => s.id === params.id);

			if (!seriesItem) {
				return HttpResponse.json(
					{ error: "Series not found" },
					{ status: 404 },
				);
			}

			const body = (await request.json()) as Record<string, boolean>;

			// Return MetadataLocks (all fields required)
			return HttpResponse.json({
				title: body.title ?? false,
				summary: body.summary ?? false,
				publisher: body.publisher ?? false,
				imprint: body.imprint ?? false,
				ageRating: body.ageRating ?? false,
				language: body.language ?? false,
				status: body.status ?? false,
				readingDirection: body.readingDirection ?? false,
				titleSort: body.titleSort ?? false,
				totalBookCount: body.totalBookCount ?? false,
				year: body.year ?? false,
				genres: body.genres ?? false,
				tags: body.tags ?? false,
				customMetadata: body.customMetadata ?? false,
			});
		},
	),

	// Get user rating for series (some series have pre-existing ratings for demo)
	http.get("/api/v1/series/:id/rating", async ({ params }) => {
		await delay(50);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		// Some series have pre-existing ratings for demo purposes (0-100 scale)
		const SERIES_RATINGS: Record<
			string,
			{ rating: number; notes: string | null }
		> = {
			"One Piece": {
				rating: 95,
				notes: "Absolute masterpiece! The world-building is incredible.",
			},
			"Batman: Year One": {
				rating: 85,
				notes: "Classic origin story, great noir atmosphere.",
			},
			"Attack on Titan": { rating: 90, notes: null },
			Saga: { rating: 88, notes: "Beautiful art and compelling story." },
		};

		if (seriesItem && seriesItem.title in SERIES_RATINGS) {
			const ratingData = SERIES_RATINGS[seriesItem.title];
			return HttpResponse.json({
				id: `rating-${params.id}`,
				seriesId: params.id,
				userId: "mock-user-id",
				rating: ratingData.rating,
				notes: ratingData.notes,
				createdAt: "2024-06-15T10:30:00Z",
				updatedAt: "2024-06-15T10:30:00Z",
			});
		}

		// Return 404 to indicate no rating exists (user hasn't rated yet)
		return HttpResponse.json({ error: "Rating not found" }, { status: 404 });
	}),

	// Set user rating for series
	http.put("/api/v1/series/:id/rating", async ({ params, request }) => {
		await delay(100);
		const body = (await request.json()) as { rating: number; notes?: string };
		return HttpResponse.json({
			id: "mock-rating-id",
			seriesId: params.id,
			userId: "mock-user-id",
			rating: body.rating,
			notes: body.notes || null,
			createdAt: new Date().toISOString(),
			updatedAt: new Date().toISOString(),
		});
	}),

	// Delete user rating for series
	http.delete("/api/v1/series/:id/rating", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),

	// Get average community rating for series
	http.get("/api/v1/series/:id/ratings/average", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Get community rating data for this series
		const communityRating = SERIES_COMMUNITY_RATINGS[seriesItem.title];

		if (!communityRating) {
			// No community ratings for this series
			return HttpResponse.json({
				average: null,
				count: 0,
			});
		}

		return HttpResponse.json({
			average: communityRating.average,
			count: communityRating.count,
		});
	}),

	// Generate thumbnails for series (queues a background task)
	http.post("/api/v1/series/:id/thumbnails/generate", async () => {
		await delay(100);
		return HttpResponse.json({ task_id: crypto.randomUUID() });
	}),

	// ============================================
	// Alternate Titles
	// ============================================

	// Get alternate titles for series
	http.get("/api/v1/series/:id/alternate-titles", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Sample alternate titles for some series
		const alternateTitlesData: Record<
			string,
			Array<{ title: string; label: string }>
		> = {
			"One Piece": [
				{ title: "ワンピース", label: "Japanese" },
				{ title: "Wan Pīsu", label: "Romaji" },
			],
			"Attack on Titan": [
				{ title: "進撃の巨人", label: "Japanese" },
				{ title: "Shingeki no Kyojin", label: "Romaji" },
			],
			Naruto: [{ title: "ナルト", label: "Japanese" }],
		};

		const titles = (alternateTitlesData[seriesItem.title] || []).map(
			(t, index) => ({
				id: `alt-title-${params.id}-${index}`,
				seriesId: params.id,
				title: t.title,
				label: t.label,
				createdAt: seriesItem.createdAt,
				updatedAt: seriesItem.updatedAt,
			}),
		);

		return HttpResponse.json({ titles });
	}),

	// Create alternate title
	http.post(
		"/api/v1/series/:id/alternate-titles",
		async ({ params, request }) => {
			await delay(100);
			const seriesItem = mockSeries.find((s) => s.id === params.id);

			if (!seriesItem) {
				return HttpResponse.json(
					{ error: "Series not found" },
					{ status: 404 },
				);
			}

			const body = (await request.json()) as { title: string; label: string };
			const now = new Date().toISOString();

			return HttpResponse.json({
				id: `alt-title-${params.id}-${Date.now()}`,
				seriesId: params.id,
				title: body.title,
				label: body.label,
				createdAt: now,
				updatedAt: now,
			});
		},
	),

	// Update alternate title
	http.patch(
		"/api/v1/series/:id/alternate-titles/:titleId",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as { title?: string; label?: string };

			return HttpResponse.json({
				id: params.titleId,
				seriesId: params.id,
				title: body.title || "Updated Title",
				label: body.label || "Updated Label",
				createdAt: "2024-01-01T00:00:00Z",
				updatedAt: new Date().toISOString(),
			});
		},
	),

	// Delete alternate title
	http.delete("/api/v1/series/:id/alternate-titles/:titleId", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// External Ratings
	// ============================================

	// Get external ratings for series
	http.get("/api/v1/series/:id/external-ratings", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		const ratings = getExternalRatingsForSeries(
			params.id as string,
			seriesItem.title,
		);
		return HttpResponse.json({ ratings });
	}),

	// Create external rating
	http.post(
		"/api/v1/series/:id/external-ratings",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as {
				source_name: string;
				rating: number;
				vote_count?: number;
			};
			const now = new Date().toISOString();

			return HttpResponse.json({
				id: `ext-rating-${params.id}-${Date.now()}`,
				seriesId: params.id,
				sourceName: body.source_name,
				rating: body.rating,
				voteCount: body.vote_count || null,
				fetchedAt: now,
				createdAt: now,
				updatedAt: now,
			});
		},
	),

	// Delete external rating
	http.delete("/api/v1/series/:id/external-ratings/:ratingId", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// External Links
	// ============================================

	// Get external links for series
	http.get("/api/v1/series/:id/external-links", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Sample external links for some series
		const externalLinksData: Record<
			string,
			Array<{ sourceName: string; url: string; externalId?: string }>
		> = {
			"One Piece": [
				{
					sourceName: "myanimelist",
					url: "https://myanimelist.net/manga/13",
					externalId: "13",
				},
				{
					sourceName: "anilist",
					url: "https://anilist.co/manga/30013",
					externalId: "30013",
				},
			],
			"Attack on Titan": [
				{
					sourceName: "myanimelist",
					url: "https://myanimelist.net/manga/23390",
					externalId: "23390",
				},
			],
		};

		const links = (externalLinksData[seriesItem.title] || []).map(
			(l, index) => ({
				id: `ext-link-${params.id}-${index}`,
				seriesId: params.id,
				sourceName: l.sourceName,
				url: l.url,
				externalId: l.externalId || null,
				createdAt: seriesItem.createdAt,
				updatedAt: seriesItem.updatedAt,
			}),
		);

		return HttpResponse.json({ links });
	}),

	// Create external link
	http.post(
		"/api/v1/series/:id/external-links",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as {
				source_name: string;
				url: string;
				external_id?: string;
			};
			const now = new Date().toISOString();

			return HttpResponse.json({
				id: `ext-link-${params.id}-${Date.now()}`,
				seriesId: params.id,
				sourceName: body.source_name,
				url: body.url,
				externalId: body.external_id || null,
				createdAt: now,
				updatedAt: now,
			});
		},
	),

	// Delete external link
	http.delete("/api/v1/series/:id/external-links/:linkId", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// Series Covers
	// ============================================

	// List covers for series
	http.get("/api/v1/series/:id/covers", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Return a default cover and optionally a custom one
		const covers = [
			{
				id: `cover-${params.id}-default`,
				seriesId: params.id,
				source: "book",
				bookId: `book-${params.id}-1`,
				isSelected: true,
				createdAt: seriesItem.createdAt,
			},
		];

		return HttpResponse.json({ covers });
	}),

	// Upload custom cover
	http.post("/api/v1/series/:id/cover", async ({ params }) => {
		await delay(200);
		const now = new Date().toISOString();

		return HttpResponse.json({
			id: `cover-${params.id}-custom-${Date.now()}`,
			seriesId: params.id,
			source: "custom",
			bookId: null,
			isSelected: true,
			createdAt: now,
		});
	}),

	// Select cover
	http.put("/api/v1/series/:id/covers/:coverId/select", async () => {
		await delay(100);
		return new HttpResponse(null, { status: 204 });
	}),

	// Delete cover
	http.delete("/api/v1/series/:id/covers/:coverId", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// Series Core Updates
	// ============================================

	// PATCH series (core fields like title)
	http.patch("/api/v1/series/:id", async ({ params, request }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		const body = (await request.json()) as { title?: string };

		return HttpResponse.json({
			...seriesItem,
			title: body.title ?? seriesItem.title,
			updatedAt: new Date().toISOString(),
		});
	}),

	// PUT series metadata (replace full metadata)
	http.put("/api/v1/series/:id/metadata", async ({ params, request }) => {
		await delay(150);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		const body = (await request.json()) as Record<string, unknown>;

		return HttpResponse.json({
			id: seriesItem.id,
			title: body.title ?? seriesItem.title,
			summary: body.summary ?? null,
			publisher: body.publisher ?? null,
			imprint: body.imprint ?? null,
			ageRating: body.ageRating ?? null,
			language: body.language ?? "en",
			status: body.status ?? "ongoing",
			readingDirection: body.readingDirection ?? "ltr",
			titleSort:
				body.titleSort ?? seriesItem.title.toLowerCase().replace(/^the\s+/, ""),
			totalBookCount: body.totalBookCount ?? seriesItem.bookCount,
			year: body.year ?? seriesItem.year,
			customMetadata: body.customMetadata ?? null,
			updatedAt: new Date().toISOString(),
		});
	}),

	// Analyze series (trigger metadata fetch)
	http.post("/api/v1/series/:id/analyze", async () => {
		await delay(100);
		return HttpResponse.json({ message: "Analysis queued" });
	}),

	// Analyze unanalyzed books in series
	http.post("/api/v1/series/:id/analyze-unanalyzed", async () => {
		await delay(100);
		return HttpResponse.json({
			message: "Analysis queued for unanalyzed books",
		});
	}),

	// Mark series as read
	http.post("/api/v1/series/:id/read", async () => {
		await delay(100);
		return new HttpResponse(null, { status: 204 });
	}),

	// Mark series as unread
	http.post("/api/v1/series/:id/unread", async () => {
		await delay(100);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// Preprocessing Rules / Search Title
	// ============================================

	// Get preprocessed search title for a series
	http.get("/api/v1/series/:id/metadata/search-title", async ({ params }) => {
		await delay(100);
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		// Simulate preprocessing - remove common suffixes
		let searchTitle = seriesItem.title;
		const suffixesToRemove = [
			/ \(Digital\)$/i,
			/ \(Complete\)$/i,
			/ \[Digital\]$/i,
			/ - Complete$/i,
		];
		let rulesApplied = 0;
		for (const suffix of suffixesToRemove) {
			if (suffix.test(searchTitle)) {
				searchTitle = searchTitle.replace(suffix, "");
				rulesApplied++;
			}
		}

		return HttpResponse.json({
			originalTitle: seriesItem.title,
			searchTitle,
			rulesApplied,
		});
	}),

	// Reprocess series title using library preprocessing rules
	http.post(
		"/api/v1/series/:id/title/reprocess",
		async ({ params, request }) => {
			await delay(150);
			const seriesItem = mockSeries.find((s) => s.id === params.id);

			if (!seriesItem) {
				return HttpResponse.json(
					{ error: "Series not found" },
					{ status: 404 },
				);
			}

			const body = (await request.json().catch(() => ({}))) as {
				dryRun?: boolean;
			};

			// Simulate preprocessing - remove common suffixes
			let newTitle = seriesItem.title;
			const suffixesToRemove = [
				/ \(Digital\)$/i,
				/ \(Complete\)$/i,
				/ \[Digital\]$/i,
				/ - Complete$/i,
			];
			for (const suffix of suffixesToRemove) {
				newTitle = newTitle.replace(suffix, "");
			}

			const changed = newTitle !== seriesItem.title;

			if (body.dryRun) {
				// Return synchronous preview
				return HttpResponse.json({
					seriesId: seriesItem.id,
					originalTitle: seriesItem.title,
					newTitle,
					changed,
					skipped: false,
					skipReason: null,
					titleSortCleared: changed,
				});
			}

			// Return task ID for async processing
			return HttpResponse.json({
				taskId: `reprocess-title-${params.id}-${Date.now()}`,
			});
		},
	),

	// Get series by ID (must come AFTER specific routes like /in-progress, /recently-added, etc.)
	// Supports ?full=true for full series response with metadata
	http.get("/api/v1/series/:id", async ({ params, request }) => {
		await delay(100);
		const url = new URL(request.url);
		const full = url.searchParams.get("full") === "true";
		const seriesItem = mockSeries.find((s) => s.id === params.id);

		if (!seriesItem) {
			return HttpResponse.json({ error: "Series not found" }, { status: 404 });
		}

		if (full) {
			return HttpResponse.json(toFullSeriesResponse(seriesItem));
		}
		return HttpResponse.json(seriesItem);
	}),

	// Get series thumbnail
	http.get("/api/v1/series/:id/thumbnail", async () => {
		await delay(50);
		// Return the cover SVG as an image response
		return new HttpResponse(coverSvg, {
			headers: {
				"Content-Type": "image/svg+xml",
			},
		});
	}),

	// List series by library (1-indexed)
	// Supports ?full=true for full series response with metadata
	http.get(
		"/api/v1/libraries/:libraryId/series",
		async ({ params, request }) => {
			await delay(200);
			const url = new URL(request.url);
			const page = Math.max(
				1,
				Number.parseInt(url.searchParams.get("page") || "1", 10),
			);
			const pageSize = Number.parseInt(
				url.searchParams.get("pageSize") || "50",
				10,
			);
			const full = url.searchParams.get("full") === "true";

			const filteredSeries = getSeriesByLibrary(params.libraryId as string);
			// 1-indexed pagination
			const start = (page - 1) * pageSize;
			const end = start + pageSize;
			const items = filteredSeries.slice(start, end);

			if (full) {
				const fullItems = items.map(toFullSeriesResponse);
				return HttpResponse.json(
					createPaginatedResponse(fullItems, {
						page,
						pageSize,
						total: filteredSeries.length,
						basePath: `/api/v1/libraries/${params.libraryId}/series`,
					}),
				);
			}

			return HttpResponse.json(
				createPaginatedResponse(items, {
					page,
					pageSize,
					total: filteredSeries.length,
					basePath: `/api/v1/libraries/${params.libraryId}/series`,
				}),
			);
		},
	),

	// Library-scoped: List in-progress series
	// Supports ?full=true for full series response with metadata
	http.get(
		"/api/v1/libraries/:libraryId/series/in-progress",
		async ({ params, request }) => {
			await delay(200);
			const url = new URL(request.url);
			const full = url.searchParams.get("full") === "true";

			// Return a subset of in-progress series for this library
			const librarySeries = getSeriesByLibrary(params.libraryId as string);
			const inProgressSeries = librarySeries.slice(0, 5);

			if (full) {
				return HttpResponse.json(inProgressSeries.map(toFullSeriesResponse));
			}
			return HttpResponse.json(inProgressSeries);
		},
	),
];

/**
 * Convert a SeriesDto to FullSeriesResponse format
 * Used when ?full=true query parameter is specified
 */
function toFullSeriesResponse(seriesItem: (typeof mockSeries)[number]) {
	const publisher =
		seriesPublishers[seriesItem.title] ||
		seriesItem.publisher ||
		"Unknown Publisher";
	const genreNames = seriesGenres[seriesItem.title] || [];
	const tagNames = seriesTags[seriesItem.title] || [];
	const customMetadata = getCustomMetadataForSeries(seriesItem.title);

	// Convert genre names to GenreDto objects
	const genres = genreNames.map((name, index) => ({
		id: `genre-${seriesItem.id}-${index}`,
		name,
		seriesCount: null,
		createdAt: seriesItem.createdAt,
	}));

	// Convert tag names to TagDto objects
	const tags = tagNames.map((name, index) => ({
		id: `tag-${seriesItem.id}-${index}`,
		name,
		seriesCount: null,
		createdAt: seriesItem.createdAt,
	}));

	// Determine language and reading direction based on publisher
	const isJapanese =
		publisher.includes("Viz") ||
		publisher.includes("Kodansha") ||
		publisher.includes("Shueisha");
	const language = isJapanese ? "ja" : "en";
	const readingDirection = isJapanese ? "rtl" : "ltr";

	// Determine age rating based on content
	const matureContent = [
		"The Walking Dead",
		"Attack on Titan",
		"Saga",
		"Preacher",
		"Watchmen",
	];
	const ageRating = matureContent.includes(seriesItem.title) ? 17 : 13;

	// Determine status
	const completedSeries = [
		"Batman: Year One",
		"Watchmen",
		"Attack on Titan",
		"Death Note",
		"Fullmetal Alchemist",
	];
	const status = completedSeries.includes(seriesItem.title)
		? "completed"
		: "ongoing";

	// Build FullSeriesResponse
	return {
		id: seriesItem.id,
		libraryId: seriesItem.libraryId,
		libraryName: seriesItem.libraryName || "Unknown Library",
		bookCount: seriesItem.bookCount,
		unreadCount: seriesItem.unreadCount,
		path: `/media/comics/${seriesItem.title.replace(/[^a-zA-Z0-9]/g, "-")}`,
		selectedCoverSource: "first_book",
		hasCustomCover: false,
		createdAt: seriesItem.createdAt,
		updatedAt: seriesItem.updatedAt,
		metadata: {
			title: seriesItem.title,
			titleSort: seriesItem.title.toLowerCase().replace(/^the\s+/, ""),
			summary: getSeriesSummary(seriesItem.title),
			publisher,
			imprint: publisher.includes("Vertigo") ? "Vertigo" : null,
			ageRating,
			language,
			status,
			readingDirection,
			totalBookCount: seriesItem.bookCount,
			year: seriesItem.year,
			customMetadata,
			locks: {
				titleLock: false,
				summaryLock: false,
				publisherLock: false,
				imprintLock: false,
				ageRatingLock: false,
				languageLock: false,
				statusLock: false,
				readingDirectionLock: false,
				titleSortLock: false,
				totalBookCountLock: false,
				yearLock: false,
				genresLock: false,
				tagsLock: false,
				customMetadataLock: false,
			},
		},
		genres,
		tags,
		alternateTitles: [],
		externalRatings: getExternalRatingsForSeries(
			seriesItem.id,
			seriesItem.title,
		),
		externalLinks: [],
		externalIds: getExternalIdsForSeries(seriesItem.id, seriesItem.title),
	};
}

// Helper to get current mock series (for testing)
export const getMockSeries = () => [...mockSeries];
