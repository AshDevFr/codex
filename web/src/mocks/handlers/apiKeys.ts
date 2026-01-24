/**
 * API Keys mock handlers
 */

import { faker } from "@faker-js/faker";
import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import { ROLE_PERMISSIONS } from "@/types/permissions";
import { createPaginatedResponse } from "../data/factories";

type ApiKeyDto = components["schemas"]["ApiKeyDto"];

// In-memory store for API keys
let mockApiKeys: ApiKeyDto[] = [
	{
		id: faker.string.uuid(),
		userId: "admin-user-id",
		name: "Mobile App Key",
		keyPrefix: "codex_abc123",
		permissions: ROLE_PERMISSIONS.admin,
		isActive: true,
		expiresAt: new Date(Date.now() + 90 * 24 * 60 * 60 * 1000).toISOString(),
		lastUsedAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
		createdAt: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
		updatedAt: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
	},
	{
		id: faker.string.uuid(),
		userId: "admin-user-id",
		name: "Read-Only Script",
		keyPrefix: "codex_def456",
		permissions: ["libraries-read", "series-read", "books-read", "pages-read"],
		isActive: true,
		expiresAt: null,
		lastUsedAt: null,
		createdAt: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString(),
		updatedAt: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString(),
	},
];

export const apiKeysHandlers = [
	// List API keys (paginated, 1-indexed)
	http.get("/api/v1/api-keys", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const page = Math.max(
			1,
			Number.parseInt(url.searchParams.get("page") || "1", 10),
		);
		const pageSize = Number.parseInt(
			url.searchParams.get("page_size") || "50",
			10,
		);

		// 1-indexed pagination
		const start = (page - 1) * pageSize;
		const end = start + pageSize;
		const items = mockApiKeys.slice(start, end);

		return HttpResponse.json(
			createPaginatedResponse(items, {
				page,
				pageSize,
				total: mockApiKeys.length,
				basePath: "/api/v1/api-keys",
			}),
		);
	}),

	// Get single API key
	http.get("/api/v1/api-keys/:id", async ({ params }) => {
		await delay(100);
		const key = mockApiKeys.find((k) => k.id === params.id);
		if (!key) {
			return HttpResponse.json({ error: "API key not found" }, { status: 404 });
		}
		return HttpResponse.json(key);
	}),

	// Create API key
	http.post("/api/v1/api-keys", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as {
			name: string;
			permissions?: string[];
			expiresAt?: string | null;
		};

		if (!body.name) {
			return HttpResponse.json({ error: "Name is required" }, { status: 400 });
		}

		// Generate a random key prefix and full key
		const prefix = `codex_${faker.string.alphanumeric(6)}`;
		const fullKey = `${prefix}${faker.string.alphanumeric(32)}`;

		const newKey: ApiKeyDto = {
			id: faker.string.uuid(),
			userId: "admin-user-id",
			name: body.name,
			keyPrefix: prefix,
			permissions: body.permissions || ROLE_PERMISSIONS.admin,
			isActive: true,
			expiresAt: body.expiresAt || null,
			lastUsedAt: null,
			createdAt: new Date().toISOString(),
			updatedAt: new Date().toISOString(),
		};

		mockApiKeys.push(newKey);

		return HttpResponse.json(
			{
				...newKey,
				key: fullKey, // Only returned on creation
			},
			{ status: 201 },
		);
	}),

	// Update API key
	http.patch("/api/v1/api-keys/:id", async ({ params, request }) => {
		await delay(200);
		const body = (await request.json()) as {
			name?: string;
			permissions?: string[];
			isActive?: boolean;
			expiresAt?: string | null;
		};

		const keyIndex = mockApiKeys.findIndex((k) => k.id === params.id);
		if (keyIndex === -1) {
			return HttpResponse.json({ error: "API key not found" }, { status: 404 });
		}

		const updatedKey: ApiKeyDto = {
			...mockApiKeys[keyIndex],
			...(body.name !== undefined && { name: body.name }),
			...(body.permissions !== undefined && { permissions: body.permissions }),
			...(body.isActive !== undefined && { isActive: body.isActive }),
			...(body.expiresAt !== undefined && { expiresAt: body.expiresAt }),
			updatedAt: new Date().toISOString(),
		};

		mockApiKeys[keyIndex] = updatedKey;
		return HttpResponse.json(updatedKey);
	}),

	// Delete API key
	http.delete("/api/v1/api-keys/:id", async ({ params }) => {
		await delay(200);
		const keyIndex = mockApiKeys.findIndex((k) => k.id === params.id);
		if (keyIndex === -1) {
			return HttpResponse.json({ error: "API key not found" }, { status: 404 });
		}

		mockApiKeys = mockApiKeys.filter((k) => k.id !== params.id);
		return new HttpResponse(null, { status: 204 });
	}),
];

// Helper to reset mock state (for testing)
export const resetMockApiKeys = () => {
	mockApiKeys = [
		{
			id: faker.string.uuid(),
			userId: "admin-user-id",
			name: "Mobile App Key",
			keyPrefix: "codex_abc123",
			permissions: ROLE_PERMISSIONS.admin,
			isActive: true,
			expiresAt: new Date(Date.now() + 90 * 24 * 60 * 60 * 1000).toISOString(),
			lastUsedAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
			createdAt: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
			updatedAt: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
		},
	];
};
