# @ashdev/codex-plugin-sync-anilist

A Codex plugin for syncing manga reading progress between Codex and [AniList](https://anilist.co). Supports push/pull of reading status, chapters read, scores, and dates.

## Features

- Two-way sync of manga reading progress with AniList
- Push reading status, chapters read, scores, and dates to AniList
- Pull updates from AniList back to Codex
- Conflict detection when both sides have changed
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
| `scoreFormat` | string | `POINT_10` | How scores are mapped. AniList supports `POINT_100`, `POINT_10_DECIMAL`, `POINT_10`, `POINT_5`, `POINT_3` |

## Using the Plugin

Once connected, the sync plugin works automatically:

1. Go to **Settings** > **Integrations**
2. Click **Sync Now** to trigger a manual sync
3. View sync status including pending push/pull counts

The plugin matches Codex series to AniList entries using external IDs stored in the `series_external_ids` table with the `api:anilist` source.

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
