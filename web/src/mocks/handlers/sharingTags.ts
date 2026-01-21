/**
 * MSW handlers for sharing tags API endpoints
 */

import { delay, HttpResponse, http } from "msw";

// Mock sharing tags
const mockSharingTags: Array<{
	id: string;
	name: string;
	normalizedName: string;
	description: string | null;
	seriesCount: number;
	userCount: number;
	createdAt: string;
	updatedAt: string;
}> = [
	{
		id: "tag-kids",
		name: "Kids",
		normalizedName: "kids",
		description: "Content appropriate for children",
		seriesCount: 5,
		userCount: 2,
		createdAt: "2024-01-01T00:00:00Z",
		updatedAt: "2024-01-01T00:00:00Z",
	},
	{
		id: "tag-mature",
		name: "Mature",
		normalizedName: "mature",
		description: "Adult content",
		seriesCount: 3,
		userCount: 1,
		createdAt: "2024-01-02T00:00:00Z",
		updatedAt: "2024-01-02T00:00:00Z",
	},
];

// Mock series sharing tags (series_id -> tag_ids)
const mockSeriesSharingTags: Record<string, string[]> = {};

// Mock user sharing tag grants (user_id -> grants)
const mockUserGrants: Record<
	string,
	Array<{
		id: string;
		sharingTagId: string;
		sharingTagName: string;
		accessMode: "allow" | "deny";
		createdAt: string;
	}>
> = {};

