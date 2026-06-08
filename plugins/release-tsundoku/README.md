# @ashdev/codex-plugin-release-tsundoku

A Codex release-source plugin that announces new volume and chapter coverage
for your tracked series using a [Tsundoku](https://github.com/AshDevFr) instance's
incremental series feed. **Notification-only** — Codex does not download anything.

## Features

- **Exact external-ID matching, no fuzzy logic.** Series are matched to your
  Codex catalog by provider IDs (MangaBaka, AniList, MAL, MangaUpdates, Kitsu,
  Shikimori, Anime-Planet, Anime News Network), so announcements always land on
  the right series with full confidence.
- **Incremental, cursor-based.** Walks Tsundoku's keyset-paginated
  `/api/v1/series/feed`, persisting its position in the source's state (etag
  slot) so each poll only processes activity since the last run.
- **Volume- and chapter-aware.** The feed's merged, gap-preserving coverage
  spans map directly onto Codex's release model.

## Authentication

None. The Tsundoku feed endpoint is public; you only need to point the plugin at
your instance with `baseUrl`.

## Admin Setup

### Adding the Plugin to Codex

Add the plugin from **Settings → Plugins** (it appears in the official plugin
gallery as "Tsundoku Releases"), or configure it manually:

- **Command:** `npx`
- **Args:**
  ```
  -y
  @ashdev/codex-plugin-release-tsundoku
  ```

Set `baseUrl` to your Tsundoku instance (e.g. `https://tsundoku.example.com`)
and save. The plugin auto-registers a single source row ("Tsundoku Releases") in
**Settings → Release tracking**, where you can disable it, change the poll
interval, or trigger an immediate poll.

### Linking Series to Tsundoku

A series is matched whenever it carries at least one external ID that Tsundoku
also knows. Populate these by running a metadata refresh (e.g. the MangaBaka
metadata plugin) or by pasting an ID into the series' tracking panel. Supported
providers, in match-priority order:

`mangabaka`, `anilist`, `mal`, `mangaupdates`, `kitsu`, `shikimori`,
`anime_planet`, `anime_news_network`.

## Configuration

| Field              | Required | Default | Description                                                              |
| ------------------ | -------- | ------- | ------------------------------------------------------------------------ |
| `baseUrl`          | yes      | —       | Tsundoku instance base URL. The plugin appends `/api/v1/series/feed`.    |
| `defaultLanguage`  | no       | `en`    | ISO 639-1 tag stamped on every announcement (the feed carries none).     |
| `pageLimit`        | no       | `100`   | Items per feed page (1–500).                                             |
| `requestTimeoutMs` | no       | `10000` | Per-page fetch timeout in milliseconds.                                   |

## How It Works

On each poll the plugin:

1. Loads its stored feed cursor (the host hands it back as the source's
   persisted `etag`).
2. Builds a reverse index (`provider:id → Codex series`) from your tracked
   series via the host's `releases/list_tracked`.
3. Walks the feed from the cursor. Each item is matched against the index by
   external ID; on a hit it records a release candidate (confidence `1.0`) whose
   `volumes`/`chapters` mirror the item's coverage. The cursor is persisted after
   each processed page, so an interrupted walk resumes cleanly.
4. Reports counters back to the host; the host applies its own threshold,
   auto-ignore (for coverage you already own), and dedup.

The candidate's dedup key is the coverage high-water mark
(`tsundoku:{seriesId}:v{highestVolume}:c{highestChapter}`), so a new
announcement fires only when the frontier advances; re-delivery of the same
coverage dedups host-side.

If the very first feed page can't be fetched (e.g. `baseUrl` is wrong or the
instance is unreachable), the poll fails and the source shows `last_error` in
**Settings → Release tracking** rather than silently reporting "0 items". In
Docker, remember the plugin runs inside the worker container: use a URL the
container can resolve (e.g. `http://host.docker.internal:<port>`), not
`http://localhost:<port>`.

### Limitations

- **Default language.** Tsundoku tracks official release coverage and carries no
  language, so every candidate uses `defaultLanguage` (`en` unless overridden).
  Per-series language preferences still gate the high-water mark host-side.
- **Incremental backfill gap.** Because the walk is cursor-based, a series you
  start tracking *after* its last Tsundoku coverage change won't get a catch-up
  announcement until it changes again. This is correct for "new releases going
  forward"; a full backfill would require resetting the cursor.
- **High-water dedup.** A filled interior gap that doesn't move the highest
  volume/chapter won't re-announce.

## Development

```bash
# Install dependencies
npm install

# Build the plugin
npm run build

# Type check
npm run typecheck

# Run tests
npm run test

# Lint
npm run lint
```

## Project Structure

```
src/
├── index.ts        # Plugin lifecycle, config, source registration, poll loop
├── manifest.ts     # Capability + config schema + supported providers
├── fetcher.ts      # Feed wire types + paginated fetchFeedPage
├── matcher.ts      # Reverse index + exact external-ID matching
└── candidate.ts    # Feed item → ReleaseCandidate mapping
```

## License

MIT
