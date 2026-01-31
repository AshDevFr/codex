# @ashdev/codex-plugin-metadata-mangabaka

A Codex metadata plugin for fetching manga metadata from [MangaBaka](https://mangabaka.org). MangaBaka aggregates metadata from multiple sources including AniList, MyAnimeList, MangaDex, and more.

## Features

- Search for manga/manhwa/manhua by title
- Fetch comprehensive metadata including:
  - Titles in multiple languages (English, Japanese, Korean, Chinese)
  - Synopsis/description
  - Publication status (ongoing, completed, hiatus, cancelled)
  - Genres and tags
  - Authors and artists
  - Cover images
  - Ratings
  - External links to AniList, MAL, MangaDex

## Prerequisites

You need a MangaBaka API key to use this plugin:

1. Create an account at [mangabaka.org](https://mangabaka.org)
2. Go to [Settings > API](https://mangabaka.org/settings/api)
3. Generate an API key

## Installation

```bash
npm install -g @ashdev/codex-plugin-metadata-mangabaka
```

Or run directly with npx (no installation required).

## Adding the Plugin to Codex

### Using npx (Recommended)

1. Log in to Codex as an administrator
2. Navigate to **Settings** > **Plugins**
3. Click **Add Plugin**
4. Fill in the form:
   - **Name**: `metadata-mangabaka`
   - **Display Name**: `MangaBaka Metadata`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0`
   - **Scopes**: Select `series:detail`
5. In the **Credentials** tab:
   - **Credential Delivery**: Select `Initialize Message` or `Both`
   - **Credentials**: `{"api_key": "your-mangabaka-api-key"}`
6. Click **Save**
7. Click **Test Connection** to verify the plugin works
8. Toggle **Enabled** to activate the plugin

### npx Options

| Configuration | Arguments | Description |
|--------------|-----------|-------------|
| Latest version | `-y @ashdev/codex-plugin-metadata-mangabaka` | Always uses latest |
| Pinned version | `-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Skips version check if cached |

**Flags:**
- `-y`: Auto-confirms installation (required for containers)
- `--prefer-offline`: Uses cached version without checking npm registry

### Using Docker

For Docker deployments, use npx with `--prefer-offline` for faster startup:

```
Command: npx
Arguments: -y --prefer-offline @ashdev/codex-plugin-metadata-mangabaka@1.0.0
```

Pre-warm the cache in your Dockerfile:

```dockerfile
# Pre-cache plugin during image build
RUN npx -y @ashdev/codex-plugin-metadata-mangabaka@1.0.0 --version || true
```

### Manual Installation (Alternative)

For maximum performance, install globally:

```bash
npm install -g @ashdev/codex-plugin-metadata-mangabaka
```

Then configure:
- **Command**: `codex-plugin-metadata-mangabaka`
- **Arguments**: (leave empty)

## Configuration

### Credentials

The plugin requires a MangaBaka API key. Configure it in the Codex UI or via the API:

```json
{
  "api_key": "mb-123412341234"
}
```

### Credential Delivery Method

This plugin receives credentials via the `initialize` message, so you must set the **Credential Delivery** option appropriately:

| Method | Value | Description |
|--------|-------|-------------|
| Initialize Message | `init_message` | Credentials passed in the JSON-RPC `initialize` request (recommended) |
| Both | `both` | Credentials passed as both environment variables and in `initialize` |

**Note:** The `env` (environment variables only) method will **not work** with this plugin because it reads credentials from the `onInitialize` callback, not from environment variables.

### Parameters

The plugin supports optional parameters to customize behavior:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `base_url` | string | `https://api.mangabaka.org` | Override the API base URL |

Example parameters configuration:

```json
{
  "base_url": "https://api.mangabaka.org"
}
```

### Rate Limiting

The plugin automatically handles rate limiting from the MangaBaka API:

- When rate limited (HTTP 429), the plugin returns a `RateLimitError` with the retry delay
- The `Retry-After` header is used to determine wait time (defaults to 60 seconds)
- Codex will automatically retry requests after the specified delay

If you encounter frequent rate limiting, consider spacing out your metadata refresh operations.

## Using the Plugin

Once enabled, the MangaBaka plugin appears in the series detail page:

1. Navigate to any series in your library
2. Click the **Metadata** button (or look for the plugin icon)
3. Click **Search MangaBaka Metadata**
4. Enter the series title to search
5. Select the best match from the results
6. Preview the metadata changes
7. Click **Apply** to update your series metadata

The plugin will show:
- **Will Apply**: Fields that will be updated
- **Locked**: Fields you've locked (won't be changed)
- **Unchanged**: Fields that already match

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
plugins/metadata-mangabaka/
├── src/
│   ├── index.ts          # Plugin entry point
│   ├── manifest.ts       # Plugin manifest
│   ├── api.ts            # MangaBaka API client
│   ├── mappers.ts        # Response mappers
│   ├── types.ts          # MangaBaka API types
│   └── handlers/
│       ├── search.ts     # Search handler
│       ├── get.ts        # Get metadata handler
│       └── match.ts      # Auto-match handler
├── dist/
│   └── index.js          # Built bundle (excluded from git)
├── package.json
├── tsconfig.json
└── README.md
```

## API Reference

### Search

Searches MangaBaka for series matching a query.

**Parameters:**
- `query`: Search string
- `contentType`: Always `"series"`
- `limit`: Max results (default: 20)
- `cursor`: Page cursor for pagination

**Returns:**
- `results`: Array of search results with `relevanceScore` (0.0-1.0)
- `nextCursor`: Cursor for next page (if available)

### Get

Fetches full metadata for a specific series.

**Parameters:**
- `externalId`: MangaBaka series ID
- `contentType`: Always `"series"`

**Returns:**
- Full series metadata including titles, summary, genres, etc.

### Match

Finds the best match for an existing series (used for auto-matching).

**Parameters:**
- `title`: Series title to match
- `year`: Publication year (optional hint)
- `contentType`: Always `"series"`

**Returns:**
- `match`: Best matching result or `null`
- `confidence`: Match confidence (0.0-1.0)
- `alternatives`: Other potential matches if confidence is low

## Troubleshooting

### "api_key credential is required"

Make sure you've configured the API key in the plugin credentials section.

### "Plugin not initialized"

The plugin hasn't received credentials yet. Check that:
1. The plugin is properly configured in Settings > Plugins
2. Credentials are saved
3. Try disabling and re-enabling the plugin

### "Rate limited"

MangaBaka has API rate limits. The plugin will report the retry delay from the API. Wait for the specified time before retrying. See the [Rate Limiting](#rate-limiting) section for more details.

## License

MIT
