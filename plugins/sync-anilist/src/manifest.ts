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
    userSyncProvider: true,
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
  configSchema: {
    description: "AniList sync configuration",
    fields: [
      {
        key: "scoreFormat",
        label: "Score Format",
        description:
          "How scores are mapped. AniList supports POINT_100, POINT_10_DECIMAL, POINT_10, POINT_5, POINT_3",
        type: "string" as const,
        required: false,
        default: "POINT_10",
      },
    ],
  },
} as const satisfies PluginManifest & {
  capabilities: { userSyncProvider: true };
};
