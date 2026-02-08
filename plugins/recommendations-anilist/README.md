# @ashdev/codex-plugin-recommendations-anilist

A Codex plugin for personalized manga recommendations powered by [AniList](https://anilist.co) community data. Generates recommendations based on your reading history and ratings.

## Features

- Personalized manga recommendations from AniList
- Based on your library ratings and reading history
- Configurable maximum number of recommendations
- Uses AniList's recommendation and user list APIs

## Authentication

This plugin supports two authentication methods:

### OAuth (Recommended)

If your Codex administrator has configured OAuth:

1. Go to **Settings** > **Integrations**
2. Click **Connect with AniList Recommendations**
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
   - **Name**: `recommendations-anilist`
   - **Display Name**: `AniList Recommendations`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-recommendations-anilist@1.9.3`
5. Click **Save**
6. Click **Test Connection** to verify the plugin works

### Configuring OAuth (Optional)

To enable OAuth login for your users:

1. Go to [AniList Developer Settings](https://anilist.co/settings/developer)
2. Click **Create New Client**
3. Set the redirect URL to `{your-codex-url}/api/v1/user/plugins/oauth/callback`
4. Save and copy the **Client ID**
5. In Codex, go to **Settings** > **Plugins** > click the gear icon on AniList Recommendations
6. Go to the **OAuth** tab
7. Paste the **Client ID** (and optionally the **Client Secret**)
8. Click **Save Changes**

Without OAuth configured, users can still connect by pasting a personal access token.

### npx Options

| Configuration | Arguments | Description |
|--------------|-----------|-------------|
| Latest version | `-y @ashdev/codex-plugin-recommendations-anilist` | Always uses latest |
| Pinned version | `-y @ashdev/codex-plugin-recommendations-anilist@1.9.3` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-recommendations-anilist@1.9.3` | Skips version check if cached |

## Configuration

### Plugin Config

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `maxRecommendations` | number | `20` | Maximum number of recommendations to generate (1-50) |

## Using the Plugin

Once connected, recommendations appear in the Codex UI:

1. Go to **Settings** > **Integrations** and verify the plugin shows as **Connected**
2. Recommendations are generated based on your library ratings and reading history

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
plugins/recommendations-anilist/
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
