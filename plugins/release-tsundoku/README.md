# @ashdev/codex-plugin-release-tsundoku

A Codex release-source plugin that announces new volume and chapter coverage
for your tracked series using a [Tsundoku](https://github.com/AshDevFr) instance's
incremental series feed. **Notification-only** — Codex does not download anything.

## Features

- **External-ID matching by weighted voting, no title fuzzing.** Series are
  matched to your Codex catalog by provider IDs (MangaBaka, AniList, MAL,
  MangaUpdates, Kitsu, Shikimori, Anime-Planet, Anime News Network). Because
  some providers occasionally share/merge IDs across distinct series, each
  shared ID *votes*: an agreeing ID adds its weight (MangaBaka 3, AniList 2,
  rest 1), a disagreeing one subtracts it, and a series matches only when
  agreement wins. So a trusted disagreement (different MangaBaka IDs) overrides
  a sloppy agreement (a shared MAL ID), and genuinely ambiguous ties are
  skipped rather than mis-attributed.
- **Filtered feed, no stored cursor.** Each poll `POST`s your tracked series'
  `provider:externalId` set to Tsundoku's filtered `/api/v1/series/feed`, so the
  response contains only your series (not the whole catalog). There's no
  persisted cursor — every poll re-evaluates your tracked set's current
  coverage and lets Codex dedup unchanged releases. Newly tracked series are
  picked up automatically and untracked ones drop out, with no bookkeeping.
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

1. Builds a match context from your tracked series via the host's
   `releases/list_tracked`, and derives the `provider:externalId` filter set.
2. `POST`s that filter to `/api/v1/series/feed`, paginating through the response
   (cursor used only within the poll; nothing is persisted). The response is
   narrowed to your tracked series.
3. Matches each returned item to a tracked series by weighted external-ID voting
   (see Features); on a confident match it records a release candidate whose
   `volumes`/`chapters` mirror the item's coverage and whose confidence reflects
   the vote. When several feed entries map to the same Codex series, only the
   best-scoring one is recorded (ambiguous ties are skipped).
4. Reports counters back to the host; the host applies its own threshold,
   auto-ignore (for coverage you already own), and dedup.

The candidate's dedup key is the coverage high-water mark
(`tsundoku:{seriesId}:v{highestVolume}:c{highestChapter}`), so a new
announcement fires only when the frontier advances; re-delivery of the same
coverage dedups host-side. Because each poll re-evaluates the full tracked set,
**newly tracked series are backfilled on the next poll** and untracked ones stop
without any cursor bookkeeping.

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
- **Full re-walk each poll.** Each poll re-fetches the current coverage of your
  whole tracked set (filtered server-side, so only your series). Cheap at
  typical sizes and polled a few times a day; if it ever needs to scale, an
  incremental cursor could be reintroduced (with explicit invalidation on
  track/untrack).
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
├── matcher.ts      # Weighted-vote external-ID matching + cross-item resolution
└── candidate.ts    # Feed item → ReleaseCandidate mapping
```

## License

MIT
