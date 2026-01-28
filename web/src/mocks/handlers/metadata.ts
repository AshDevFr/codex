/**
 * Metadata API mock handlers (genres, tags, etc.)
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type GenreDto = components["schemas"]["GenreDto"];
type TagDto = components["schemas"]["TagDto"];

// Helper to convert internal mock genre to GenreDto
const toGenreDto = (g: {
	id: string;
	name: string;
	series_count: number;
}): GenreDto => ({
	id: g.id,
	name: g.name,
	seriesCount: g.series_count,
	createdAt: "2024-01-01T00:00:00Z",
});

// Helper to convert internal mock tag to TagDto
const toTagDto = (t: {
	id: string;
	name: string;
	series_count: number;
}): TagDto => ({
	id: t.id,
	name: t.name,
	seriesCount: t.series_count,
	createdAt: "2024-01-01T00:00:00Z",
});

// Mock genres data - includes all genres used in series metadata
const mockGenres = [
	{ id: "genre-1", name: "Action", series_count: 15 },
	{ id: "genre-2", name: "Adventure", series_count: 10 },
	{ id: "genre-3", name: "Comedy", series_count: 12 },
	{ id: "genre-4", name: "Crime", series_count: 4 },
	{ id: "genre-5", name: "Dark Fantasy", series_count: 3 },
	{ id: "genre-6", name: "Drama", series_count: 10 },
	{ id: "genre-7", name: "Fantasy", series_count: 8 },
	{ id: "genre-8", name: "Horror", series_count: 5 },
	{ id: "genre-9", name: "Martial Arts", series_count: 3 },
	{ id: "genre-10", name: "Mythology", series_count: 2 },
	{ id: "genre-11", name: "Noir", series_count: 2 },
	{ id: "genre-12", name: "Political Drama", series_count: 2 },
	{ id: "genre-13", name: "Post-Apocalyptic", series_count: 4 },
	{ id: "genre-14", name: "Romance", series_count: 7 },
	{ id: "genre-15", name: "Science Fiction", series_count: 6 },
	{ id: "genre-16", name: "Slice of Life", series_count: 4 },
	{ id: "genre-17", name: "Superhero", series_count: 8 },
];

// Mock tags data - includes all tags used in series metadata
const mockTags = [
	{ id: "tag-1", name: "adventure", series_count: 5 },
	{ id: "tag-2", name: "classic", series_count: 3 },
	{ id: "tag-3", name: "community", series_count: 2 },
	{ id: "tag-4", name: "completed", series_count: 15 },
	{ id: "tag-5", name: "crime drama", series_count: 2 },
	{ id: "tag-6", name: "dreams", series_count: 1 },
	{ id: "tag-7", name: "endless", series_count: 1 },
	{ id: "tag-8", name: "family", series_count: 3 },
	{ id: "tag-9", name: "friendship", series_count: 4 },
	{ id: "tag-10", name: "literary", series_count: 2 },
	{ id: "tag-11", name: "long-running", series_count: 5 },
	{ id: "tag-12", name: "mature themes", series_count: 4 },
	{ id: "tag-13", name: "military", series_count: 3 },
	{ id: "tag-14", name: "mystery", series_count: 4 },
	{ id: "tag-15", name: "mythology", series_count: 2 },
	{ id: "tag-16", name: "origin story", series_count: 3 },
	{ id: "tag-17", name: "pirates", series_count: 2 },
	{ id: "tag-18", name: "plot twists", series_count: 5 },
	{ id: "tag-19", name: "space opera", series_count: 2 },
	{ id: "tag-20", name: "street-level", series_count: 2 },
	{ id: "tag-21", name: "survival", series_count: 3 },
	{ id: "tag-22", name: "titans", series_count: 1 },
	{ id: "tag-23", name: "war", series_count: 3 },
	{ id: "tag-24", name: "world-building", series_count: 6 },
	{ id: "tag-25", name: "zombies", series_count: 2 },
];

// Series-specific tags mapping
const seriesTagsMap: Record<string, string[]> = {
	"One Piece": [
		"adventure",
		"pirates",
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
	"Batman: Year One": [
		"origin story",
		"street-level",
		"crime drama",
		"classic",
	],
	Saga: ["space opera", "family", "war", "mature themes"],
	"The Walking Dead": ["zombies", "survival", "community", "completed"],
	Sandman: ["mythology", "literary", "endless", "dreams"],
};

// Series-specific genres mapping
const seriesGenresMap: Record<string, string[]> = {
	"One Piece": ["Action", "Adventure", "Comedy", "Fantasy"],
	"Attack on Titan": ["Dark Fantasy", "Action", "Post-Apocalyptic"],
	"Batman: Year One": ["Superhero", "Crime", "Noir"],
	Saga: ["Science Fiction", "Fantasy", "Drama", "Romance"],
	"The Walking Dead": ["Horror", "Drama", "Post-Apocalyptic"],
	Sandman: ["Fantasy", "Horror", "Mythology"],
	Naruto: ["Action", "Adventure", "Martial Arts"],
};

export const metadataHandlers = [
	// ============================================
	// Global Genres
	// ============================================

	// Get all genres
	http.get("/api/v1/genres", async ({ request }) => {
		await delay(100);
		const url = new URL(request.url);
		const page = Number(url.searchParams.get("page")) || 1;
		const pageSize = Number(url.searchParams.get("pageSize")) || 50;

		// Calculate pagination
		const startIndex = (page - 1) * pageSize;
		const endIndex = startIndex + pageSize;
		const paginatedGenres = mockGenres.slice(startIndex, endIndex);
		const totalPages = Math.ceil(mockGenres.length / pageSize);

		// Return paginated response format expected by the API client
		return HttpResponse.json({
			data: paginatedGenres.map(toGenreDto),
			page,
			pageSize,
			total: mockGenres.length,
			totalPages,
			links: {
				self: `/api/v1/genres?page=${page}&pageSize=${pageSize}`,
				...(page > 1 && {
					prev: `/api/v1/genres?page=${page - 1}&pageSize=${pageSize}`,
				}),
				...(page < totalPages && {
					next: `/api/v1/genres?page=${page + 1}&pageSize=${pageSize}`,
				}),
			},
		});
	}),

	// Delete a genre globally
	http.delete("/api/v1/genres/:genreId", async () => {
		await delay(100);
		return new HttpResponse(null, { status: 204 });
	}),

	// Cleanup unused genres
	http.post("/api/v1/genres/cleanup", async () => {
		await delay(200);
		return HttpResponse.json({ removed: 0 });
	}),

	// ============================================
	// Series Genres
	// ============================================

	// Get genres for a series
	http.get("/api/v1/series/:seriesId/genres", async ({ params }) => {
		await delay(100);
		// Return some sample genres based on series
		const genreNames = seriesGenresMap[params.seriesId as string] || [
			"Action",
			"Adventure",
		];
		const genres: GenreDto[] = genreNames.map((name, index) => {
			const genre = mockGenres.find((g) => g.name === name);
			return {
				id: genre?.id || `genre-series-${params.seriesId}-${index}`,
				name,
				seriesCount: genre?.series_count || 1,
				createdAt: "2024-01-01T00:00:00Z",
			};
		});
		return HttpResponse.json({ genres });
	}),

	// Replace all genres for a series (PUT)
	http.put("/api/v1/series/:seriesId/genres", async ({ request }) => {
		await delay(100);
		const body = (await request.json()) as {
			genreIds?: string[];
			genreNames?: string[];
		};
		const names = body.genreNames || [];
		const genres: GenreDto[] = names.map((name, index) => ({
			id: `genre-${index}`,
			name,
			seriesCount: 1,
			createdAt: new Date().toISOString(),
		}));
		return HttpResponse.json({ genres });
	}),

	// Add a genre to a series (POST)
	http.post("/api/v1/series/:seriesId/genres", async ({ params, request }) => {
		await delay(100);
		const body = (await request.json()) as {
			genreId?: string;
			genreName?: string;
		};
		const genre: GenreDto = {
			id: body.genreId || `genre-${params.seriesId}-${Date.now()}`,
			name: body.genreName || "New Genre",
			seriesCount: 1,
			createdAt: new Date().toISOString(),
		};
		return HttpResponse.json(genre);
	}),

	// Remove a genre from a series
	http.delete("/api/v1/series/:seriesId/genres/:genreId", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// Global Tags
	// ============================================

	// Get all tags
	http.get("/api/v1/tags", async ({ request }) => {
		await delay(100);
		const url = new URL(request.url);
		const page = Number(url.searchParams.get("page")) || 1;
		const pageSize = Number(url.searchParams.get("pageSize")) || 50;

		// Calculate pagination
		const startIndex = (page - 1) * pageSize;
		const endIndex = startIndex + pageSize;
		const paginatedTags = mockTags.slice(startIndex, endIndex);
		const totalPages = Math.ceil(mockTags.length / pageSize);

		// Return paginated response format expected by the API client
		return HttpResponse.json({
			data: paginatedTags.map(toTagDto),
			page,
			pageSize,
			total: mockTags.length,
			totalPages,
			links: {
				self: `/api/v1/tags?page=${page}&pageSize=${pageSize}`,
				...(page > 1 && {
					prev: `/api/v1/tags?page=${page - 1}&pageSize=${pageSize}`,
				}),
				...(page < totalPages && {
					next: `/api/v1/tags?page=${page + 1}&pageSize=${pageSize}`,
				}),
			},
		});
	}),

	// Delete a tag globally
	http.delete("/api/v1/tags/:tagId", async () => {
		await delay(100);
		return new HttpResponse(null, { status: 204 });
	}),

	// Cleanup unused tags
	http.post("/api/v1/tags/cleanup", async () => {
		await delay(200);
		return HttpResponse.json({ removed: 0 });
	}),

	// ============================================
	// Series Tags
	// ============================================

	// Get tags for a series
	http.get("/api/v1/series/:seriesId/tags", async ({ params }) => {
		await delay(100);
		// Return some sample tags based on series
		const tagNames = seriesTagsMap[params.seriesId as string] || [
			"adventure",
			"action",
		];
		const tags: TagDto[] = tagNames.map((name, index) => {
			const tag = mockTags.find((t) => t.name === name);
			return {
				id: tag?.id || `tag-series-${params.seriesId}-${index}`,
				name,
				seriesCount: tag?.series_count || 1,
				createdAt: "2024-01-01T00:00:00Z",
			};
		});
		return HttpResponse.json({ tags });
	}),

	// Replace all tags for a series (PUT)
	http.put("/api/v1/series/:seriesId/tags", async ({ request }) => {
		await delay(100);
		const body = (await request.json()) as {
			tagIds?: string[];
			tagNames?: string[];
		};
		const names = body.tagNames || [];
		const tags: TagDto[] = names.map((name, index) => ({
			id: `tag-${index}`,
			name,
			seriesCount: 1,
			createdAt: new Date().toISOString(),
		}));
		return HttpResponse.json({ tags });
	}),

	// Add a tag to a series (POST)
	http.post("/api/v1/series/:seriesId/tags", async ({ params, request }) => {
		await delay(100);
		const body = (await request.json()) as { tagId?: string; tagName?: string };
		const tag: TagDto = {
			id: body.tagId || `tag-${params.seriesId}-${Date.now()}`,
			name: body.tagName || "new-tag",
			seriesCount: 1,
			createdAt: new Date().toISOString(),
		};
		return HttpResponse.json(tag);
	}),

	// Remove a tag from a series
	http.delete("/api/v1/series/:seriesId/tags/:tagId", async () => {
		await delay(50);
		return new HttpResponse(null, { status: 204 });
	}),
];

// Export mock data for testing
export const getMockGenres = () => [...mockGenres];
export const getMockTags = () => [...mockTags];
