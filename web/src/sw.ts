/// <reference lib="webworker" />

/**
 * Codex service worker (`injectManifest` mode).
 *
 * Mirrors the runtime caching the prior `generateSW` config provided,
 * plus a per-book CacheFirst route that serves downloaded books from a
 * dedicated cache. The downloaded-id set is hydrated from IndexedDB at boot
 * and kept in sync via a BroadcastChannel published by the page-side
 * download manager.
 *
 * Update flow stays manual: `clientsClaim()` is not called and the SW only
 * skips waiting in response to the SKIP_WAITING message that
 * `PwaUpdatePrompt` sends when the user confirms.
 */

import { CacheableResponsePlugin } from "workbox-cacheable-response";
import { ExpirationPlugin } from "workbox-expiration";
import {
  cleanupOutdatedCaches,
  createHandlerBoundToURL,
  precacheAndRoute,
} from "workbox-precaching";
import { NavigationRoute, registerRoute } from "workbox-routing";
import { CacheFirst, NetworkFirst } from "workbox-strategies";

import {
  DOWNLOADS_BROADCAST_CHANNEL,
  type DownloadsBroadcast,
  getAllDownloads,
} from "./lib/offline/db";
import {
  cacheNameForBook,
  matchDownloadedBookRequest,
  NAVIGATION_DENYLIST,
} from "./lib/offline/routeMatcher";

declare const self: ServiceWorkerGlobalScope & {
  __WB_MANIFEST: Array<{ url: string; revision: string | null }>;
};

// 1) Precache the app shell using the manifest injected at build time.
precacheAndRoute(self.__WB_MANIFEST);
cleanupOutdatedCaches();

// 2) SPA navigation fallback. Serve /index.html for client-side routes so
//    deep links (e.g. /library/123/series/abc) resolve under standalone
//    display mode. Backend paths (NAVIGATION_DENYLIST) are excluded so they
//    always hit network.
registerRoute(
  new NavigationRoute(createHandlerBoundToURL("/index.html"), {
    denylist: NAVIGATION_DENYLIST,
  }),
);

// 3) Downloaded books: per-book CacheFirst. Registered before the generic
//    /api/* NetworkFirst route below so a downloaded book is served from
//    its dedicated cache rather than the shared NetworkFirst flow.
const downloadedBookIds = new Set<string>();

void (async () => {
  try {
    const downloads = await getAllDownloads();
    for (const record of downloads) {
      if (record.status === "complete") {
        downloadedBookIds.add(record.id);
      }
    }
  } catch (err) {
    // Database may not exist yet on the very first SW boot (before the page
    // has written anything). Treat that as an empty set and move on.
    console.warn("[sw] failed to hydrate downloaded book set", err);
  }
})();

if (typeof BroadcastChannel !== "undefined") {
  const channel = new BroadcastChannel(DOWNLOADS_BROADCAST_CHANNEL);
  channel.addEventListener(
    "message",
    (ev: MessageEvent<DownloadsBroadcast>) => {
      const payload = ev.data;
      if (payload.kind === "put") {
        if (payload.record.status === "complete") {
          downloadedBookIds.add(payload.record.id);
        } else {
          downloadedBookIds.delete(payload.record.id);
        }
      } else if (payload.kind === "delete") {
        downloadedBookIds.delete(payload.id);
      } else if (payload.kind === "clear") {
        downloadedBookIds.clear();
      }
    },
  );
}

registerRoute(
  ({ url, request }) =>
    matchDownloadedBookRequest(url, request.method, downloadedBookIds) !== null,
  async ({ url, request, event }) => {
    const match = matchDownloadedBookRequest(
      url,
      request.method,
      downloadedBookIds,
    );
    if (!match) {
      // Race: the book was evicted between the matcher and the handler.
      // Falling through to network keeps the response correct, just slow.
      return fetch(request);
    }
    const handler = new CacheFirst({
      cacheName: cacheNameForBook(match.bookId),
      plugins: [new CacheableResponsePlugin({ statuses: [0, 200] })],
    });
    return handler.handle({ request, event });
  },
);

// 4) Generic /api/* — NetworkFirst with a short cache TTL so a recent
//    library listing stays visible offline without serving stale auth state.
registerRoute(
  ({ url }) => url.pathname.startsWith("/api/"),
  new NetworkFirst({
    cacheName: "codex-api",
    networkTimeoutSeconds: 5,
    plugins: [
      new CacheableResponsePlugin({ statuses: [0, 200] }),
      new ExpirationPlugin({ maxEntries: 64, maxAgeSeconds: 60 * 5 }),
    ],
  }),
);

// 5) Fonts and images — CacheFirst, long TTL (rarely change).
registerRoute(
  ({ request }) =>
    request.destination === "font" || request.destination === "image",
  new CacheFirst({
    cacheName: "codex-assets",
    plugins: [
      new CacheableResponsePlugin({ statuses: [0, 200] }),
      new ExpirationPlugin({
        maxEntries: 128,
        maxAgeSeconds: 60 * 60 * 24 * 30,
      }),
    ],
  }),
);

// 6) Update flow. The page calls `updateServiceWorker(true)` from
//    `PwaUpdatePrompt` which posts this message; we then activate the
//    waiting SW immediately so the next navigation hits the fresh assets.
self.addEventListener("message", (event) => {
  const data = event.data as { type?: string } | undefined;
  if (data?.type === "SKIP_WAITING") {
    self.skipWaiting();
  }
});
