import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

/** Canonical external ID source for AniList (`api:<service>` convention) */
export const EXTERNAL_ID_SOURCE_ANILIST = "api:anilist" as const;

export const manifest = {
  name: "recommendations-anilist",
  displayName: "AniList Recommendations",
  version: packageJson.version,
  description:
    "Personalized manga recommendations from AniList based on your reading history and ratings.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.1",
  capabilities: {
    userRecommendationProvider: true,
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
  configSchema: {
    description: "Recommendation configuration",
    fields: [],
  },
  userConfigSchema: {
    description: "Per-user recommendation settings",
    fields: [
      {
        key: "searchFallback",
        label: "Search Fallback",
        description:
          "When a series has no AniList ID, search by title to find a match. Disable for strict matching only.",
        type: "boolean" as const,
        required: false,
        default: true,
      },
      {
        key: "allowedCountries",
        label: "Country of Origin Filter",
        description:
          'Comma-separated ISO country codes to include (e.g. "JP" for manga, "KR" for manhwa, "CN" for manhua). Leave empty for no filter.',
        type: "string" as const,
        required: false,
        default: "",
        example: "JP,KR",
      },
      {
        key: "excludedGenres",
        label: "Excluded Genres",
        description:
          'Comma-separated genres to exclude from recommendations (e.g. "Hentai,Ecchi").',
        type: "string" as const,
        required: false,
        default: "",
        example: "Hentai",
      },
      {
        key: "excludedFormats",
        label: "Excluded Formats",
        description:
          'Comma-separated formats to exclude (e.g. "NOVEL,ONE_SHOT"). Valid values: MANGA, NOVEL, ONE_SHOT.',
        type: "string" as const,
        required: false,
        default: "",
        example: "NOVEL",
      },
      {
        key: "minAniListScore",
        label: "Minimum AniList Score",
        description:
          "Minimum average score (0-100) on AniList to include a recommendation. Set to 0 to disable.",
        type: "number" as const,
        required: false,
        default: 0,
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
