/**
 * MSW handlers for settings API endpoints
 */

import { http, HttpResponse, delay } from "msw";
import { createSetting, createSettingHistory, createList } from "../data/factories";

// Generate mock settings data
const mockSettings = [
	createSetting({ key: "server.name", value: "Codex", category: "server", description: "Server display name" }),
	createSetting({ key: "server.port", value: "8080", value_type: "integer", category: "server", description: "Server port" }),
	createSetting({ key: "auth.registration_enabled", value: "true", value_type: "boolean", category: "auth", description: "Allow user registration" }),
	createSetting({ key: "auth.session_timeout", value: "86400", value_type: "integer", category: "auth", description: "Session timeout in seconds" }),
	createSetting({ key: "scanning.concurrent_jobs", value: "2", value_type: "integer", category: "scanning", description: "Number of concurrent scan jobs" }),
	createSetting({ key: "scanning.deep_scan_interval", value: "604800", value_type: "integer", category: "scanning", description: "Deep scan interval in seconds" }),
	createSetting({ key: "thumbnails.quality", value: "85", value_type: "integer", category: "thumbnails", description: "Thumbnail quality (1-100)" }),
	createSetting({ key: "thumbnails.max_width", value: "400", value_type: "integer", category: "thumbnails", description: "Maximum thumbnail width" }),
];

export const settingsHandlers = [
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
	http.put("/api/v1/admin/settings/:settingKey", async ({ params, request }) => {
		await delay(100);
		const { settingKey } = params;
		const body = await request.json() as { value: string };
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
	}),

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
		const body = await request.json() as { settings: Array<{ key: string; value: string }> };

		const updatedSettings = body.settings.map((update) => {
			const settingIndex = mockSettings.findIndex((s) => s.key === update.key);
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
		}).filter(Boolean);

		return HttpResponse.json(updatedSettings);
	}),

	// Get setting history
	http.get("/api/v1/admin/settings/:settingKey/history", async ({ params }) => {
		await delay(100);
		const { settingKey } = params;

		const history = createList(
			() => createSettingHistory({ key: settingKey as string }),
			5
		);

		return HttpResponse.json(history);
	}),
];
