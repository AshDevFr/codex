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
  oauth: {
    authorizationUrl: "https://anilist.co/api/v2/oauth/authorize",
    tokenUrl: "https://anilist.co/api/v2/oauth/token",
    scopes: [],
    pkce: false,
  },
  userDescription: "Personalized manga recommendations powered by AniList community data",
  adminSetupInstructions:
    "To enable OAuth login, create an AniList API client at https://anilist.co/settings/developer. Set the redirect URL to {your-codex-url}/api/v1/user/plugins/oauth/callback. Enter the Client ID below. Without OAuth configured, users can still connect by pasting a personal access token.",
  userSetupInstructions:
    "Connect your AniList account via OAuth, or paste a personal access token. To generate a token, visit https://anilist.co/settings/developer, create a client with redirect URL https://anilist.co/api/v2/oauth/pin, then authorize it to receive your token.",
} as const satisfies PluginManifest & {
  capabilities: { userRecommendationProvider: true };
};
