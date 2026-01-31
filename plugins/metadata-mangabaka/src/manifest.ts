import type { MetadataContentType, PluginManifest } from "@ashdev/codex-plugin-sdk";

export const manifest = {
  name: "metadata-mangabaka",
  displayName: "MangaBaka Metadata",
  version: "1.0.0",
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
} as const satisfies PluginManifest & {
  capabilities: { metadataProvider: MetadataContentType[] };
};
