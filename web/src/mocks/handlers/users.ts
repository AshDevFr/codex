/**
 * MSW handlers for user management API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import { createList, createUser } from "../data/factories";

// Generate mock users
const mockUsers = [
	createUser({
		id: "admin-user-id",
		username: "admin",
		email: "admin@example.com",
		role: "admin",
	}),
	createUser({
		id: "maintainer-user-id",
		username: "maintainer",
		email: "maintainer@example.com",
		role: "maintainer",
	}),
	createUser({
		id: "reader-user-id",
		username: "reader",
		email: "reader@example.com",
		role: "reader",
	}),
	...createList(() => createUser(), 7),
];

// Mock user preferences
const mockUserPreferences: Array<{
	key: string;
	value: unknown;
	updatedAt: string;
}> = [
	{ key: "theme", value: "system", updatedAt: "2024-01-01T00:00:00Z" },
	{
		key: "library.defaultView",
		value: "grid",
		updatedAt: "2024-01-01T00:00:00Z",
	},
	{ key: "library.itemsPerPage", value: 20, updatedAt: "2024-01-01T00:00:00Z" },
	{
		key: "reader.readingDirection",
		value: "ltr",
		updatedAt: "2024-01-01T00:00:00Z",
	},
	{ key: "reader.fitMode", value: "width", updatedAt: "2024-01-01T00:00:00Z" },
	{
		key: "notifications.enabled",
		value: true,
		updatedAt: "2024-01-01T00:00:00Z",
	},
];

// Mock user ratings (0-100 scale)
const mockUserRatings = [
	{
		id: "rating-1",
		seriesId: "series-1",
		rating: 95,
		notes: "Incredible world-building!",
		createdAt: "2024-06-01T00:00:00Z",
	},
	{
		id: "rating-2",
		seriesId: "series-2",
		rating: 85,
		notes: null,
		createdAt: "2024-06-02T00:00:00Z",
	},
	{
		id: "rating-3",
		seriesId: "series-3",
		rating: 90,
		notes: "A masterpiece",
		createdAt: "2024-06-03T00:00:00Z",
	},
];

export const usersHandlers = [
	// List all users (paginated, 1-indexed)
	http.get("/api/v1/users", async ({ request }) => {
		await delay(100);
		const url = new URL(request.url);
		const page = Math.max(1, parseInt(url.searchParams.get("page") || "1", 10));
		const pageSize = parseInt(url.searchParams.get("pageSize") || "50", 10);
		const role = url.searchParams.get("role");
		// sharingTag filter available but not implemented in mock (would require mock sharing tag grants)
		// const sharingTag = url.searchParams.get("sharingTag");

		// Apply filters
		let filteredUsers = [...mockUsers];
		if (role) {
			filteredUsers = filteredUsers.filter((u) => u.role === role);
		}

		// Apply pagination (1-indexed)
		const total = filteredUsers.length;
		const totalPages = Math.ceil(total / pageSize) || 1;
		const start = (page - 1) * pageSize;
		const end = start + pageSize;
		const paginatedUsers = filteredUsers.slice(start, end);

		// Build HATEOAS links
		const basePath = "/api/v1/users";
		const links = {
			self: `${basePath}?page=${page}&pageSize=${pageSize}`,
			first: `${basePath}?page=1&pageSize=${pageSize}`,
			prev:
				page > 1 ? `${basePath}?page=${page - 1}&pageSize=${pageSize}` : null,
			next:
				page < totalPages
					? `${basePath}?page=${page + 1}&pageSize=${pageSize}`
					: null,
			last: `${basePath}?page=${totalPages}&pageSize=${pageSize}`,
		};

		return HttpResponse.json({
			data: paginatedUsers,
			page,
			pageSize,
			total,
			totalPages,
			links,
		});
	}),

	// Get single user
	http.get("/api/v1/users/:userId", async ({ params }) => {
		await delay(50);
		const { userId } = params;
		const user = mockUsers.find((u) => u.id === userId);

		if (!user) {
			return new HttpResponse(null, { status: 404 });
		}

		return HttpResponse.json(user);
	}),

	// Create user
	http.post("/api/v1/users", async ({ request }) => {
		await delay(100);
		const body = (await request.json()) as {
			username: string;
			email: string;
			password: string;
			role?: "reader" | "maintainer" | "admin";
		};

		const newUser = createUser({
			username: body.username,
			email: body.email,
			role: body.role ?? "reader",
		});

		mockUsers.push(newUser);
		return HttpResponse.json(newUser, { status: 201 });
	}),

	// Update user
	http.patch("/api/v1/users/:userId", async ({ params, request }) => {
		await delay(100);
		const { userId } = params;
		const body = (await request.json()) as Partial<{
			username: string;
			email: string;
			role: "reader" | "maintainer" | "admin";
			isActive: boolean;
			permissions: string[];
		}>;

		const userIndex = mockUsers.findIndex((u) => u.id === userId);
		if (userIndex === -1) {
			return new HttpResponse(null, { status: 404 });
		}

		mockUsers[userIndex] = {
			...mockUsers[userIndex],
			...body,
			updatedAt: new Date().toISOString(),
		};

		return HttpResponse.json(mockUsers[userIndex]);
	}),

	// Delete user
	http.delete("/api/v1/users/:userId", async ({ params }) => {
		await delay(100);
		const { userId } = params;
		const userIndex = mockUsers.findIndex((u) => u.id === userId);

		if (userIndex === -1) {
			return new HttpResponse(null, { status: 404 });
		}

		mockUsers.splice(userIndex, 1);
		return new HttpResponse(null, { status: 204 });
	}),

	// Change user password
	http.post("/api/v1/users/:userId/password", async ({ params }) => {
		await delay(100);
		const { userId } = params;
		const user = mockUsers.find((u) => u.id === userId);

		if (!user) {
			return new HttpResponse(null, { status: 404 });
		}

		return HttpResponse.json({ message: "Password updated successfully" });
	}),

	// ============================================
	// User Preferences
	// ============================================

	// Get all user preferences
	http.get("/api/v1/user/preferences", async () => {
		await delay(100);
		return HttpResponse.json({
			preferences: mockUserPreferences,
		});
	}),

	// Get a specific preference
	http.get("/api/v1/user/preferences/:key", async ({ params }) => {
		await delay(50);
		const pref = mockUserPreferences.find((p) => p.key === params.key);

		if (!pref) {
			return HttpResponse.json(
				{ error: "Preference not found" },
				{ status: 404 },
			);
		}

		return HttpResponse.json(pref);
	}),

	// Set a specific preference
	http.put("/api/v1/user/preferences/:key", async ({ params, request }) => {
		await delay(100);
		const body = (await request.json()) as { value: unknown };
		const now = new Date().toISOString();

		const existingIndex = mockUserPreferences.findIndex(
			(p) => p.key === params.key,
		);
		const pref = {
			key: params.key as string,
			value: body.value,
			updatedAt: now,
		};

		if (existingIndex >= 0) {
			mockUserPreferences[existingIndex] = pref;
		} else {
			mockUserPreferences.push(pref);
		}

		return HttpResponse.json(pref);
	}),

	// Set multiple preferences (bulk)
	http.put("/api/v1/user/preferences", async ({ request }) => {
		await delay(100);
		const body = (await request.json()) as Record<string, unknown>;
		const now = new Date().toISOString();

		for (const [key, value] of Object.entries(body)) {
			const existingIndex = mockUserPreferences.findIndex((p) => p.key === key);
			const pref = { key, value, updatedAt: now };

			if (existingIndex >= 0) {
				mockUserPreferences[existingIndex] = pref;
			} else {
				mockUserPreferences.push(pref);
			}
		}

		return HttpResponse.json({ preferences: mockUserPreferences });
	}),

	// Delete a preference
	http.delete("/api/v1/user/preferences/:key", async ({ params }) => {
		await delay(50);
		const index = mockUserPreferences.findIndex((p) => p.key === params.key);

		if (index >= 0) {
			mockUserPreferences.splice(index, 1);
		}

		return new HttpResponse(null, { status: 204 });
	}),

	// ============================================
	// User Ratings (across all series)
	// ============================================

	// Get all ratings for current user
	http.get("/api/v1/user/ratings", async () => {
		await delay(100);
		return HttpResponse.json({
			ratings: mockUserRatings,
		});
	}),
];
