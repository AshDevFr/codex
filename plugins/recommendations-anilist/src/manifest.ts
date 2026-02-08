import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

export const manifest = {
  name: "recommendations-anilist",
  displayName: "AniList Recommendations",
  version: packageJson.version,
  description:
    "Personalized manga recommendations from AniList based on your reading history and ratings.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.0",
  capabilities: {
    userRecommendationProvider: true,
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
    description: "Recommendation configuration",
    fields: [
      {
        key: "maxRecommendations",
        label: "Maximum Recommendations",
        description: "Maximum number of recommendations to generate (1-50)",
        type: "number" as const,
        required: false,
        default: 20,
      },
    ],
  },
} as const satisfies PluginManifest & {
  capabilities: { userRecommendationProvider: true };
};
