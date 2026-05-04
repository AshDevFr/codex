import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

/** Default poll interval: 24 hours. Daily polls keep the per-uploader fan-out
 * gentle and respect Nyaa's preference for low-frequency clients. */
export const DEFAULT_POLL_INTERVAL_S = 86_400;

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
      defaultPollIntervalS: DEFAULT_POLL_INTERVAL_S,
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
          "Comma-separated list of trusted uploader handles or queries. Each entry is either `username` (a Nyaa user feed) or `q:<query>` (a fallback site-wide search filter, useful for groups without a dedicated account). Confidence stays above the rejection threshold only for entries that match a tracked series alias.",
        type: "string" as const,
        required: false,
        default: "",
        example: "1r0n,TankobonBlur,q:LuminousScans",
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
    "1. Configure the Uploader Subscriptions field with a comma-separated list of trusted uploader handles (e.g. `1r0n,TankobonBlur`). Use `q:<query>` for groups without a Nyaa account. 2. Make sure tracked series have aliases that match how the uploader names releases (e.g. include alternate spellings, romanizations, the volume-ranges tag uploaders use). 3. The plugin polls the uploader feeds at the configured interval; any release whose title matches a tracked alias is recorded as a candidate. Filtering by formats / `(Digital)` tag happens at parse time and is logged but doesn't reject candidates by default.",
} as const satisfies PluginManifest & {
  capabilities: { releaseSource: { kinds: ["rss-uploader"] } };
};
