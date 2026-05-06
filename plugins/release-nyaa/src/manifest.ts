import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

/** Default per-fetch HTTP timeout. Nyaa is usually fast; 10s is generous. */
export const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;

/**
 * Default minimum confidence threshold for emitted candidates. Nyaa matches
 * series via title parsing + alias comparison, which is fuzzier than the
 * external-ID match used by MangaUpdates. The host's threshold (default 0.7)
 * still filters at record time; this is the plugin-side floor below which we
 * don't even bother calling `releases/record`.
 */
export const DEFAULT_MIN_CONFIDENCE = 0.7;

export const manifest = {
  name: "release-nyaa",
  displayName: "Nyaa Releases",
  version: packageJson.version,
  description:
    "Announces new chapter / volume torrents for tracked series via Nyaa.si uploader RSS feeds. Limited to an admin-configured uploader allowlist; matches via title aliases.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.1",
  capabilities: {
    releaseSource: {
      kinds: ["rss-uploader"],
      requiresAliases: true,
      canAnnounceChapters: true,
      canAnnounceVolumes: true,
    },
  },
  configSchema: {
    description:
      "Nyaa plugin configuration. The plugin polls the listed uploaders' RSS feeds (or, for groups without a Nyaa account, a fallback search query) and emits release candidates only for tracked series whose aliases match the parsed title. Notification-only: Codex never downloads torrents.",
    fields: [
      {
        key: "uploaders",
        label: "Uploader Subscriptions",
        description:
          "Comma-separated list of trusted uploader handles or queries. Each entry is one of: `username` (a Nyaa user feed); `q:<query>` (a plain site-wide search); or `q:?<params>` (URL-style allowlisted params: `q`, `c`, `f`, `u` — e.g. `q:?c=3_1&q=Berserk` to search the Literature → English-translated category). Confidence stays above the rejection threshold only for entries that match a tracked series alias.",
        type: "string" as const,
        required: false,
        default: "",
        example: "1r0n,TankobonBlur,q:LuminousScans,q:?c=3_1&q=Berserk",
      },
      {
        key: "requestTimeoutMs",
        label: "Request Timeout (ms)",
        description:
          "How long to wait for a single Nyaa RSS fetch before giving up. Defaults to 10000 (10 seconds).",
        type: "number" as const,
        required: false,
        default: DEFAULT_REQUEST_TIMEOUT_MS,
      },
      {
        key: "baseUrl",
        label: "Nyaa Base URL",
        description:
          "Override the Nyaa base URL. Useful for mirrors or for tests. Defaults to https://nyaa.si.",
        type: "string" as const,
        required: false,
        default: "https://nyaa.si",
        example: "https://nyaa.si",
      },
    ],
  },
  userDescription:
    "Watches Nyaa.si uploader feeds for new releases of tracked series. Matches by title alias — make sure your series' aliases (auto-populated from metadata or added manually in the Tracking panel) cover the way the uploader names them. Notification-only — Codex never downloads anything.",
  adminSetupInstructions:
    "1. Set the **Uploaders** config field to a comma-separated list. Each entry is one of: `username` (a Nyaa user feed, e.g. `tsuna69`), `q:<query>` (a plain site-wide search, e.g. `q:LuminousScans`), or `q:?<params>` (URL-style search with allowlisted keys `q`, `c`, `f`, `u`, e.g. `q:?c=3_1&q=Berserk` for the English-translated Literature category). 2. Save. The plugin restarts and the host materializes one row per entry in **Settings → Release tracking** — that's where you flip rows on/off, override the poll interval, or hit *Poll now*. 3. Make sure tracked series have aliases that match how the uploader names releases (alternate spellings, romanizations, volume-range tags). The plugin auto-prunes rows when you remove an entry from the list and re-save, so the Release tracking table stays in sync with this CSV.",
} as const satisfies PluginManifest & {
  capabilities: { releaseSource: { kinds: ["rss-uploader"] } };
};
