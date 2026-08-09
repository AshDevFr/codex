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
    fields: [],
  },
  userDescription:
    "Manga recommendations powered by MangaBaka, with no account or API key needed. Covers manga, manhwa, and manhua only; it will not produce results for western comics or ebooks.",
  userSetupInstructions:
    "Just enable it, there is nothing to connect. Recommendations are seeded from series in your library that MangaBaka can identify, so run a metadata match with the MangaBaka Metadata plugin first to get the best results.",
} as const satisfies PluginManifest & {
  capabilities: { userRecommendationProvider: true };
};
