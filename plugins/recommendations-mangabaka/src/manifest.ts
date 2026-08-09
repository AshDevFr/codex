import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

/**
 * Canonical external ID source for MangaBaka (`api:<service>` convention).
 *
 * This value is load-bearing beyond identification: the host reads
 * `capabilities.externalIdSource` to build the `excludeIds` list it sends with
 * every `recommendations/get` call, collecting the external IDs of series the
 * user has already read. It must match the source string `metadata-mangabaka`
 * stamps onto series, or exclusion silently degrades to a no-op and users get
 * recommended things they have already finished.
 */
export const EXTERNAL_ID_SOURCE_MANGABAKA = "api:mangabaka" as const;

/** Public series page on MangaBaka, used for `externalUrl`. */
export function seriesUrl(id: number | string): string {
  return `https://mangabaka.org/${id}`;
}

export const manifest = {
  name: "recommendations-mangabaka",
  displayName: "MangaBaka Recommendations",
  version: packageJson.version,
  description:
    "Manga recommendations from MangaBaka, combining tag-vector similarity across your library with what other readers of the same series enjoy. No account required.",
  author: "Codex",
  homepage: "https://mangabaka.org",
  protocolVersion: "1.1",
  capabilities: {
    userRecommendationProvider: true,
    externalIdSource: EXTERNAL_ID_SOURCE_MANGABAKA,
  },
  // No `requiredCredentials` and no `oauth` block: every endpoint this plugin
  // uses is public. That is the point of it, and the main practical difference
  // from the AniList provider.
  configSchema: {
    description: "Optional configuration for the MangaBaka recommendations plugin",
    fields: [
      {
        key: "timeout",
        label: "Request Timeout",
        description: "HTTP request timeout in seconds for calls to MangaBaka.",
        type: "number" as const,
        required: false,
        default: 30,
        example: 45,
      },
      {
        key: "base_url",
        label: "API Base URL",
        description:
          "Override the MangaBaka API base URL. Rarely needed, but the recommendation endpoints are beta, so this is the escape hatch if they move hosts or you front them with a proxy.",
        type: "string" as const,
        required: false,
        default: "https://api.mangabaka.org",
        example: "https://api.mangabaka.org",
      },
    ],
  },
  // Multi-value settings are comma-separated strings rather than arrays: the
  // user-plugin settings UI renders booleans, numbers, and JSON specially and
  // falls back to a plain text input for everything else, so an array type
  // would show up as a text box with no help. This matches the convention the
  // AniList provider already uses.
  userConfigSchema: {
    description: "Per-user recommendation settings",
    fields: [
      {
        key: "contentRating",
        label: "Content Ratings",
        description:
          "Comma-separated ratings to allow. Valid values: safe, suggestive, erotica, pornographic. Leave empty to allow everything.",
        type: "string" as const,
        required: false,
        default: "",
        example: "safe,suggestive",
      },
      {
        key: "includedTypes",
        label: "Included Types",
        description:
          'Comma-separated series types to allow (e.g. "manga,manhwa"). Valid values: manga, manhwa, manhua, novel, oel. Leave empty for no restriction.',
        type: "string" as const,
        required: false,
        default: "",
        example: "manga,manhwa",
      },
      {
        key: "excludedTypes",
        label: "Excluded Types",
        description:
          'Comma-separated series types to exclude (e.g. "novel" to keep light novels out of your results).',
        type: "string" as const,
        required: false,
        default: "",
        example: "novel",
      },
      {
        key: "excludedGenres",
        label: "Excluded Genres",
        description: 'Comma-separated genres to exclude (e.g. "hentai,ecchi").',
        type: "string" as const,
        required: false,
        default: "",
        example: "hentai",
      },
      {
        key: "excludedTags",
        label: "Excluded Tags",
        description:
          'Comma-separated tag names to exclude (e.g. "Death Game,Gore"). Names are matched against MangaBaka\'s tag list, ignoring case. Unrecognised names are skipped.',
        type: "string" as const,
        required: false,
        default: "",
        example: "Gore",
      },
      {
        key: "minimumRating",
        label: "Minimum Rating",
        description:
          "Only recommend series rated at least this highly on MangaBaka (0-100). Set to 0 for no minimum.",
        type: "number" as const,
        required: false,
        default: 0,
        example: 70,
      },
      {
        key: "contentWeight",
        label: "Similarity vs. Reader Overlap",
        description:
          "Balance between the two signals, from 0 to 1. Higher favours series with similar tags to your library; lower favours series that readers of your favourites also read. 0.5 treats them equally.",
        type: "number" as const,
        required: false,
        default: 0.5,
        example: 0.5,
      },
      {
        key: "collaborativeSeeds",
        label: "Reader Overlap Seeds",
        description:
          "How many of your top-rated series to look up reader overlap for (0-10). Set to 0 to turn the reader-overlap signal off entirely. Higher values mean more variety but more requests.",
        type: "number" as const,
        required: false,
        default: 5,
        example: 5,
      },
      {
        key: "excludeSameAuthor",
        label: "Exclude Same Author",
        description:
          "Hide series by an author you already read. They are ranked lower by default rather than hidden, since an author's other work is often a good recommendation.",
        type: "boolean" as const,
        required: false,
        default: false,
      },
    ],
  },
  userDescription:
    "Manga recommendations powered by MangaBaka, with no account or API key needed. Covers manga, manhwa, and manhua only; it will not produce results for western comics or ebooks.",
  userSetupInstructions:
    "Just enable it, there is nothing to connect. Recommendations are seeded from series in your library that MangaBaka can identify, so run a metadata match with the MangaBaka Metadata plugin first to get the best results.",
} as const satisfies PluginManifest & {
  capabilities: { userRecommendationProvider: true };
};
