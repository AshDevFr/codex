import type { MetadataContentType, PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

// Default config values
export const DEFAULT_MAX_RESULTS = 5;

export const manifest = {
  name: "metadata-echo",
  displayName: "Echo Metadata Plugin",
  version: packageJson.version,
  description:
    "Test metadata plugin that echoes back search queries (supports both series and book metadata)",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.0",
  capabilities: {
    // Demonstrates multi-content-type plugin support
    metadataProvider: ["series", "book"] as MetadataContentType[],
  },
  configSchema: {
    description: "Configuration options for the Echo test plugin",
    fields: [
      {
        key: "maxResults",
        label: "Maximum Results",
        description: "Maximum number of results to return for search queries (1-20)",
        type: "number" as const,
        required: false,
        default: DEFAULT_MAX_RESULTS,
        example: 10,
      },
    ],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};
