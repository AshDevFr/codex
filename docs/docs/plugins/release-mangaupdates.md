---
---

# MangaUpdates Releases Plugin

The MangaUpdates Releases plugin announces new chapter and volume releases for tracked series by polling per-series RSS feeds at [MangaUpdates](https://www.mangaupdates.com). It is a **notify-only** plugin: Codex surfaces announcements; you acquire externally.

## Features

- Per-series RSS polling against MangaUpdates' v1 API.
- Multi-language support: each scanlation release carries a language tag (English, Spanish, Indonesian, French, German, Portuguese, etc.).
- Per-series language preferences with a server-wide default.
- Admin-configurable scanlation group blocklist.
- Idempotent ledger writes (re-polling never re-announces an already-seen release).
- Daily default poll interval; conditional GET keeps bandwidth low.

## How it works

1. Codex schedules a poll for the source row (default: once per 24 hours).
2. The plugin asks the host for tracked series scoped to those with a `mangaupdates` external ID.
3. For each series, the plugin GETs `https://api.mangaupdates.com/v1/series/{id}/rss`.
4. Each RSS item is parsed into a release candidate: chapter / volume number, scanlation group, language code, release page URL.
5. Candidates are filtered by the configured language list and group blocklist, then submitted to the host's release ledger.
6. The host applies a confidence threshold (1.0 here, since matches are ID-keyed) and dedups on `(source_id, external_release_id)`.
7. On successful insert, `series_tracking.latest_known_chapter` / `latest_known_volume` advance to the high-water mark — but only for releases in the series' effective language list.

The plugin **never** downloads release files. The "Open" link on the inbox row sends you to the MangaUpdates release page; how you acquire the chapter is up to you.

## Setup

### Populating MangaUpdates IDs

For the plugin to find any tracked series, those series need a `mangaupdates` external ID. There are two ways to populate this:

**Manual entry** (works for any series):

1. Go to the series' detail page and open the **Tracking** panel.
2. Add a new external ID with source `mangaupdates` and the numeric ID from the series' MangaUpdates URL (e.g. `https://www.mangaupdates.com/series/abc123/series-name` → use the numeric internal ID exposed by the v1 API).

**Metadata-refresh population**: when the MangaBaka metadata provider runs, it cross-references and stores the MangaUpdates ID automatically for series that exist in MangaBaka's database.

### Language preferences

MangaUpdates aggregates scanlation releases across many languages. The plugin filters announcements to languages you've configured.

- **Per-series**: set `languages` to a list of [ISO 639-1 codes](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes) on the series' tracking config, e.g. `["en"]` for English only or `["en", "es"]` for English + Spanish.
- **Server-wide default**: when a series doesn't override `languages`, the plugin falls back to the `release_tracking.default_languages` setting (default: `["en"]`).
- **Forward-only**: changing the language list affects future polls. Already-recorded ledger rows aren't retroactively hidden — use the inbox's language filter to re-scope an existing view.
- **Untagged entries**: MangaUpdates entries that don't carry a language code are dropped by default. There is no current admin override for this; if you need it, file an issue.

Common language codes:

| Code | Language    |
| ---- | ----------- |
| en   | English     |
| es   | Spanish     |
| id   | Indonesian  |
| fr   | French      |
| de   | German      |
| pt   | Portuguese  |
| it   | Italian     |
| pl   | Polish      |
| ru   | Russian     |

### Group blocklist

Admins can configure `blockedGroups` (comma-separated) to silently drop releases from named scanlation groups. Matching is case-insensitive on the group name as it appears in the RSS title (the part following `by ` and before the language tag). Useful for dropping known low-quality / MTL-only groups.

```
blockedGroups: "MTL Group, LowQualityScans"
```

## Configuration reference

| Field              | Scope        | Default | Notes                                                                 |
| ------------------ | ------------ | ------- | --------------------------------------------------------------------- |
| `blockedGroups`    | admin        | `""`    | CSV. Case-insensitive match on group name.                            |
| `requestTimeoutMs` | admin        | `10000` | Hard timeout per RSS fetch. Clamped to `[1000, 60000]`.               |
| `languages`        | per-series   | `null`  | ISO 639-1 codes. `null` falls back to the server-wide default.        |
| `default_languages` | server-wide | `["en"]` | `release_tracking.default_languages` setting. Affects all release-tracking plugins, not just this one. |

## Limitations

- **Per-series ETags not implemented yet.** The plugin issues unconditional GETs against each tracked series' feed every poll. With daily polls and small per-series feeds this is a non-issue, but it does mean a 304 response is essentially never seen on this source. A future revision will add per-(source, series) state to wire conditional GETs through.
- **Volume bundles are best-effort.** Volume-only entries (e.g. `Vol.15 by VolBundler (en)`) are recognized and announce on the volume axis, but mixed entries (`Vol.2 c.14 by Group (en)`) bump both chapter and volume marks. Whether a volume bundle should retroactively suppress already-announced loose chapters is governed by the host's matcher, not this plugin.
- **No retroactive language re-filter.** Switching `languages` only affects future polls. Old ledger rows in dropped languages stay in the inbox unless dismissed; the inbox's language filter scopes the view.

## Risks

- **Rate limits.** MangaUpdates serves the RSS endpoints publicly without API keys. The plugin uses a daily default poll cadence and per-host backoff (driven by the host) to back off on 429 / 503 responses. Tracking hundreds of series with sub-hourly intervals will likely get you rate-limited; stick to daily.
- **Missing IDs.** Series without a `mangaupdates` external ID are silently skipped. This is by design (the plugin would otherwise have to fuzzy-match titles, which the n8n flow proved is unsafe).
