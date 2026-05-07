# @ashdev/codex-plugin-release-nyaa

A Codex plugin that announces new chapter and volume torrent releases for tracked series via [Nyaa.si](https://nyaa.si) uploader and search RSS feeds, limited to an admin-configured allowlist of trusted uploaders. Notification-only: Codex never downloads anything.

## Features

- Polls Nyaa user feeds, plain search queries, and category-scoped searches
- Alias-based matching against tracked series (no Nyaa-side IDs required)
- Per-uploader source rows so each subscription has its own poll cadence, ETag, and last-error status
- Auto-prunes source rows when entries are removed from the uploader list
- Configurable confidence floor below which candidates are dropped silently
- Optional base URL override (useful for mirrors and tests)

## Authentication

None. The plugin only reads public Nyaa.si RSS feeds.

## Admin Setup

### Adding the Plugin to Codex

1. Log in to Codex as an administrator
2. Navigate to **Settings** > **Plugins**
3. Click **Add Plugin**
4. Fill in the form:
   - **Name**: `release-nyaa`
   - **Display Name**: `Nyaa Releases`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-release-nyaa@1.19.0`
5. Click **Save**
6. Set the **Uploaders** config field (see [Uploader Subscriptions](#uploader-subscriptions) below) and save again.
7. Click **Test Connection** to verify the plugin works.

After saving, the host materializes one row per entry in **Settings → Release tracking** — that's where you flip rows on/off, override the poll interval, or hit *Poll now*. Removing an entry from the list and re-saving auto-prunes the corresponding row.

### Tracking & Aliases

The plugin matches Nyaa releases to tracked series by alias, since uploaders rarely embed external IDs in titles. Make sure each tracked series has aliases that cover how the uploader names releases (alternate spellings, romanizations, volume-range tags). Aliases are auto-populated from metadata or can be added manually in the series' Tracking panel.

### npx Options

| Configuration | Arguments | Description |
|--------------|-----------|-------------|
| Latest version | `-y @ashdev/codex-plugin-release-nyaa` | Always uses latest |
| Pinned version | `-y @ashdev/codex-plugin-release-nyaa@1.19.0` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-release-nyaa@1.19.0` | Skips version check if cached |

## Configuration

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `uploaders` | string-array (or legacy CSV) | `[]` | List of trusted uploader handles or queries. See below. |
| `requestTimeoutMs` | number | `10000` | How long to wait for a single Nyaa RSS fetch before giving up, in milliseconds. |
| `baseUrl` | string | `https://nyaa.si` | Override the Nyaa base URL. Useful for mirrors or for tests. |

### Uploader Subscriptions

Each entry in `uploaders` is one of:

| Form | Example | Meaning |
|------|---------|---------|
| `username` | `tsuna69` | A Nyaa user feed |
| `q:<query>` | `q:LuminousScans` | A plain site-wide search |
| `q:?<params>` | `q:?c=3_1&q=Berserk` | URL-style search with the allowlisted keys `q`, `c`, `f`, `u` (e.g. category `3_1` = Literature → English-translated) |

JSON arrays are preferred; comma-separated strings are still accepted for backwards compatibility.

```json
["1r0n", "TankobonBlur", "q:LuminousScans", "q:?c=3_1&q=Berserk"]
```

## How It Works

On `onInitialize` (which the host re-runs after every config save), the plugin parses the `uploaders` list and calls `releases/register_sources`, materializing one `release_sources` row per subscription, keyed on `(plugin_id, sourceKey)` where `sourceKey` is `kind:identifier` (e.g. `user:tsuna69`, `query:luminousscans`, `params:c=3_1&q=berserk`).

On every `releases/poll`:

1. The plugin recovers the subscription from `params.config.subscription` (falling back to parsing `params.sourceKey`).
2. It pulls tracked series and aliases from the host (`releases/list_tracked`).
3. It conditionally GETs the RSS feed using `params.etag`.
4. Each item is parsed, the title is normalized, and the result is matched against every tracked series' alias list. Confidence is `0.95` on exact normalized match and drops to a fuzzy floor of `0.7` for near-matches; below that, the candidate is dropped silently.
5. Surviving candidates are streamed back to the host via `releases/record`.
6. The new ETag and upstream status are returned for the host's per-host backoff layer.

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
plugins/release-nyaa/
├── src/
│   ├── index.ts            # Plugin entry point & poll loop
│   ├── manifest.ts         # Plugin manifest
│   ├── fetcher.ts          # Subscription parsing + conditional GET
│   ├── parser.ts           # RSS item parser
│   ├── matcher.ts          # Title normalization & alias matching
│   └── *.test.ts           # Unit tests
├── dist/
│   └── index.js            # Built bundle (excluded from git)
├── package.json
├── tsconfig.json
└── README.md
```

## License

MIT
