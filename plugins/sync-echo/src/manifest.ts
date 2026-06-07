import type { PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

// Default config values
export const DEFAULT_PULL_COUNT = 3;
export const DEFAULT_MAX_PAYLOAD_FILES = 500;

export const manifest = {
  name: "sync-echo",
  displayName: "Echo Sync Plugin",
  version: packageJson.version,
  description:
    "Test sync plugin that echoes back push payloads and returns deterministic pull entries. Records every request/response to files for debugging.",
  author: "Codex",
  homepage: "https://github.com/AshDevFr/codex",
  protocolVersion: "1.0",
  capabilities: {
    userReadSync: true,
    // Opt in to the per-book detailed progress payload so it can be inspected.
    wantsDetailedProgress: true,
  },
  configSchema: {
    description: "Configuration options for the Echo sync test plugin",
    fields: [
      {
        key: "pullCount",
        label: "Pull Entry Count",
        description: "How many deterministic entries pullProgress should return (1-50).",
        type: "number" as const,
        required: false,
        default: DEFAULT_PULL_COUNT,
        example: 5,
      },
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
    "A debug sync plugin: it accepts any push, returns canned reading entries on pull, and records all protocol traffic to files. No external account needed.",
} as const satisfies PluginManifest & {
  capabilities: { userReadSync: true };
};
