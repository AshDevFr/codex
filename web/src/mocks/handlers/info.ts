/**
 * MSW handlers for app info API endpoint
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type AppInfoDto = components["schemas"]["AppInfoDto"];

export const infoHandlers = [
  // App info endpoint (public, no authentication required)
  http.get("/api/v1/info", async () => {
    await delay(50);
    const info: AppInfoDto = {
      version: "1.0.0-mock",
      name: "codex",
    };
    return HttpResponse.json(info);
  }),
];
