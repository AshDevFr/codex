/**
 * MSW handlers for settings API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import {
	createList,
	createSetting,
	createSettingHistory,
} from "../data/factories";


// Mock system integrations (admin-managed)
const mockSystemIntegrations: Array<{
	id: string;
	name: string;
	type: string;
	isEnabled: boolean;
	config: Record<string, unknown>;
	lastTestAt: string | null;
	lastTestResult: string | null;
	createdAt: string;
	updatedAt: string;
}> = [
	{
		id: "integration-komga",
		name: "Komga Sync",
		type: "komga",
		isEnabled: true,
		config: { baseUrl: "https://komga.example.com", apiKey: "***" },
		lastTestAt: "2024-06-15T10:00:00Z",
		lastTestResult: "success",
		createdAt: "2024-01-01T00:00:00Z",
		updatedAt: "2024-06-15T10:00:00Z",
	},
	{
		id: "integration-mal-system",
		name: "MyAnimeList Metadata",
		type: "myanimelist",
		isEnabled: false,
		config: { clientId: "***" },
		lastTestAt: null,
		lastTestResult: null,
		createdAt: "2024-01-01T00:00:00Z",
		updatedAt: "2024-01-01T00:00:00Z",
	},
];

// Generate mock settings data matching the actual database seed
const mockSettings = [
	// Scanner settings
	createSetting({
		key: "scanner.scan_timeout_minutes",
		value: "120",
		value_type: "integer",
		category: "Scanner",
		description: "Maximum time (in minutes) for a single scan before timeout",
		default_value: "120",
	}),
	createSetting({
		key: "scanner.retry_failed_files",
		value: "false",
		value_type: "boolean",
		category: "Scanner",
		description: "Automatically retry files that failed to scan",
		default_value: "false",
	}),
	createSetting({
		key: "scanner.batch_size",
		value: "100",
		value_type: "integer",
		category: "Scanner",
		description:
			"Number of files to process in each batch during library scanning",
		default_value: "100",
	}),
	createSetting({
		key: "scanner.parallel_hashing",
		value: "8",
		value_type: "integer",
		category: "Scanner",
		description: "Number of files to hash concurrently during scanning",
		default_value: "8",
	}),
	createSetting({
		key: "scanner.parallel_series",
		value: "4",
		value_type: "integer",
		category: "Scanner",
		description: "Number of series to process concurrently during scanning",
		default_value: "4",
	}),
	// Application settings
	createSetting({
		key: "application.name",
		value: "Codex",
		value_type: "string",
		category: "Application",
		description: "Application display name (for branding/white-labeling)",
		default_value: "Codex",
	}),
	// Authentication settings
	createSetting({
		key: "auth.registration_enabled",
		value: "false",
		value_type: "boolean",
		category: "Authentication",
		description: "Allow new users to register accounts",
		default_value: "false",
	}),
	// Task settings
	createSetting({
		key: "task.poll_interval_seconds",
		value: "5",
		value_type: "integer",
		category: "Task",
		description: "Interval (in seconds) for polling task queue",
		default_value: "5",
	}),
	createSetting({
		key: "task.cleanup_interval_seconds",
		value: "30",
		value_type: "integer",
		category: "Task",
		description: "Interval (in seconds) for cleaning up completed tasks",
		default_value: "30",
	}),
	createSetting({
		key: "task.prioritize_scans_over_analysis",
		value: "true",
		value_type: "boolean",
		category: "Task",
		description: "Prioritize scan tasks over analysis tasks in the queue",
		default_value: "true",
	}),
	// Deduplication settings
	createSetting({
		key: "deduplication.enabled",
		value: "true",
		value_type: "boolean",
		category: "Deduplication",
		description: "Enable automatic duplicate detection scanning",
		default_value: "true",
	}),
	createSetting({
		key: "deduplication.cron_schedule",
		value: "",
		value_type: "string",
		category: "Deduplication",
		description: "Cron schedule for automatic duplicate detection",
		default_value: "",
	}),
	// Purge settings
	createSetting({
		key: "purge.purge_empty_series",
		value: "true",
		value_type: "boolean",
		category: "Purge",
		description: "When purging deleted books, also delete empty series",
		default_value: "true",
	}),
	// Thumbnail settings
	createSetting({
		key: "thumbnail.max_dimension",
		value: "400",
		value_type: "integer",
		category: "Thumbnail",
		description: "Maximum width or height for generated thumbnails",
		default_value: "400",
	}),
	createSetting({
		key: "thumbnail.jpeg_quality",
		value: "85",
		value_type: "integer",
		category: "Thumbnail",
		description: "JPEG quality for thumbnail images (1-100)",
		default_value: "85",
	}),
	// Display settings
	createSetting({
		key: "display.custom_metadata_template",
		value: `{{#if custom_metadata}}
## Additional Information

{{#each custom_metadata}}
- **{{@key}}**: {{this}}
{{/each}}
{{/if}}`,
		value_type: "string",
		category: "Display",
		description:
			"Handlebars-style Markdown template for displaying custom metadata on series detail pages.",
		default_value: "",
	}),
];

export const settingsHandlers = [
	// ============================================
	// Branding Settings (unauthenticated)
	// ============================================

	// Get branding settings (unauthenticated - used on login page)
	http.get("/api/v1/settings/branding", async () => {
		await delay(50);
		const appNameSetting = mockSettings.find(
			(s) => s.key === "application.name",
		);
		return HttpResponse.json({
			application_name: appNameSetting?.value || "Codex",
		});
	}),

	// ============================================
	// Admin Settings
	// ============================================

	// List all settings
	http.get("/api/v1/admin/settings", async ({ request }) => {
		await delay(100);
		const url = new URL(request.url);
		const category = url.searchParams.get("category");

		let filteredSettings = mockSettings;
		if (category) {
			filteredSettings = mockSettings.filter((s) => s.category === category);
		}

		return HttpResponse.json(filteredSettings);
	}),

	// Get single setting
	http.get("/api/v1/admin/settings/:settingKey", async ({ params }) => {
		await delay(50);
		const { settingKey } = params;
		const setting = mockSettings.find((s) => s.key === settingKey);

		if (!setting) {
			return new HttpResponse(null, { status: 404 });
		}

		return HttpResponse.json(setting);
	}),

	// Update setting
	http.put(
		"/api/v1/admin/settings/:settingKey",
		async ({ params, request }) => {
			await delay(100);
			const { settingKey } = params;
			const body = (await request.json()) as { value: string };
			const settingIndex = mockSettings.findIndex((s) => s.key === settingKey);

			if (settingIndex === -1) {
				return new HttpResponse(null, { status: 404 });
			}

			mockSettings[settingIndex] = {
				...mockSettings[settingIndex],
				value: body.value,
				updated_at: new Date().toISOString(),
				version: mockSettings[settingIndex].version + 1,
			};

			return HttpResponse.json(mockSettings[settingIndex]);
		},
	),

	// Reset setting to default
	http.post("/api/v1/admin/settings/:settingKey/reset", async ({ params }) => {
		await delay(100);
		const { settingKey } = params;
		const settingIndex = mockSettings.findIndex((s) => s.key === settingKey);

		if (settingIndex === -1) {
			return new HttpResponse(null, { status: 404 });
		}

		mockSettings[settingIndex] = {
			...mockSettings[settingIndex],
			value: mockSettings[settingIndex].default_value,
			updated_at: new Date().toISOString(),
			version: mockSettings[settingIndex].version + 1,
		};

		return HttpResponse.json(mockSettings[settingIndex]);
	}),

	// Bulk update settings
	http.post("/api/v1/admin/settings/bulk", async ({ request }) => {
		await delay(150);
		const body = (await request.json()) as {
			settings: Array<{ key: string; value: string }>;
		};

		const updatedSettings = body.settings
			.map((update) => {
				const settingIndex = mockSettings.findIndex(
					(s) => s.key === update.key,
				);
				if (settingIndex !== -1) {
					mockSettings[settingIndex] = {
						...mockSettings[settingIndex],
						value: update.value,
						updated_at: new Date().toISOString(),
						version: mockSettings[settingIndex].version + 1,
					};
					return mockSettings[settingIndex];
				}
				return null;
			})
			.filter(Boolean);

		return HttpResponse.json(updatedSettings);
	}),

	// Get setting history
	http.get("/api/v1/admin/settings/:settingKey/history", async ({ params }) => {
		await delay(100);
		const { settingKey } = params;

		const history = createList(
			() => createSettingHistory({ key: settingKey as string }),
			5,
		);

		return HttpResponse.json(history);
	}),

	// ============================================
	// Public Settings (non-admin)
	// ============================================

	// Get public settings (accessible to all authenticated users)
	http.get("/api/v1/settings/public", async () => {
		await delay(50);
		// Return a subset of settings that are safe for non-admin users
		const publicSettings = {
			applicationName:
				mockSettings.find((s) => s.key === "application.name")?.value || "Codex",
			registrationEnabled:
				mockSettings.find((s) => s.key === "auth.registration_enabled")
					?.value === "true",
			version: "1.0.0",
		};
		return HttpResponse.json(publicSettings);
	}),

	// ============================================
	// System Integrations (Admin)
	// ============================================

	// List all system integrations
	http.get("/api/v1/admin/integrations", async () => {
		await delay(100);
		return HttpResponse.json({ integrations: mockSystemIntegrations });
	}),

	// Get a specific integration
	http.get("/api/v1/admin/integrations/:id", async ({ params }) => {
		await delay(50);
		const integration = mockSystemIntegrations.find((i) => i.id === params.id);

		if (!integration) {
			return HttpResponse.json(
				{ error: "Integration not found" },
				{ status: 404 },
			);
		}

		return HttpResponse.json(integration);
	}),

	// Create a new integration
	http.post("/api/v1/admin/integrations", async ({ request }) => {
		await delay(150);
		const body = (await request.json()) as {
			name: string;
			type: string;
			config?: Record<string, unknown>;
		};
		const now = new Date().toISOString();

		const integration = {
			id: `integration-${Date.now()}`,
			name: body.name,
			type: body.type,
			isEnabled: false,
			config: body.config || {},
			lastTestAt: null,
			lastTestResult: null,
			createdAt: now,
			updatedAt: now,
		};

		mockSystemIntegrations.push(integration);
		return HttpResponse.json(integration, { status: 201 });
	}),

	// Update an integration
	http.patch("/api/v1/admin/integrations/:id", async ({ params, request }) => {
		await delay(100);
		const body = (await request.json()) as {
			name?: string;
			config?: Record<string, unknown>;
		};
		const integration = mockSystemIntegrations.find((i) => i.id === params.id);

		if (!integration) {
			return HttpResponse.json(
				{ error: "Integration not found" },
				{ status: 404 },
			);
		}

		if (body.name) integration.name = body.name;
		if (body.config)
			integration.config = { ...integration.config, ...body.config };
		integration.updatedAt = new Date().toISOString();

		return HttpResponse.json(integration);
	}),

	// Delete an integration
	http.delete("/api/v1/admin/integrations/:id", async ({ params }) => {
		await delay(100);
		const index = mockSystemIntegrations.findIndex((i) => i.id === params.id);

		if (index === -1) {
			return HttpResponse.json(
				{ error: "Integration not found" },
				{ status: 404 },
			);
		}

		mockSystemIntegrations.splice(index, 1);
		return new HttpResponse(null, { status: 204 });
	}),

	// Enable an integration
	http.post("/api/v1/admin/integrations/:id/enable", async ({ params }) => {
		await delay(100);
		const integration = mockSystemIntegrations.find((i) => i.id === params.id);

		if (!integration) {
			return HttpResponse.json(
				{ error: "Integration not found" },
				{ status: 404 },
			);
		}

		integration.isEnabled = true;
		integration.updatedAt = new Date().toISOString();

		return HttpResponse.json(integration);
	}),

	// Disable an integration
	http.post("/api/v1/admin/integrations/:id/disable", async ({ params }) => {
		await delay(100);
		const integration = mockSystemIntegrations.find((i) => i.id === params.id);

		if (!integration) {
			return HttpResponse.json(
				{ error: "Integration not found" },
				{ status: 404 },
			);
		}

		integration.isEnabled = false;
		integration.updatedAt = new Date().toISOString();

		return HttpResponse.json(integration);
	}),

	// Test an integration
	http.post("/api/v1/admin/integrations/:id/test", async ({ params }) => {
		await delay(500); // Simulate network test
		const integration = mockSystemIntegrations.find((i) => i.id === params.id);

		if (!integration) {
			return HttpResponse.json(
				{ error: "Integration not found" },
				{ status: 404 },
			);
		}

		// Simulate test result
		const success = Math.random() > 0.2; // 80% success rate
		integration.lastTestAt = new Date().toISOString();
		integration.lastTestResult = success ? "success" : "failed";
		integration.updatedAt = new Date().toISOString();

		if (success) {
			return HttpResponse.json({
				success: true,
				message: "Connection successful",
				details: { latency: Math.floor(Math.random() * 200) + 50 },
			});
		} else {
			return HttpResponse.json(
				{
					success: false,
					message: "Connection failed",
					error: "Could not reach the server",
				},
				{ status: 400 },
			);
		}
	}),
];
