/**
 * MSW handlers index
 *
 * Aggregates all mock API handlers for the application.
 */

import { authHandlers } from "./auth";
import { libraryHandlers } from "./libraries";
import { seriesHandlers } from "./series";
import { bookHandlers } from "./books";
import { http, HttpResponse, delay } from "msw";

// Additional utility handlers
const utilityHandlers = [
  // Health check
  http.get("/api/v1/health", async () => {
    await delay(50);
    return HttpResponse.json({ status: "ok" });
  }),

  // Setup status (assume setup is complete)
  http.get("/api/v1/setup/status", async () => {
    await delay(50);
    return HttpResponse.json({
      isSetupComplete: true,
      hasAdmin: true,
      hasLibraries: true,
    });
  }),

  // Metrics
  http.get("/api/v1/metrics", async () => {
    await delay(100);
    return HttpResponse.json({
      totalLibraries: 4,
      totalSeries: 25,
      totalBooks: 100,
      totalUsers: 2,
      totalReadProgress: 15,
      diskUsage: {
        thumbnails: 52428800,
        total: 1073741824,
      },
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
  ...utilityHandlers,
];

// Re-export individual handlers for selective use
export { authHandlers } from "./auth";
export { libraryHandlers } from "./libraries";
export { seriesHandlers } from "./series";
export { bookHandlers } from "./books";
