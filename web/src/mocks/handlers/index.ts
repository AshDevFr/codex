/**
 * MSW handlers index
 *
 * Aggregates all mock API handlers for the application.
 */

import { delay, HttpResponse, http } from "msw";
import { authHandlers } from "./auth";
import { bookHandlers } from "./books";
import { duplicatesHandlers } from "./duplicates";
import { eventHandlers } from "./events";
import { libraryHandlers } from "./libraries";
import { metadataHandlers } from "./metadata";
import { metricsHandlers } from "./metrics";
import { seriesHandlers } from "./series";
import { settingsHandlers } from "./settings";
import { tasksHandlers } from "./tasks";
import { usersHandlers } from "./users";

// Additional utility handlers
const utilityHandlers = [
	// Health check
	http.get("/api/v1/health", async () => {
		await delay(50);
		return HttpResponse.json({ status: "ok" });
	}),

	// Setup status - configurable via VITE_MOCK_SETUP_REQUIRED env var
	http.get("/api/v1/setup/status", async () => {
		await delay(50);
		const setupRequired = import.meta.env.VITE_MOCK_SETUP_REQUIRED === "true";
		return HttpResponse.json({
			setupRequired,
			hasUsers: !setupRequired,
		});
	}),

	// Setup initialize - create admin user
	http.post("/api/v1/setup/initialize", async ({ request }) => {
		await delay(500);
		const body = (await request.json()) as {
			username: string;
			email: string;
			password: string;
		};

		return HttpResponse.json({
			accessToken:
				"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
			tokenType: "Bearer",
			expiresIn: 86400,
			user: {
				id: "admin-user-id",
				username: body.username,
				email: body.email,
				isAdmin: true,
				createdAt: new Date().toISOString(),
				updatedAt: new Date().toISOString(),
			},
		});
	}),

	// Setup configure settings
	http.patch("/api/v1/setup/settings", async () => {
		await delay(300);
		return HttpResponse.json({
			message: "Settings configured successfully",
		});
	}),

	// Filesystem browse (for library path selection)
	http.get("/api/v1/filesystem/browse", async ({ request }) => {
		await delay(200);
		const url = new URL(request.url);
		const path = url.searchParams.get("path") || "/";

		return HttpResponse.json({
			path,
			entries: [
				{ name: "media", path: `${path}/media`, isDirectory: true, size: 0 },
				{ name: "home", path: `${path}/home`, isDirectory: true, size: 0 },
				{ name: "var", path: `${path}/var`, isDirectory: true, size: 0 },
			],
		});
	}),

	// Filesystem drives
	http.get("/api/v1/filesystem/drives", async () => {
		await delay(100);
		return HttpResponse.json([
			{ name: "/", path: "/", isDirectory: true, size: 0 },
		]);
	}),
];

// Combine all handlers
export const handlers = [
	...authHandlers,
	...libraryHandlers,
	...seriesHandlers,
	...bookHandlers,
	...eventHandlers,
	...metadataHandlers,
	...settingsHandlers,
	...usersHandlers,
	...metricsHandlers,
	...tasksHandlers,
	...duplicatesHandlers,
	...utilityHandlers,
];

// Re-export individual handlers for selective use
export { authHandlers } from "./auth";
export { bookHandlers } from "./books";
export { duplicatesHandlers } from "./duplicates";
export { eventHandlers } from "./events";
export { libraryHandlers } from "./libraries";
export { metadataHandlers } from "./metadata";
export { metricsHandlers } from "./metrics";
export { seriesHandlers } from "./series";
export { settingsHandlers } from "./settings";
export { tasksHandlers } from "./tasks";
export { usersHandlers } from "./users";