export const sharingTagsHandlers = [
	// ============================================
	// Admin Sharing Tags CRUD
	// ============================================

	// List all sharing tags
	http.get("/api/v1/admin/sharing-tags", async () => {
		await delay(100);
		return HttpResponse.json({ items: mockSharingTags });
	}),

	// Get single sharing tag
	http.get("/api/v1/admin/sharing-tags/:tagId", async ({ params }) => {
		await delay(50);
		const tag = mockSharingTags.find((t) => t.id === params.tagId);
		if (!tag) {
			return HttpResponse.json({ error: "Not found" }, { status: 404 });
		}
		return HttpResponse.json(tag);
	}),

	// Create sharing tag
	http.post("/api/v1/admin/sharing-tags", async ({ request }) => {
		await delay(100);
		const body = (await request.json()) as {
			name: string;
			description?: string;
		};

		const newTag = {
			id: `tag-${Date.now()}`,
			name: body.name,
			normalizedName: body.name.toLowerCase(),
			description: body.description || null,
			seriesCount: 0,
			userCount: 0,
			createdAt: new Date().toISOString(),
			updatedAt: new Date().toISOString(),
		};

		mockSharingTags.push(newTag);
		return HttpResponse.json(newTag, { status: 201 });
	}),

	// Update sharing tag
	http.patch(
		"/api/v1/admin/sharing-tags/:tagId",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as {
				name?: string;
				description?: string | null;
			};

			const tagIndex = mockSharingTags.findIndex((t) => t.id === params.tagId);
			if (tagIndex === -1) {
				return HttpResponse.json({ error: "Not found" }, { status: 404 });
			}

			if (body.name !== undefined) {
				mockSharingTags[tagIndex].name = body.name;
				mockSharingTags[tagIndex].normalizedName = body.name.toLowerCase();
			}
			if (body.description !== undefined) {
				mockSharingTags[tagIndex].description = body.description;
			}
			mockSharingTags[tagIndex].updatedAt = new Date().toISOString();

			return HttpResponse.json(mockSharingTags[tagIndex]);
		},
	),

	// Delete sharing tag
	http.delete("/api/v1/admin/sharing-tags/:tagId", async ({ params }) => {
		await delay(100);
		const tagIndex = mockSharingTags.findIndex((t) => t.id === params.tagId);
		if (tagIndex === -1) {
			return HttpResponse.json({ error: "Not found" }, { status: 404 });
		}

		mockSharingTags.splice(tagIndex, 1);
		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// Series Sharing Tags
	// ============================================

	// Get sharing tags for a series
	http.get("/api/v1/series/:seriesId/sharing-tags", async ({ params }) => {
		await delay(50);
		const tagIds = mockSeriesSharingTags[params.seriesId as string] || [];
		const tags = mockSharingTags
			.filter((t) => tagIds.includes(t.id))
			.map((t) => ({
				id: t.id,
				name: t.name,
				description: t.description,
			}));
		return HttpResponse.json(tags);
	}),

	// Set sharing tags for a series (replace all)
	http.put(
		"/api/v1/series/:seriesId/sharing-tags",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as { sharingTagIds: string[] };
			const seriesId = params.seriesId as string;

			mockSeriesSharingTags[seriesId] = body.sharingTagIds;

			const tags = mockSharingTags
				.filter((t) => body.sharingTagIds.includes(t.id))
				.map((t) => ({
					id: t.id,
					name: t.name,
					description: t.description,
				}));
			return HttpResponse.json(tags);
		},
	),

	// Add a sharing tag to a series
	http.post(
		"/api/v1/series/:seriesId/sharing-tags",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as { sharingTagId: string };
			const seriesId = params.seriesId as string;

			if (!mockSeriesSharingTags[seriesId]) {
				mockSeriesSharingTags[seriesId] = [];
			}
			if (!mockSeriesSharingTags[seriesId].includes(body.sharingTagId)) {
				mockSeriesSharingTags[seriesId].push(body.sharingTagId);
			}

			const tags = mockSharingTags
				.filter((t) => mockSeriesSharingTags[seriesId].includes(t.id))
				.map((t) => ({
					id: t.id,
					name: t.name,
					description: t.description,
				}));
			return HttpResponse.json(tags);
		},
	),

	// Remove a sharing tag from a series
	http.delete(
		"/api/v1/series/:seriesId/sharing-tags/:tagId",
		async ({ params }) => {
			await delay(100);
			const seriesId = params.seriesId as string;
			const tagId = params.tagId as string;

			if (mockSeriesSharingTags[seriesId]) {
				mockSeriesSharingTags[seriesId] = mockSeriesSharingTags[
					seriesId
				].filter((id) => id !== tagId);
			}

			return new HttpResponse(null, { status: 204 });
		},
	),

	// ============================================
	// User Sharing Tag Grants
	// ============================================

	// Get current user's sharing tag grants
	http.get("/api/v1/user/sharing-tags", async () => {
		await delay(50);
		// Use a default user ID for current user
		const grants = mockUserGrants["current-user"] || [];
		return HttpResponse.json({ grants });
	}),

	// Get grants for a specific user (admin)
	http.get("/api/v1/users/:userId/sharing-tags", async ({ params }) => {
		await delay(50);
		const grants = mockUserGrants[params.userId as string] || [];
		return HttpResponse.json({ grants });
	}),

	// Set a grant for a user
	http.put(
		"/api/v1/users/:userId/sharing-tags",
		async ({ params, request }) => {
			await delay(100);
			const body = (await request.json()) as {
				sharingTagId: string;
				accessMode: "allow" | "deny";
			};
			const userId = params.userId as string;

			if (!mockUserGrants[userId]) {
				mockUserGrants[userId] = [];
			}

			const tag = mockSharingTags.find((t) => t.id === body.sharingTagId);
			if (!tag) {
				return HttpResponse.json({ error: "Tag not found" }, { status: 404 });
			}

			// Check if grant already exists
			const existingIndex = mockUserGrants[userId].findIndex(
				(g) => g.sharingTagId === body.sharingTagId,
			);

			const grant = {
				id:
					existingIndex >= 0
						? mockUserGrants[userId][existingIndex].id
						: `grant-${Date.now()}`,
				sharingTagId: body.sharingTagId,
				sharingTagName: tag.name,
				accessMode: body.accessMode,
				createdAt:
					existingIndex >= 0
						? mockUserGrants[userId][existingIndex].createdAt
						: new Date().toISOString(),
			};

			if (existingIndex >= 0) {
				mockUserGrants[userId][existingIndex] = grant;
			} else {
				mockUserGrants[userId].push(grant);
			}

			return HttpResponse.json(grant);
		},
	),

	// Remove a grant from a user
	http.delete(
		"/api/v1/users/:userId/sharing-tags/:tagId",
		async ({ params }) => {
			await delay(100);
			const userId = params.userId as string;
			const tagId = params.tagId as string;

			if (mockUserGrants[userId]) {
				mockUserGrants[userId] = mockUserGrants[userId].filter(
					(g) => g.sharingTagId !== tagId,
				);
			}

			return new HttpResponse(null, { status: 204 });
		},
	),
];
