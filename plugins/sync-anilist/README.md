# @ashdev/codex-plugin-sync-anilist

A Codex plugin for syncing manga reading progress between Codex and [AniList](https://anilist.co). Supports push/pull of reading status, chapters read, scores, and dates.

## Features

- Two-way sync of manga reading progress with AniList
- Push reading status, chapters read, scores, and dates to AniList
- Pull updates from AniList back to Codex
- Highest-progress-wins conflict resolution (progress only moves forward)
- External ID matching via AniList API IDs (`api:anilist`)

## Authentication

This plugin supports two authentication methods:

### OAuth (Recommended)

If your Codex administrator has configured OAuth:

1. Go to **Settings** > **Integrations**
2. Click **Connect with AniList Sync**
3. Authorize Codex on AniList
4. You're connected!

### Personal Access Token

If OAuth is not configured by the admin:

1. Go to [AniList Developer Settings](https://anilist.co/settings/developer)
2. Click **Create New Client**
3. Set the redirect URL to `https://anilist.co/api/v2/oauth/pin`
4. Click **Save**, then **Authorize** your new client
5. Copy the token shown on the pin page
6. In Codex, go to **Settings** > **Integrations**
7. Paste the token in the access token field and click **Save Token**

## Admin Setup

### Adding the Plugin to Codex

1. Log in to Codex as an administrator
2. Navigate to **Settings** > **Plugins**
3. Click **Add Plugin**
4. Fill in the form:
   - **Name**: `sync-anilist`
   - **Display Name**: `AniList Sync`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-sync-anilist@1.9.3`
5. Click **Save**
6. Click **Test Connection** to verify the plugin works

### Configuring OAuth (Optional)

To enable OAuth login for your users:

1. Go to [AniList Developer Settings](https://anilist.co/settings/developer)
2. Click **Create New Client**
3. Set the redirect URL to `{your-codex-url}/api/v1/user/plugins/oauth/callback`
4. Save and copy the **Client ID**
5. In Codex, go to **Settings** > **Plugins** > click the gear icon on AniList Sync
6. Go to the **OAuth** tab
7. Paste the **Client ID** (and optionally the **Client Secret**)
8. Click **Save Changes**

Without OAuth configured, users can still connect by pasting a personal access token.

### npx Options

| Configuration | Arguments | Description |
|--------------|-----------|-------------|
| Latest version | `-y @ashdev/codex-plugin-sync-anilist` | Always uses latest |
| Pinned version | `-y @ashdev/codex-plugin-sync-anilist@1.9.3` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-sync-anilist@1.9.3` | Skips version check if cached |

## Configuration

### Plugin Config

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `syncRatings` | boolean | `false` | Include user ratings and notes in sync. When off, only reading progress is synced. |

## Using the Plugin

Once connected, the sync plugin works automatically:

1. Go to **Settings** > **Integrations**
2. Click **Sync Now** to trigger a manual sync
3. View sync status including pulled/pushed/applied counts

The plugin matches Codex series to AniList entries using external IDs stored in the `series_external_ids` table with the `api:anilist` source.

## Sync Behavior

### Sync Modes

You can configure which direction data flows in **Settings** > **Integrations** > **Settings**:

| Mode | Description |
|------|-------------|
| **Pull & Push** (default) | Import progress from AniList, then export Codex progress to AniList |
| **Pull Only** | Import progress from AniList without writing anything back |
| **Push Only** | Export Codex progress to AniList without importing |

### How Sync Works

When sync runs in **Pull & Push** mode, it executes two phases in order:

1. **Pull** — Fetches your reading list from AniList, matches entries to Codex series via external IDs, and marks the corresponding books as read in Codex.
2. **Push** — Reads your current Codex reading progress and sends it to AniList, overwriting the remote entry.

### Conflict Resolution

Codex uses a **highest progress wins** strategy. There is no manual conflict resolution — instead, progress can only move forward:

| Scenario | Result |
|----------|--------|
| AniList ahead (e.g., 5 vols read) and Codex behind (3 vols read) | Pull marks books 4–5 as read in Codex. Push sends 5 to AniList. Both agree. |
| Codex ahead (5 vols read) and AniList behind (3 vols read) | Pull tries to mark first 3 as read — already completed, skipped. Push sends 5 to AniList. **Codex wins.** |
| Both changed differently | Pull applies remote progress first (additive only), then Push sends local state to AniList. Effectively **Codex wins** because push runs last. |

Key behaviors:

- **Pull is additive only** — it marks unread books as read but never un-reads a book. If you lower your chapter count on AniList, that change is ignored.
- **Push overwrites the remote** — after pulling, Codex sends its current state to AniList, which may overwrite changes made directly on AniList.
- **Progress is monotonic** — once a book is marked as read, sync will not undo it. Progress only moves forward.

### Completed Status

The plugin is conservative about marking series as "Completed" on AniList:

- A series is pushed as **Completed** only when all local books are read **and** the series metadata includes a `total_book_count` that matches.
- Otherwise, the series is pushed as **Reading** — even if all local books are read — because Codex can't be sure the library contains the full series.

### Rating & Notes Sync

When **Sync Ratings & Notes** is enabled in plugin settings:

- **Push**: Codex ratings (1-100 scale) and notes are sent to AniList, converted to the user's chosen AniList score format (auto-detected from their profile).
- **Pull**: AniList scores and notes are imported into Codex, but only when Codex has **no existing rating** for that series. Existing Codex ratings are never overwritten (**Codex wins**).
- Notes without a score are skipped on pull (Codex requires a rating to store notes).

### Push Configuration

These options are available in the plugin settings:

| Option | Default | Description |
|--------|---------|-------------|
| Progress Unit | Volumes | Whether each Codex book counts as a "volume" or "chapter" on AniList. Use "volumes" to avoid misleading "Read chapter X" activity on AniList. |
| Push Completed Series | On | Include series where all local books are read |
| Push In-Progress Series | On | Include series where at least one book has been started |
| Count In-Progress Books | Off | Whether partially-read books count toward the progress number |
| Auto-Pause After Days | 0 (disabled) | Number of days without reading activity before an in-progress series is set to Paused on AniList |
| Auto-Drop After Days | 0 (disabled) | Number of days without reading activity before an in-progress series is set to Dropped on AniList |

### Auto-Pause & Auto-Drop

You can configure automatic status changes for series you haven't read in a while:

| Configuration | Behavior |
|--------------|----------|
| Pause=5, Drop=0 | Not read in 5 days -> Paused |
| Pause=5, Drop=7 | Not read in 5 days -> Paused; not read in 7 days -> Dropped |
| Pause=5, Drop=3 | Not read in 3 days -> Dropped (drop fires first since threshold is shorter) |
| Pause=0, Drop=4 | Not read in 4 days -> Dropped (no pause step) |
| Pause=0, Drop=0 | Disabled (default) |

Key behaviors:

- **Only affects in-progress series** — completed series are never auto-paused or auto-dropped.
- **Based on last reading activity** — the timer resets every time you read any book in the series.
- **Drop takes priority** — if both pause and drop thresholds are met, the series is dropped (not paused).

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
plugins/sync-anilist/
├── src/
│   ├── index.ts          # Plugin entry point
│   ├── manifest.ts       # Plugin manifest
│   ├── anilist.ts        # AniList API client
│   └── anilist.test.ts   # API client tests
├── dist/
│   └── index.js          # Built bundle (excluded from git)
├── package.json
├── tsconfig.json
└── README.md
```

## License

MIT
