import type { MetadataContentType, PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

// Default config values
export const DEFAULT_MAX_RESULTS = 10;

export const manifest = {
  name: "metadata-openlibrary",
  displayName: "Open Library",
  version: packageJson.version,
  description:
    "Fetches book metadata from Open Library (openlibrary.org). Supports ISBN lookup and title search for EPUBs, PDFs, and other book formats.",
  author: "Codex",
  homepage: "https://openlibrary.org",
  protocolVersion: "1.0",
  capabilities: {
    // Book metadata provider only (not series)
    metadataProvider: ["book"] as MetadataContentType[],
  },
  configSchema: {
    description: "Configuration options for the Open Library plugin",
    fields: [
      {
        key: "maxResults",
        label: "Maximum Results",
        description: "Maximum number of results to return for search queries (1-50)",
        type: "number" as const,
        required: false,
        default: DEFAULT_MAX_RESULTS,
        example: 20,
      },
    ],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};
