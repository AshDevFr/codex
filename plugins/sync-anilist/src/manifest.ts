import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
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
    externalIdSource: "api:anilist",
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
    description: "AniList sync settings",
    fields: [
      {
        key: "syncRatings",
        label: "Sync Ratings & Notes",
        description:
          "Include user ratings and notes in sync. When off, only reading progress is synced.",
        type: "boolean" as const,
        required: false,
        default: false,
      },
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
        key: "pushCompletedSeries",
        label: "Push Completed Series",
        description:
          "Push series where all local books are marked as read",
        type: "boolean" as const,
        required: false,
        default: true,
      },
      {
        key: "pushInProgressSeries",
        label: "Push In-Progress Series",
        description:
          "Push series where at least one book has been started",
        type: "boolean" as const,
        required: false,
        default: true,
      },
      {
        key: "pushInProgressVolumes",
        label: "Count In-Progress Volumes",
        description:
          "Include partially-read volumes/chapters in the progress count (otherwise only fully read ones are counted)",
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
  userDescription:
    "Sync manga reading progress between Codex and AniList",
  adminSetupInstructions:
    "To enable OAuth login, create an AniList API client at https://anilist.co/settings/developer. Set the redirect URL to {your-codex-url}/api/v1/user/plugins/oauth/callback. Enter the Client ID below. Without OAuth configured, users can still connect by pasting a personal access token.",
  userSetupInstructions:
    "Connect your AniList account via OAuth, or paste a personal access token. To generate a token, visit https://anilist.co/settings/developer, create a client with redirect URL https://anilist.co/api/v2/oauth/pin, then authorize it to receive your token.",
} as const satisfies PluginManifest & {
  capabilities: { userReadSync: true };
};
