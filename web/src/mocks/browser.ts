/**
 * MSW browser setup
 *
 * Configures the service worker for browser-based API mocking.
 * This enables frontend development without a running backend.
 *
 * Usage:
 * - Start with mocking: npm run dev:mock
 * - Or set VITE_MOCK_API=true in .env.local
 */

import { setupWorker } from "msw/browser";
import { handlers } from "./handlers";

// Create the worker instance
export const worker = setupWorker(...handlers);

// Export for conditional startup
export async function startMockServiceWorker() {
	if (import.meta.env.VITE_MOCK_API === "true") {
		console.log("[MSW] Mock API enabled");
		return worker.start({
			onUnhandledRequest: "bypass", // Don't warn for unhandled requests (static assets, etc.)
			serviceWorker: {
				url: "/mockServiceWorker.js",
			},
		});
	}
	return Promise.resolve();
}
