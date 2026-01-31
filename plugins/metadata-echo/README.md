# @ashdev/codex-plugin-metadata-echo

A minimal test metadata plugin for the Codex plugin system. Echoes back search queries and provides predictable responses for testing and development.

## Purpose

This plugin serves two purposes:

1. **Protocol Validation**: Demonstrates correct implementation of the Codex plugin protocol
2. **Development Testing**: Provides predictable responses for testing the plugin UI without external API dependencies

## Installation

```bash
npm install -g @ashdev/codex-plugin-metadata-echo
```

Or run directly with npx (no installation required).

## Adding the Plugin to Codex

### Using npx (Recommended)

1. Log in to Codex as an administrator
2. Navigate to **Settings** > **Plugins**
3. Click **Add Plugin**
4. Fill in the form:
   - **Name**: `metadata-echo`
   - **Display Name**: `Echo Metadata Plugin`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-metadata-echo@1.0.0`
   - **Scopes**: Select `series:detail`
5. Click **Save**
6. Click **Test Connection** to verify the plugin works
7. Toggle **Enabled** to activate the plugin

### npx Options

| Configuration | Arguments | Description |
|--------------|-----------|-------------|
| Latest version | `-y @ashdev/codex-plugin-metadata-echo` | Always uses latest |
| Pinned version | `-y @ashdev/codex-plugin-metadata-echo@1.0.0` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-metadata-echo@1.0.0` | Skips version check if cached |

**Flags:**
- `-y`: Auto-confirms installation (required for containers)
- `--prefer-offline`: Uses cached version without checking npm registry

### Using Docker

For Docker deployments, use npx with `--prefer-offline` for faster startup:

```
Command: npx
Arguments: -y --prefer-offline @ashdev/codex-plugin-metadata-echo@1.0.0
```

### Manual Installation (Alternative)

For maximum performance, install globally:

```bash
npm install -g @ashdev/codex-plugin-metadata-echo
```

Then configure:
- **Command**: `codex-plugin-metadata-echo`
- **Arguments**: (leave empty)

## Using the Plugin

Once enabled, the Echo plugin appears in the series detail page:

1. Navigate to any series in your library
2. Click the **Metadata** button (or look for the plugin icon)
3. Click **Search Echo Metadata Plugin**
4. Enter any search query
5. The plugin will echo your query back as search results
6. Select a result to see the preview
7. Click **Apply** to test the metadata apply flow

This is useful for:
- Testing the plugin UI without needing real API credentials
- Verifying the metadata preview and apply workflow
- Debugging plugin integration issues

## Response Behavior

### Search (`metadata/search`)

Returns two results for any query:

1. **Primary result**: Title is `"Echo: {query}"` with `relevanceScore: 1.0`
2. **Secondary result**: Title is `"Echo Result 2 for: {query}"` with `relevanceScore: 0.8`

### Get (`metadata/get`)

Returns metadata with the external ID embedded in the title and URL:

- Title: `"Echo Series: {externalId}"`
- External URL: `https://echo.example.com/series/{externalId}`
- Includes sample genres, tags, authors, and rating

### Match (`metadata/match`)

Returns a match based on the normalized title:

- External ID: `match-{normalized-title}`
- Confidence: `0.85`
- Includes one alternative match

## As a Reference Implementation

Use this plugin as a template for building your own metadata plugins:

```typescript
import {
  createMetadataPlugin,
  type MetadataProvider,
} from "@ashdev/codex-plugin-sdk";

const provider: MetadataProvider = {
  async search(params) {
    return {
      results: [
        {
          externalId: "123",
          title: "Example",
          alternateTitles: [],
          relevanceScore: 0.95,
          preview: {
            status: "ongoing",
            genres: ["Action"],
          },
        },
      ],
    };
  },

  async get(params) {
    return {
      externalId: params.externalId,
      externalUrl: `https://example.com/${params.externalId}`,
      alternateTitles: [],
      genres: [],
      tags: [],
      authors: [],
      artists: [],
      externalLinks: [],
    };
  },

  // Optional: implement match for auto-matching
  async match(params) {
    return {
      match: null,
      confidence: 0,
    };
  },
};

createMetadataPlugin({
  manifest: {
    name: "metadata-my-plugin",
    displayName: "My Metadata Plugin",
    version: "1.0.0",
    description: "My custom metadata plugin",
    author: "Me",
    protocolVersion: "1.0",
    capabilities: { metadataProvider: true },
    contentTypes: ["series"],
    scopes: ["series:detail"],
  },
  provider,
});
```

## Development

```bash
# Install dependencies
npm install

# Build the plugin
npm run build

# Type check
npm run typecheck

# Run tests
npm test

# Lint
npm run lint
```

## Project Structure

```
plugins/metadata-echo/
├── src/
│   └── index.ts      # Plugin implementation
├── dist/
│   └── index.js      # Built bundle (excluded from git)
├── package.json
├── tsconfig.json
└── README.md
```

## License

MIT
