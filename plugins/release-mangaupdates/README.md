# @ashdev/codex-plugin-release-mangaupdates

A Codex plugin that announces new chapter and volume releases for tracked manga series via [MangaUpdates](https://www.mangaupdates.com) per-series RSS feeds. Notification-only: Codex never downloads anything.

## Features

- Per-series RSS polling via the MangaUpdates feed for each tracked series
- Filters releases by per-series language preferences (with a server-wide default)
- Server-wide scanlation group blocklist
- Conditional GET (ETag) support to keep upstream load low
- Auto-registers itself as a single source row in **Settings → Release tracking** on first start
- No credentials required — MangaUpdates RSS feeds are public

## Authentication

None. The plugin only reads public per-series RSS feeds.

## Admin Setup

### Adding the Plugin to Codex

1. Log in to Codex as an administrator
2. Navigate to **Settings** > **Plugins**
3. Click **Add Plugin**
4. Fill in the form:
   - **Name**: `release-mangaupdates`
   - **Display Name**: `MangaUpdates Releases`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-release-mangaupdates@1.19.0`
5. Click **Save**
6. Click **Test Connection** to verify the plugin works

On first start the plugin auto-registers a single source row (`MangaUpdates Releases`) in **Settings → Release tracking**, where you can disable it, change the poll interval, or hit *Poll now*.

### Linking Series to MangaUpdates

For a tracked series to receive announcements, it needs a `mangaupdates` external ID. Either:

- Let a metadata-provider plugin populate it (for example, MangaBaka cross-references), or
- Paste the ID manually into the series' tracking panel

### npx Options

| Configuration | Arguments | Description |
|--------------|-----------|-------------|
| Latest version | `-y @ashdev/codex-plugin-release-mangaupdates` | Always uses latest |
| Pinned version | `-y @ashdev/codex-plugin-release-mangaupdates@1.19.0` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-release-mangaupdates@1.19.0` | Skips version check if cached |

## Configuration

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `blockedGroups` | string (CSV) | `""` | Comma-separated scanlation group names to exclude from announcements (case-insensitive, exact match). Per-series overrides may further extend this list. |
| `requestTimeoutMs` | number | `10000` | How long to wait for a single RSS fetch before giving up, in milliseconds. |

Per-series language preferences live on each series' tracking config and override the server default (`release_tracking.default_languages`).

## How It Works

On every `releases/poll`:

1. The plugin pulls the tracked-series scope from the host (filtered server-side to series with a `mangaupdates` external ID).
2. For each series, it conditionally GETs the RSS feed using the stored ETag.
3. Items are filtered by per-series language list and the admin-configured group blocklist.
4. Surviving items are streamed back to the host via `releases/record`. The host's matcher applies the threshold and ledger dedup.
5. The new ETag is passed back so the host updates the source row.

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
plugins/release-mangaupdates/
├── src/
│   ├── index.ts            # Plugin entry point & poll loop
│   ├── manifest.ts         # Plugin manifest
│   ├── fetcher.ts          # Conditional GET against MangaUpdates RSS
│   ├── parser.ts           # RSS item parser
│   ├── filter.ts           # Language + blocklist filtering
│   └── *.test.ts           # Unit tests
├── dist/
│   └── index.js            # Built bundle (excluded from git)
├── package.json
├── tsconfig.json
└── README.md
```

## License

MIT
