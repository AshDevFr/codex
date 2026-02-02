import type { MetadataContentType, PluginManifest } from "@ashdev/codex-plugin-sdk";
import packageJson from "../package.json" with { type: "json" };

export const manifest = {
  name: "metadata-mangabaka",
  displayName: "MangaBaka Metadata",
  version: packageJson.version,
  description: "Fetch manga metadata from MangaBaka - aggregated data from multiple sources",
  author: "Codex",
  homepage: "https://mangabaka.org",
  protocolVersion: "1.0",
  capabilities: {
    metadataProvider: ["series"] as MetadataContentType[],
  },
  requiredCredentials: [
    {
      key: "api_key",
      label: "API Key",
      description: "Get your API key at https://mangabaka.org/settings/api (requires account)",
      required: true,
      sensitive: true,
      type: "password",
      placeholder: "mb-...",
    },
  ],
  configSchema: {
    description: "Optional configuration for the MangaBaka plugin",
    fields: [
      {
        key: "timeout",
        label: "Request Timeout",
        description: "HTTP request timeout in seconds for API calls to MangaBaka",
        type: "number",
        required: false,
        default: 60,
        example: 30,
      },
    ],
  },
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};
