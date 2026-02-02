/**
 * MSW handlers for app info API endpoint
 */

import { delay, HttpResponse, http } from "msw";

export const infoHandlers = [
  // App info endpoint (public, no authentication required)
  http.get("/api/v1/info", async () => {
    await delay(50);
    return HttpResponse.json({
      version: "1.0.0-mock",
      name: "codex",
    });
  }),
];
