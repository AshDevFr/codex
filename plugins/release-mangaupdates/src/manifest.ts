import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

/**
 * External-ID source name for MangaUpdates.
 *
 * MangaUpdates IDs are populated by metadata-provider plugins (e.g.
 * MangaBaka cross-references) or pasted manually by the user via the series
 * tracking panel. The release plugin needs the bare source name (no
 * `plugin:` prefix) here to match the host's external-ID filter.
 */
export const EXTERNAL_ID_SOURCE_MANGAUPDATES = "mangaupdates" as const;

/** Default poll interval: 24 hours. Daily polls match upstream cadence and
 * keep the per-series fan-out gentle for users tracking hundreds of series. */
export const DEFAULT_POLL_INTERVAL_S = 86_400;

export const manifest = {
  name: "release-mangaupdates",
  displayName: "MangaUpdates Releases",
  version: packageJson.version,
  description:
    "Announces new chapter releases for tracked series via MangaUpdates per-series RSS feeds. Filters by user-configured languages.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.1",
  capabilities: {
    releaseSource: {
      kinds: ["rss-series"],
      requiresAliases: false,
      requiresExternalIds: [EXTERNAL_ID_SOURCE_MANGAUPDATES],
      canAnnounceChapters: true,
      canAnnounceVolumes: true,
      defaultPollIntervalS: DEFAULT_POLL_INTERVAL_S,
    },
  },
  configSchema: {
    description:
      "MangaUpdates plugin configuration. Per-series language preferences live on each series' tracking config; the values here are server-wide defaults applied when a series doesn't override them.",
    fields: [
      {
        key: "blockedGroups",
        label: "Blocked Scanlation Groups",
        description:
          "Comma-separated list of scanlation group names to exclude from announcements (case-insensitive, exact match). Per-series overrides may further extend this list.",
        type: "string" as const,
        required: false,
        default: "",
        example: "LowQualityScans,MTL Group",
      },
      {
        key: "requestTimeoutMs",
        label: "Request Timeout (ms)",
        description:
          "How long to wait for a single RSS fetch before giving up. Defaults to 10000 (10 seconds).",
        type: "number" as const,
        required: false,
        default: 10_000,
      },
    ],
  },
  userDescription:
    "Announces new chapters for series you've tracked, using their MangaUpdates IDs. Filters releases to languages you can read. Notification-only — Codex does not download anything.",
  adminSetupInstructions:
    "1. No config is required to get started — saving the plugin is enough. The plugin auto-registers a single source row (`MangaUpdates Releases`) in **Settings → Release tracking** on first start, where you can disable it, change the poll interval, or hit *Poll now*. 2. To get announcements for a series, edit its tracking panel and either paste a `mangaupdates` external ID or let the metadata-refresh path populate it from MangaBaka cross-references. 3. Optional: set `blockedGroups` (CSV, case-insensitive) to filter noisy scanlators server-wide; per-series language preferences live on each series' tracking config and override the server default (`release_tracking.default_languages`). No credentials are needed; MangaUpdates RSS feeds are public.",
} as const satisfies PluginManifest & {
  capabilities: { releaseSource: { kinds: ["rss-series"] } };
};
