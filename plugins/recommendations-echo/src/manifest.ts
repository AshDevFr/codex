import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

// Default config values
export const DEFAULT_MAX_PAYLOAD_FILES = 500;
// Number of generic recommendations returned when the library is empty.
export const DEFAULT_FALLBACK_COUNT = 3;

export const manifest = {
  name: "recommendations-echo",
  displayName: "Echo Recommendations Plugin",
  version: packageJson.version,
  description:
    "Test recommendations plugin that echoes library seeds back as recommendations. Records every request/response to files for debugging.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.1",
  capabilities: {
    userRecommendationProvider: true,
  },
  configSchema: {
    description: "Configuration options for the Echo recommendations test plugin",
    fields: [
      {
        key: "recordPayloads",
        label: "Record Payloads",
        description:
          "Write each request and its response to JSON files under the plugin's data directory for debugging.",
        type: "boolean" as const,
        required: false,
        default: true,
      },
      {
        key: "maxPayloadFiles",
        label: "Max Payload Files",
        description:
          "Maximum number of recorded payload files to keep; oldest are pruned. Only used when payload recording is enabled.",
        type: "number" as const,
        required: false,
        default: DEFAULT_MAX_PAYLOAD_FILES,
        example: 500,
      },
    ],
  },
  userDescription:
    "A debug recommendations plugin: it echoes your library seeds back as recommendations and records all protocol traffic to files. No external account needed.",
} as const satisfies PluginManifest & {
  capabilities: { userRecommendationProvider: true };
};
