---
---

# MangaBaka Metadata Plugin

The MangaBaka metadata plugin fetches manga, manhwa, and manhua metadata from [MangaBaka](https://mangabaka.org), which aggregates data from multiple sources (AniList, MyAnimeList, MangaDex, and others) into a single normalized record. This is the recommended series-level metadata provider for manga libraries.

## Features

- Search for manga, manhwa, or manhua by title.
- Fetch comprehensive series metadata: titles in multiple languages (English, Japanese, Korean, Chinese), synopsis, publication status, genres, tags, authors, artists, cover images, ratings.
- Cross-reference IDs to AniList, MyAnimeList, and MangaDex (also populates the MangaUpdates external ID, which is what the [MangaUpdates Releases](./release-mangaupdates.md) plugin uses to look up release feeds).
- Handles 429 rate-limit responses with `Retry-After` so the host can back off cleanly.

## Prerequisites

You need a MangaBaka API key:

1. Create an account at [mangabaka.org](https://mangabaka.org).
2. Go to [Settings > API](https://mangabaka.org/settings/api).
3. Generate an API key.

## Installation

The plugin ships in the official plugin store. From **Settings → Plugins**, find **MangaBaka Metadata** in the carousel and click **Add**. The pre-filled form has the recommended command and arguments; fill in your API key in the **Credentials** tab and save.

If you prefer to add it manually:

1. Log in as an administrator and navigate to **Settings → Plugins**.
2. Click **Add Plugin** and fill in the form:
   - **Name**: `metadata-mangabaka`
   - **Display Name**: `MangaBaka Metadata`
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0`
3. In the **Credentials** tab:
   - **Credential Delivery**: `Initialize Message` (or `Both`)
   - **Credentials**: `{"api_key": "your-mangabaka-api-key"}`
4. Save, click **Test Connection** to verify, then toggle **Enabled**.

:::caution Credential delivery
The plugin reads its API key from the JSON-RPC `initialize` payload, not from environment variables. **Credential Delivery** must be set to `Initialize Message` or `Both`. The `env` (environment variables only) option will not work.
:::

### npx options

| Configuration | Arguments | When to use |
| --- | --- | --- |
| Latest version | `-y @ashdev/codex-plugin-metadata-mangabaka` | Auto-update on every spawn |
| Pinned version | `-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Recommended for production |
| Fast startup | `-y --prefer-offline @ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Skips the npm version check when the package is already cached |

For Docker deployments, pre-warm the npx cache during the image build:

```dockerfile
RUN npx -y @ashdev/codex-plugin-metadata-mangabaka@1.0.0 --version || true
```

### Plugin permissions

By default the plugin requests every series-level metadata write permission. Trim the list in **Settings → Plugins → MangaBaka Metadata → Configuration → Permissions** if you want to limit which fields it can change.

![Plugin Permissions](../../screenshots/plugins/config-modal-permissions.png)

## Configuration

Configure the plugin in **Settings → Plugins → MangaBaka Metadata → Configuration**.

![Plugin Config - General](../../screenshots/plugins/config-modal-general.png)

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `timeout` | number | `60` | HTTP request timeout in seconds for each MangaBaka API call. |
| `sort_by` | string | `relevance_desc` | Sort order for search results. Valid values: `relevance_desc`, `popularity_asc` (recommended; surfaces well-known series first), `popularity_desc`, `title_asc`, `title_desc`, `created_at_desc`, `created_at_asc`. |
| `base_url` | string | `https://api.mangabaka.org` | Override the API base URL (rarely needed). |

### Search sort order

`relevance_desc` is MangaBaka's default and weights against your query string. For libraries dominated by mainstream titles, switching to `popularity_asc` produces better matches because the popular candidate usually wins ties; for obscure or self-published series, `relevance_desc` is safer.

## How it works

### Search and match

When you click **Fetch Metadata → MangaBaka Metadata** on a series, the plugin issues a search against `/v1/series` with the series title and the configured `sort_by`. Results come back ranked by MangaBaka, and the plugin re-scores each one locally using a title-similarity heuristic before presenting the candidate list to the UI.

For auto-match (used by [scheduled library jobs](../library-jobs.md) and the library-level **Auto-match** action), the plugin runs the same search and accepts the top result only when the local similarity score is high enough; otherwise it returns `null` so the host can fall through to the next provider.

### Metadata mapping

| MangaBaka field | Codex field | Notes |
| --- | --- | --- |
| `title` | Title | Canonical title |
| `alternate_titles` | Alternate titles | Includes Japanese / Korean / Chinese / English variants when present |
| `description` | Summary | |
| `status` | Status | `ongoing`, `completed`, `hiatus`, `cancelled` |
| `genres` | Genres | |
| `tags` | Tags | Tagged with category and rank where MangaBaka provides them |
| `authors` / `artists` | Authors | Mapped with `author` / `artist` roles |
| `cover_image` | Cover Image | URLs in multiple sizes |
| `score` | Community rating | |
| `external_ids.{anilist,mal,mangadex,mangaupdates}` | External IDs | Populated alongside the MangaBaka ID |

### External IDs

MangaBaka stores cross-source identifiers. After applying metadata, the series gets a `mangabaka` external ID (the MangaBaka series ID) plus any of `anilist`, `mal`, `mangadex`, and `mangaupdates` that MangaBaka knows about. Other plugins that key on those sources benefit immediately:

- The [AniList Sync](./anilist-sync.md) plugin can sync the series without manually pasting an AniList ID.
- The [MangaUpdates Releases](./release-mangaupdates.md) plugin starts polling the series on its next run.
- The [AniList Recommendations](./anilist-recommendations.md) plugin matches the series against your seed entries without a fallback title search.

This is why MangaBaka is the recommended first-pass provider for manga libraries: it warms up several other plugins as a side effect.

## Usage

### From a series detail page

1. Open a series in your library.
2. Click **Fetch Metadata** in the actions menu, then **MangaBaka Metadata**.
3. Enter or confirm the search title.
4. Pick the best match from the results list.
5. Review the diff (Will Apply / Locked / Unchanged).
6. Click **Apply**.

The plugin's actions surface from the series detail dropdown:

![Series Detail - Plugin Dropdown](../../screenshots/plugins/series-detail-plugin-dropdown.png)

Search results show ranked candidates with similarity scoring:

![Plugin Search Results](../../screenshots/plugins/search-results.png)

After picking one, the metadata diff preview lists every field that will change:

![Metadata Preview](../../screenshots/plugins/metadata-preview.png)

![Apply Success](../../screenshots/plugins/apply-success.png)

### From a library auto-match

1. Open a library page.
2. Click the plugin dropdown in the library header and pick **Auto-match → MangaBaka Metadata**.
3. The plugin runs against every untracked series in the library and applies high-confidence matches automatically. Results show in the library auto-match panel afterwards.

![Library Sidebar - Plugin Dropdown](../../screenshots/plugins/library-sidebar-plugin-dropdown.png)

![Library Auto-Match Success](../../screenshots/plugins/library-auto-match-success.png)

### Via a scheduled library job

Configure a [library job](../library-jobs.md) with:

- **Provider**: `metadata-mangabaka`
- **Scope**: `Series only`
- **Field groups**: pick whichever buckets you want refreshed periodically (typically `status`, `counts`, `ratings`, `tags`)
- **Existing source IDs only**: on (so the job only refreshes series that have already been matched)

Pair this with **Skip recently synced within (s)** to keep API traffic low.

## Rate limiting

MangaBaka's free tier is rate-limited. The plugin propagates 429 responses up to Codex with the `Retry-After` value (defaults to 60s). The host scheduler backs off accordingly and retries on the next available slot. If you frequently see 429s in the plugin's failure log, lower the concurrency on your library jobs (`maxConcurrency` in the job config) or stretch the cron cadence.

## Troubleshooting

### "api_key credential is required"

The plugin received no credentials. Verify the `Credentials` tab has `api_key` set, the credential-delivery method is `init_message` or `both`, and click **Test Connection** to revalidate.

### "Plugin not initialized"

The plugin process never received a successful `initialize` call. Disable and re-enable the plugin to force a restart, then check the failures panel in **Settings → Plugins → MangaBaka Metadata** for the underlying error.

### Search returns no matches

- Try the title in romaji rather than the English-localized form (e.g. `Kingdom` rather than the localized variant).
- Switch `sort_by` to `popularity_asc` for mainstream series.
- Use the **MangaBaka search URI** the plugin advertises (link icon in the search modal) to verify the title exists upstream.

### Wrong match auto-applied

Auto-match only applies high-confidence matches. If a wrong match landed, run **Reset metadata** on the series (see [Managing Series](../series-management.md#reset-metadata)) and use the per-series **Fetch Metadata** flow to pick the right candidate manually.

## Next steps

- [AniList Sync](./anilist-sync.md): keep reading progress in sync once the AniList ID has been populated.
- [MangaUpdates Releases](./release-mangaupdates.md): announce new chapters once the MangaUpdates ID has been populated.
- [Library Jobs](../library-jobs.md): schedule periodic refreshes against MangaBaka.
