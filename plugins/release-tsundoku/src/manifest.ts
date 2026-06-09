import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

/**
 * Maps a Codex external-ID source name to the provider name the Tsundoku feed
 * uses. Codex stores some sources under different names than Tsundoku emits
 * (e.g. Codex `myanimelist` ↔ Tsundoku `mal`), so we translate when building
 * the match index and the feed filter. Identity for names that already agree.
 *
 * The keys are the *bare* Codex source names — the host strips the stored
 * `api:` / `plugin:` prefix before matching `requiresExternalIds`, so a series
 * stored as `api:myanimelist` is delivered to us as `myanimelist`.
 */
export const CODEX_TO_TSUNDOKU_PROVIDER: Record<string, string> = {
  mangabaka: "mangabaka",
  anilist: "anilist",
  myanimelist: "mal",
  mangaupdates: "mangaupdates",
  kitsu: "kitsu",
  shikimori: "shikimori",
  animeplanet: "anime_planet",
  animenewsnetwork: "anime_news_network",
};

/**
 * The Codex source names the plugin asks the host for via
 * `requiresExternalIds`. These must be the names Codex *stores* (the map keys),
 * not Tsundoku's — the host filters `series_external_ids.source` against them.
 */
export const CODEX_EXTERNAL_ID_SOURCES = Object.keys(CODEX_TO_TSUNDOKU_PROVIDER);

export const manifest = {
  name: "release-tsundoku",
  displayName: "Tsundoku Releases",
  version: packageJson.version,
  description:
    "Announces new volume/chapter coverage for tracked series via a Tsundoku instance's incremental series feed. Matches series by exact external IDs (no fuzzy matching) and walks the feed by cursor, persisting its position between polls.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.1",
  capabilities: {
    releaseSource: {
      kinds: ["api-feed"],
      requiresAliases: false,
      requiresExternalIds: [...CODEX_EXTERNAL_ID_SOURCES],
      canAnnounceChapters: true,
      canAnnounceVolumes: true,
    },
  },
  configSchema: {
    description:
      "Tsundoku plugin configuration. Point `baseUrl` at your Tsundoku instance; the plugin polls its public `/api/v1/series/feed` endpoint and matches results to your tracked series by external ID.",
    fields: [
      {
        key: "baseUrl",
        label: "Tsundoku Base URL",
        description:
          "Base URL of the Tsundoku instance, e.g. `https://tsundoku.example.com`. The plugin appends `/api/v1/series/feed`. No trailing slash required.",
        type: "string" as const,
        required: true,
        example: "https://tsundoku.example.com",
      },
      {
        key: "defaultLanguage",
        label: "Default Language",
        description:
          "ISO 639-1 language tag stamped on every announcement. The Tsundoku feed tracks official release coverage and carries no language of its own, so a default is required. Per-series language preferences on each series' tracking config still gate the high-water mark host-side.",
        type: "string" as const,
        required: false,
        default: "en",
        example: "en",
      },
      {
        key: "pageLimit",
        label: "Feed Page Size",
        description:
          "Items requested per feed page (1–500). Larger pages mean fewer round-trips when walking a long backlog. Defaults to 100.",
        type: "number" as const,
        required: false,
        default: 100,
      },
      {
        key: "requestTimeoutMs",
        label: "Request Timeout (ms)",
        description:
          "How long to wait for a single feed page before giving up. Defaults to 10000 (10 seconds).",
        type: "number" as const,
        required: false,
        default: 10_000,
      },
    ],
  },
  userDescription:
    "Announces new volumes and chapters for series you've tracked, using a Tsundoku instance as the source. Matches your series by external ID (MangaBaka, AniList, MAL, and more). Notification-only — Codex does not download anything.",
  adminSetupInstructions:
    "1. Set `baseUrl` to your Tsundoku instance URL (e.g. `https://tsundoku.example.com`) and save. The plugin auto-registers a single source row (`Tsundoku Releases`) in **Settings → Release tracking**, where you can disable it, change the poll interval, or hit *Poll now*. 2. To get announcements for a series, make sure it has at least one external ID Tsundoku also knows (MangaBaka, AniList, MAL, MangaUpdates, Kitsu, Shikimori, Anime-Planet, or Anime News Network) — populate these via a metadata refresh or by pasting them in the series tracking panel. 3. Optional: adjust `defaultLanguage` (default `en`), `pageLimit`, and `requestTimeoutMs`. The Tsundoku feed endpoint is public; no credentials are needed. Note: the feed is incremental, so newly tracked series only announce on their *next* Tsundoku coverage change.",
} as const satisfies PluginManifest & {
  capabilities: { releaseSource: { kinds: ["api-feed"] } };
};
