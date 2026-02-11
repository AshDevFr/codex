import { EXTERNAL_ID_SOURCE_ANILIST, type PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

export const manifest = {
  name: "sync-anilist",
  displayName: "AniList Sync",
  version: packageJson.version,
  description:
    "Sync manga reading progress between Codex and AniList. Supports push/pull of reading status, chapters read, scores, and dates.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.0",
  capabilities: {
    userReadSync: true,
    externalIdSource: EXTERNAL_ID_SOURCE_ANILIST,
  },
  requiredCredentials: [
    {
      key: "access_token",
      label: "AniList Access Token",
      description: "OAuth access token for AniList API",
      type: "password" as const,
      required: true,
      sensitive: true,
    },
  ],
  userConfigSchema: {
    description: "AniList-specific sync settings",
    fields: [
      {
        key: "progressUnit",
        label: "Progress Unit",
        description:
          "What each book in Codex represents in AniList. Use 'volumes' for manga volumes, 'chapters' for individual chapters",
        type: "string" as const,
        required: false,
        default: "volumes",
      },
      {
        key: "pauseAfterDays",
        label: "Auto-Pause After Days",
        description:
          "Automatically set in-progress series to Paused on AniList if no reading activity in this many days. Set to 0 to disable.",
        type: "number" as const,
        required: false,
        default: 0,
      },
      {
        key: "dropAfterDays",
        label: "Auto-Drop After Days",
        description:
          "Automatically set in-progress series to Dropped on AniList if no reading activity in this many days. Set to 0 to disable. When both pause and drop are set, the shorter threshold fires first.",
        type: "number" as const,
        required: false,
        default: 0,
      },
      {
        key: "searchFallback",
        label: "Search Fallback",
        description:
          "When a series has no AniList ID, search by title to find a match and sync progress. Disable for strict matching only.",
        type: "boolean" as const,
        required: false,
        default: false,
      },
      {
        key: "private",
        label: "Private Mode",
        description:
          "When enabled, all manga list entries synced from Codex will be marked as private on AniList, visible only to you. When disabled, entries follow AniList's default visibility (public).",
        type: "boolean" as const,
        required: false,
        default: true,
      },
      {
        key: "hiddenFromStatusLists",
        label: "Hide from Status Lists",
        description:
          "When enabled, synced entries will be hidden from your standard AniList status lists (Currently Reading, Completed, etc.) but will still appear in custom lists. Has no effect when Private Mode is enabled.",
        type: "boolean" as const,
        required: false,
        default: false,
      },
    ],
  },
  oauth: {
    authorizationUrl: "https://anilist.co/api/v2/oauth/authorize",
    tokenUrl: "https://anilist.co/api/v2/oauth/token",
    scopes: [],
    pkce: false,
  },
  userDescription: "Sync manga reading progress between Codex and AniList",
  adminSetupInstructions:
    "To enable OAuth login, create an AniList API client at https://anilist.co/settings/developer. Set the redirect URL to {your-codex-url}/api/v1/user/plugins/oauth/callback. Enter the Client ID below. Without OAuth configured, users can still connect by pasting a personal access token.",
  userSetupInstructions:
    "Connect your AniList account via OAuth, or paste a personal access token. To generate a token, visit https://anilist.co/settings/developer, create a client with redirect URL https://anilist.co/api/v2/oauth/pin, then authorize it to receive your token.",
} as const satisfies PluginManifest & {
  capabilities: { userReadSync: true };
};
