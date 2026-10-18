/**
 * MSW handlers for the admin news endpoints
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type NewsResponse = components["schemas"]["NewsResponse"];

let lastSeenAt = 0;

const entries: NewsResponse["entries"] = [
  {
    id: "codex-ios-1.1.0",
    project: "codex-ios",
    kind: "release",
    title: "Codex Reader (iOS) 1.1.0",
    version: "1.1.0",
    summary:
      "Collections and read lists: the Actions menu on a series or book page now ticks the collections and read lists it is already in.",
    body: "New\n\n- Collections and read lists: the Actions menu on a series or book page now ticks the collections and read lists it is already in. Tap one to add or remove it.",
    url: "https://apps.apple.com/us/app/codex-reader-manga-comics/id6801989441",
    publishedAt: "2026-10-16T16:16:49Z",
    unread: true,
  },
  {
    id: "codex-v2.6.3",
    project: "codex",
    kind: "release",
    title: "Codex 2.6.3",
    version: "2.6.3",
    summary:
      "Keep agent worktrees out of the Docker build context; Stop a session that moved nothing from rewriting reading progress",
    body: "### Bug Fixes\n\n- *(build)* Keep agent worktrees out of the Docker build context\n- *(db)* Stop a session that moved nothing from rewriting reading progress",
    url: "https://github.com/AshDevFr/codex/releases/tag/v2.6.3",
    publishedAt: "2026-10-02T23:24:00Z",
    unread: false,
  },
];

export const newsHandlers = [
  http.get("/api/v1/news", async () => {
    await delay(50);
    const withState = entries.map((entry) => ({
      ...entry,
      unread: entry.unread && Date.parse(entry.publishedAt) > lastSeenAt,
    }));
    const response: NewsResponse = {
      enabled: true,
      lastFetchedAt: new Date().toISOString(),
      unreadCount: withState.filter((e) => e.unread).length,
      updateAvailable: {
        currentVersion: "1.0.0-mock",
        latestVersion: "2.6.3",
        url: "https://github.com/AshDevFr/codex/releases/tag/v2.6.3",
        releases: withState.filter((e) => e.project === "codex"),
      },
      entries: withState,
    };
    return HttpResponse.json(response);
  }),

  http.post("/api/v1/news/seen", async () => {
    await delay(50);
    lastSeenAt = Date.now();
    return new HttpResponse(null, { status: 204 });
  }),
];
