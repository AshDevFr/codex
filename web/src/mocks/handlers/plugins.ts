/**
 * Plugin API mock handlers
 *
 * Provides mock data for:
 * - Admin plugin management (CRUD, enable/disable, test, health)
 * - Plugin actions (get actions, execute, metadata search)
 * - Metadata preview/apply for series and books
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type PluginDto = components["schemas"]["PluginDto"];
type PluginSearchResultDto = components["schemas"]["PluginSearchResultDto"];

// Mock plugin data
const mockPlugins: PluginDto[] = [
	{
		id: "plugin-mangabaka",
		name: "mangabaka",
		displayName: "MangaBaka",
		description: "Fetches manga metadata from MangaUpdates/Baka-Updates",
		pluginType: "system",
		command: "npx",
		args: ["@codex/plugin-mangabaka"],
		workingDirectory: null,
		env: { LOG_LEVEL: "info" },
		permissions: [
			"metadata:read",
			"metadata:write:title",
			"metadata:write:summary",
			"metadata:write:genres",
			"metadata:write:tags",
			"metadata:write:year",
			"metadata:write:status",
		],
		scopes: ["library:detail", "series:detail", "series:bulk"],
		libraryIds: [],
		credentialDelivery: "env",
		hasCredentials: true,
		config: { rate_limit: 60 },
		enabled: true,
		healthStatus: "healthy",
		failureCount: 0,
		lastFailureAt: null,
		lastSuccessAt: "2024-01-20T12:00:00Z",
		disabledReason: null,
		manifest: {
			name: "mangabaka",
			displayName: "MangaBaka",
			version: "1.0.0",
			protocolVersion: "1.0",
			description: "Fetches manga metadata from MangaUpdates/Baka-Updates",
			author: "Codex Team",
			capabilities: {
				metadataProvider: ["series"],
				userSyncProvider: false,
			},
			contentTypes: ["series"],
			requiredCredentials: [
				{
					key: "api_key",
					label: "API Key",
					required: true,
					sensitive: true,
					credentialType: "password",
				},
			],
			scopes: ["series:detail", "series:bulk"],
		},
		createdAt: "2024-01-15T00:00:00Z",
		updatedAt: "2024-01-20T00:00:00Z",
	},
	{
		id: "plugin-comicvine",
		name: "comicvine",
		displayName: "ComicVine",
		description: "Fetches comic metadata from ComicVine",
		pluginType: "system",
		command: "python",
		args: ["-m", "comicvine_plugin"],
		workingDirectory: null,
		env: {},
		permissions: [
			"metadata:read",
			"metadata:write:title",
			"metadata:write:summary",
			"metadata:write:genres",
			"metadata:write:covers",
		],
		scopes: ["series:detail"],
		libraryIds: [],
		credentialDelivery: "env",
		hasCredentials: false,
		config: {},
		enabled: false,
		healthStatus: "unhealthy",
		failureCount: 3,
		lastFailureAt: "2024-01-18T15:30:00Z",
		lastSuccessAt: "2024-01-17T10:00:00Z",
		disabledReason: "Disabled after 3 failures in 3600 seconds",
		manifest: null,
		createdAt: "2024-01-10T00:00:00Z",
		updatedAt: "2024-01-18T00:00:00Z",
	},
];

// Mock search results
const mockSearchResults: PluginSearchResultDto[] = [
	{
		externalId: "mu-12345",
		title: "One Piece",
		alternateTitles: ["ワンピース", "Wan Pīsu"],
		year: 1997,
		coverUrl: "https://cdn.mangaupdates.com/covers/one-piece.jpg",
		relevanceScore: 0.98,
		preview: {
			status: "Ongoing",
			genres: ["Action", "Adventure", "Comedy", "Fantasy"],
			rating: 9.2,
			description: "Follows the adventures of Monkey D. Luffy...",
		},
	},
	{
		externalId: "mu-67890",
		title: "One Piece: Strong World",
		alternateTitles: ["One Piece Movie 10"],
		year: 2009,
		coverUrl: "https://cdn.mangaupdates.com/covers/one-piece-strong-world.jpg",
		relevanceScore: 0.75,
		preview: {
			status: "Completed",
			genres: ["Action", "Adventure"],
			rating: 8.5,
			description: "A movie adaptation...",
		},
	},
	{
		externalId: "mu-11111",
		title: "One Punch Man",
		alternateTitles: ["ワンパンマン", "Wanpanman"],
		year: 2012,
		coverUrl: "https://cdn.mangaupdates.com/covers/one-punch-man.jpg",
		relevanceScore: 0.65,
		preview: {
			status: "Ongoing",
			genres: ["Action", "Comedy", "Superhero"],
			rating: 9.0,
			description: "The story of Saitama...",
		},
	},
];

// Mock metadata preview response
const createMockPreview = (externalId: string) => ({
	fields: [
		{
			field: "title",
			currentValue: "One Piece",
			proposedValue: "ONE PIECE",
			status: "will_apply",
		},
		{
			field: "summary",
			currentValue: "An epic pirate adventure...",
			proposedValue:
				"Gol D. Roger was known as the Pirate King, the strongest and most infamous...",
			status: "will_apply",
		},
		{
			field: "year",
			currentValue: 1997,
			proposedValue: 1997,
			status: "unchanged",
		},
		{
			field: "status",
			currentValue: "ongoing",
			proposedValue: "ongoing",
			status: "unchanged",
		},
		{
			field: "genres",
			currentValue: ["Action", "Adventure"],
			proposedValue: ["Action", "Adventure", "Comedy", "Fantasy"],
			status: "will_apply",
		},
		{
			field: "tags",
			currentValue: ["pirates"],
			proposedValue: ["pirates", "treasure", "world-building", "friendship"],
			status: "locked",
			reason: "Field is locked by user",
		},
		{
			field: "publisher",
			currentValue: "Shueisha",
			proposedValue: "Shueisha Inc.",
			status: "no_permission",
			reason: "Plugin lacks metadata:write:publisher permission",
		},
		{
			field: "language",
			currentValue: null,
			proposedValue: null,
			status: "not_provided",
		},
	],
	summary: {
		willApply: 3,
		locked: 1,
		noPermission: 1,
		unchanged: 2,
		notProvided: 1,
	},
	pluginId: "plugin-mangabaka",
	pluginName: "MangaBaka",
	externalId,
	externalUrl: `https://www.mangaupdates.com/series/${externalId}`,
});

export const pluginsHandlers = [
	// ============================================
	// Admin Plugin Management
	// ============================================

	// List all plugins (admin)
	http.get("/api/v1/admin/plugins", async () => {
		await delay(150);
		return HttpResponse.json({
			plugins: mockPlugins,
			total: mockPlugins.length,
		});
	}),

	// Get single plugin (admin)
	http.get("/api/v1/admin/plugins/:id", async ({ params }) => {
		await delay(100);
		const plugin = mockPlugins.find((p) => p.id === params.id);
		if (!plugin) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		return HttpResponse.json(plugin);
	}),

	// Create plugin (admin)
	http.post("/api/v1/admin/plugins", async ({ request }) => {
		await delay(200);
		const body = (await request.json()) as Record<string, unknown>;
		const newPlugin = {
			id: `plugin-${Date.now()}`,
			...body,
			failureCount: 0,
			disabledReason: null,
			manifest: null,
			createdAt: new Date().toISOString(),
			updatedAt: new Date().toISOString(),
		};
		return HttpResponse.json(newPlugin, { status: 201 });
	}),

	// Update plugin (admin)
	http.patch("/api/v1/admin/plugins/:id", async ({ params, request }) => {
		await delay(150);
		const plugin = mockPlugins.find((p) => p.id === params.id);
		if (!plugin) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		const body = (await request.json()) as Record<string, unknown>;
		const updated = { ...plugin, ...body, updatedAt: new Date().toISOString() };
		return HttpResponse.json(updated);
	}),

	// Delete plugin (admin)
	http.delete("/api/v1/admin/plugins/:id", async ({ params }) => {
		await delay(100);
		const plugin = mockPlugins.find((p) => p.id === params.id);
		if (!plugin) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		return new HttpResponse(null, { status: 204 });
	}),

	// Enable plugin (admin)
	http.post("/api/v1/admin/plugins/:id/enable", async ({ params }) => {
		await delay(150);
		const pluginIndex = mockPlugins.findIndex((p) => p.id === params.id);
		if (pluginIndex === -1) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		// Update the mock state
		mockPlugins[pluginIndex] = {
			...mockPlugins[pluginIndex],
			enabled: true,
			healthStatus: "unknown",
			disabledReason: null,
			updatedAt: new Date().toISOString(),
		};
		return HttpResponse.json({
			plugin: mockPlugins[pluginIndex],
			message: "Plugin enabled successfully",
		});
	}),

	// Disable plugin (admin)
	http.post("/api/v1/admin/plugins/:id/disable", async ({ params }) => {
		await delay(150);
		const pluginIndex = mockPlugins.findIndex((p) => p.id === params.id);
		if (pluginIndex === -1) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		// Update the mock state
		mockPlugins[pluginIndex] = {
			...mockPlugins[pluginIndex],
			enabled: false,
			healthStatus: "disabled",
			updatedAt: new Date().toISOString(),
		};
		return HttpResponse.json({
			plugin: mockPlugins[pluginIndex],
			message: "Plugin disabled successfully",
		});
	}),

	// Test plugin (admin)
	http.post("/api/v1/admin/plugins/:id/test", async ({ params }) => {
		await delay(500); // Simulate plugin spawn time
		const plugin = mockPlugins.find((p) => p.id === params.id);
		if (!plugin) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		return HttpResponse.json({
			success: true,
			manifest: plugin.manifest,
			message: "Plugin test successful",
			latencyMs: 450,
		});
	}),

	// Get plugin health (admin)
	http.get("/api/v1/admin/plugins/:id/health", async ({ params }) => {
		await delay(100);
		const plugin = mockPlugins.find((p) => p.id === params.id);
		if (!plugin) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		return HttpResponse.json({
			pluginId: plugin.id,
			status: plugin.enabled ? "healthy" : "disabled",
			failureCount: plugin.failureCount,
			disabledReason: plugin.disabledReason,
			lastCheckedAt: new Date().toISOString(),
		});
	}),

	// Reset plugin failures (admin)
	http.post("/api/v1/admin/plugins/:id/reset", async ({ params }) => {
		await delay(100);
		const pluginIndex = mockPlugins.findIndex((p) => p.id === params.id);
		if (pluginIndex === -1) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		// Update the mock state
		mockPlugins[pluginIndex] = {
			...mockPlugins[pluginIndex],
			failureCount: 0,
			disabledReason: null,
			healthStatus: mockPlugins[pluginIndex].enabled ? "unknown" : "disabled",
			updatedAt: new Date().toISOString(),
		};
		return HttpResponse.json({
			plugin: mockPlugins[pluginIndex],
			message: "Plugin failures reset successfully",
		});
	}),

	// Get plugin failures (admin)
	http.get(
		"/api/v1/admin/plugins/:id/failures",
		async ({ params, request }) => {
			await delay(100);
			const plugin = mockPlugins.find((p) => p.id === params.id);
			if (!plugin) {
				return HttpResponse.json(
					{ error: "Plugin not found" },
					{ status: 404 },
				);
			}

			const url = new URL(request.url);
			const limit = Number.parseInt(url.searchParams.get("limit") || "20", 10);

			// Generate mock failures for the unhealthy plugin
			const failures =
				plugin.healthStatus === "unhealthy" || plugin.failureCount > 0
					? [
							{
								id: "failure-1",
								errorMessage: "Connection timeout after 30s",
								errorCode: "TIMEOUT",
								method: "metadata/series/search",
								context: { query: "One Piece" },
								occurredAt: "2024-01-18T15:30:00Z",
							},
							{
								id: "failure-2",
								errorMessage: "Rate limited by provider",
								errorCode: "RATE_LIMITED",
								method: "metadata/series/search",
								context: { query: "Naruto", retryAfterSeconds: 60 },
								occurredAt: "2024-01-18T15:25:00Z",
							},
							{
								id: "failure-3",
								errorMessage: "API key is invalid or expired",
								errorCode: "AUTH_FAILED",
								method: "initialize",
								context: null,
								occurredAt: "2024-01-18T15:20:00Z",
							},
						].slice(0, limit)
					: [];

			return HttpResponse.json({
				failures,
				total: failures.length,
				windowFailures: plugin.failureCount,
				windowSeconds: 3600,
				threshold: 3,
			});
		},
	),

	// ============================================
	// Plugin Actions (User-facing)
	// ============================================

	// Get available plugin actions for a scope
	http.get("/api/v1/plugins/actions", async ({ request }) => {
		await delay(100);
		const url = new URL(request.url);
		const scope = url.searchParams.get("scope");

		// Filter plugins by scope and enabled status
		const enabledPlugins = mockPlugins.filter(
			(p) => p.enabled && p.scopes.includes(scope || ""),
		);

		const actions = enabledPlugins.map((plugin) => ({
			pluginId: plugin.id,
			pluginName: plugin.name,
			pluginDisplayName: plugin.displayName,
			actionType: "metadata_search",
			label: `Search ${plugin.displayName}`,
			description: plugin.manifest?.description,
			icon: null,
		}));

		return HttpResponse.json({
			actions,
			scope,
		});
	}),

	// Execute plugin action
	http.post("/api/v1/plugins/:id/execute", async ({ params, request }) => {
		await delay(300);
		const plugin = mockPlugins.find((p) => p.id === params.id);
		if (!plugin) {
			return HttpResponse.json({ error: "Plugin not found" }, { status: 404 });
		}
		if (!plugin.enabled) {
			return HttpResponse.json(
				{ error: "Plugin is disabled" },
				{ status: 400 },
			);
		}

		const body = (await request.json()) as {
			action:
				| {
						metadata: {
							action: "search" | "get" | "match";
							content_type: "series";
							params: Record<string, unknown>;
						};
				  }
				| "ping";
		};

		// Handle ping action
		if (body.action === "ping") {
			return HttpResponse.json({
				success: true,
				result: "pong",
				latencyMs: 50,
			});
		}

		// Handle metadata actions
		if (typeof body.action === "object" && "metadata" in body.action) {
			const {
				action,
				content_type,
				params: actionParams,
			} = body.action.metadata;

			// Handle search action
			if (action === "search" && content_type === "series") {
				const query = (actionParams.query as string) || "";
				const results = mockSearchResults.filter(
					(r) =>
						r.title.toLowerCase().includes(query.toLowerCase()) ||
						r.alternateTitles?.some((t) =>
							t.toLowerCase().includes(query.toLowerCase()),
						),
				);
				return HttpResponse.json({
					success: true,
					result: { results },
					latencyMs: 280,
				});
			}

			// Handle get action
			if (action === "get" && content_type === "series") {
				const externalId = actionParams.externalId as string;
				const result = mockSearchResults.find(
					(r) => r.externalId === externalId,
				);
				if (!result) {
					return HttpResponse.json({
						success: false,
						error: "External ID not found",
						latencyMs: 100,
					});
				}
				return HttpResponse.json({
					success: true,
					result: {
						title: result.title,
						alternateTitles: result.alternateTitles,
						year: result.year,
						status: result.preview?.status?.toLowerCase(),
						genres: result.preview?.genres,
						summary: result.preview?.description,
						coverUrl: result.coverUrl,
					},
					latencyMs: 250,
				});
			}

			return HttpResponse.json({
				success: false,
				error: `Unknown metadata action: ${action}`,
				latencyMs: 50,
			});
		}

		return HttpResponse.json({
			success: false,
			error: "Unknown action format",
			latencyMs: 50,
		});
	}),

	// ============================================
	// Metadata Preview/Apply
	// ============================================

	// Preview series metadata
	http.post(
		"/api/v1/series/:seriesId/metadata/preview",
		async ({ request }) => {
			await delay(400);
			const body = (await request.json()) as {
				pluginId: string;
				externalId: string;
			};
			return HttpResponse.json(createMockPreview(body.externalId));
		},
	),

	// Apply series metadata
	http.post("/api/v1/series/:seriesId/metadata/apply", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as {
			pluginId: string;
			externalId: string;
			fields?: string[];
		};

		// Simulate applying only writable fields
		const appliedFields = body.fields || ["title", "summary", "genres"];
		const skippedFields = [
			{ field: "tags", reason: "Field is locked" },
			{ field: "publisher", reason: "No permission" },
		];

		return HttpResponse.json({
			success: true,
			appliedFields,
			skippedFields,
			message: `Applied ${appliedFields.length} fields from plugin`,
		});
	}),

	// Preview book metadata
	http.post("/api/v1/books/:bookId/metadata/preview", async ({ request }) => {
		await delay(400);
		const body = (await request.json()) as {
			pluginId: string;
			externalId: string;
		};
		// Reuse series preview for simplicity
		return HttpResponse.json(createMockPreview(body.externalId));
	}),

	// Apply book metadata
	http.post("/api/v1/books/:bookId/metadata/apply", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as {
			pluginId: string;
			externalId: string;
			fields?: string[];
		};

		const appliedFields = body.fields || ["title", "summary"];
		return HttpResponse.json({
			success: true,
			appliedFields,
			skippedFields: [],
			message: `Applied ${appliedFields.length} fields from plugin`,
		});
	}),

	// Auto-match series metadata
	http.post(
		"/api/v1/series/:seriesId/metadata/auto-match",
		async ({ request }) => {
			await delay(600); // Simulate search + fetch + apply
			// Parse body to validate request format
			await request.json();

			// Simulate finding a match
			const bestMatch = mockSearchResults[0]; // Use first result as best match
			const appliedFields = ["title", "summary", "genres", "year", "status"];
			const skippedFields = [
				{ field: "tags", reason: "Field is locked" },
				{ field: "publisher", reason: "No permission" },
			];

			return HttpResponse.json({
				success: true,
				matchedResult: bestMatch,
				appliedFields,
				skippedFields,
				message: `Matched '${bestMatch.title}' and applied ${appliedFields.length} field(s)`,
				externalUrl: `https://www.mangaupdates.com/series/${bestMatch.externalId}`,
			});
		},
	),

	// Enqueue auto-match task for a single series
	http.post(
		"/api/v1/series/:seriesId/metadata/auto-match/task",
		async ({ params }) => {
			await delay(100);
			const taskId = `task-${Date.now()}`;
			return HttpResponse.json({
				success: true,
				tasksEnqueued: 1,
				taskIds: [taskId],
				message: `Enqueued auto-match task for series ${params.seriesId}`,
			});
		},
	),

	// Bulk enqueue auto-match tasks for multiple series
	http.post(
		"/api/v1/series/metadata/auto-match/task/bulk",
		async ({ request }) => {
			await delay(200);
			const body = (await request.json()) as {
				pluginId: string;
				seriesIds: string[];
			};
			const taskIds = body.seriesIds.map(
				(_, i) => `task-bulk-${Date.now()}-${i}`,
			);
			return HttpResponse.json({
				success: true,
				tasksEnqueued: body.seriesIds.length,
				taskIds,
				message: `Enqueued ${body.seriesIds.length} auto-match task(s)`,
			});
		},
	),

	// Enqueue auto-match tasks for all series in a library
	http.post(
		"/api/v1/libraries/:libraryId/metadata/auto-match/task",
		async () => {
			await delay(300);
			// Simulate enqueueing tasks for 10 series
			const taskIds = Array.from(
				{ length: 10 },
				(_, i) => `task-lib-${Date.now()}-${i}`,
			);
			return HttpResponse.json({
				success: true,
				tasksEnqueued: 10,
				taskIds,
				message: "Enqueued 10 auto-match task(s) for library",
			});
		},
	),
];

// Export mock data for testing
export const getMockPlugins = () => [...mockPlugins];
export const getMockSearchResults = () => [...mockSearchResults];
