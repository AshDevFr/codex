---
---

# Nyaa Releases Plugin

The Nyaa Releases plugin announces new chapter and volume torrents for tracked series by polling Nyaa.si user RSS feeds. Unlike the [MangaUpdates plugin](./release-mangaupdates.md), which tells you *what* has been released in your languages, the Nyaa plugin tells you *where to download* a release that exists. It is **notify-only**: Codex never downloads torrents.

## What it's for

Nyaa is an acquisition-pointer source. It complements (not replaces) the translation-feed plugins:

- **MangaUpdates** answers: "Has chapter 143 been released in English?"
- **Nyaa** answers: "Is there a torrent for chapter 143 from a trusted uploader?"

Use Nyaa when you've already decided on a small allowlist of trusted uploaders (e.g. `1r0n`) and want a single feed of "new releases from these people" filtered down to your tracked series.

## Features

- Per-uploader (or per-search-query) RSS polling against Nyaa.si user feeds.
- Alias-based series matching: each parsed Nyaa title is normalized and compared to every tracked series' alias list.
- Confidence scoring: exact normalized match → 0.95; fuzzy near-match (Sørensen-Dice) → 0.7-0.85; everything below is dropped before reaching the host.
- Format-hint extraction: `(Digital)`, `(JXL)`, `(Magazine)`, etc. surface on the candidate's `formatHints` for downstream filtering.
- Volume and chapter ranges are recognized: `[1r0n] Boruto v01-14 (Digital)` and `[Group] Dandadan c126-142 (Digital)` parse correctly and pass both ends to the host.
- Idempotent ledger writes (re-polling never re-announces an already-seen release).
- Daily default poll interval; conditional GETs (ETag + Last-Modified) keep bandwidth low.
- Per-host backoff is driven by the host on 429 / 503 responses.

## How it works

1. Codex schedules a poll for the source row (default: once per 24 hours).
2. The plugin reads the configured uploader subscription list.
3. The plugin asks the host for tracked series along with their aliases (`releases/list_tracked` with `requires_aliases: true`).
4. For each subscription, the plugin fetches the Nyaa feed:
   - User feed: `https://nyaa.si/?page=rss&u=<username>`
   - Search feed (for groups without a user account): `https://nyaa.si/?page=rss&q=<query>`
5. Each RSS item is parsed: a leading `[Group]` token, chapter / volume token (single or range), and parenthesized format hints are extracted; the remaining text is the *series guess*.
6. The series guess is normalized and matched against tracked-series aliases. Confidence ≥ 0.95 on exact normalized match; otherwise the matcher computes a token-level Dice ratio and rejects below 0.85.
7. Matching candidates are submitted to the host's release ledger via `releases/record`. The host applies its threshold (default 0.7) and dedups on `(source_id, external_release_id)` and on `info_hash` (Nyaa's `nyaa:infoHash` element).

The plugin **never** downloads release files. The "Open" link on the inbox row sends you to the Nyaa view page or the `.torrent` URL; how you acquire the chapter is up to you.

## Setup

### Configure uploader subscriptions

The plugin's `uploaders` admin field is a comma-separated list of trusted uploader handles or queries:

```
uploaders: "1r0n,TankobonBlur,q:LuminousScans"
```

- Plain identifier (`1r0n`) → user feed (`https://nyaa.si/?page=rss&u=1r0n`).
- `q:<query>` or `query:<query>` → search feed (`https://nyaa.si/?page=rss&q=<query>`). Use this for groups without a Nyaa account, or to scope by tag.

Empty tokens are dropped; case-insensitive duplicates are silently deduplicated. The plugin walks subscriptions in declaration order on each poll.

### Make sure tracked series have aliases

Nyaa releases identify a series only by name in the title. The plugin matches titles to series via the `series_aliases` table:

- The `BackfillTrackingFromMetadata` task (Phase 1) seeds aliases from each series' `series_metadata.title`, `title_sort`, and alternate titles.
- You can also add aliases manually via the Tracking panel on a series detail page.

For best results, add aliases that mirror how your trusted uploaders name the release. Example: 1r0n names `Boruto: Two Blue Vortex` as `[1r0n] Boruto - Two Blue Vortex - Volume NN (Digital)`. The default normalization produces `boruto two blue vortex` from both forms, so an exact match is automatic — but if you track *Boruto* with only the alias `Boruto`, the matcher will see `boruto two blue vortex` and reject it as not similar enough to `boruto`.

### Source row

A `release_sources` row with `plugin_id="release-nyaa"` and `kind="rss-uploader"` must exist before the scheduler will poll. (See [Release tracking architecture](../architecture/release-tracking.md) for the broader picture; admin UI to create and manage source rows is tracked as a follow-up.)

## Configuration reference

| Field              | Scope        | Default                | Notes                                                                                              |
| ------------------ | ------------ | ---------------------- | -------------------------------------------------------------------------------------------------- |
| `uploaders`        | admin        | `""`                   | Comma-separated subscription list. Plain identifier = user feed; `q:<query>` = search feed.        |
| `requestTimeoutMs` | admin        | `10000`                | Hard timeout per Nyaa fetch. Clamped to `[1000, 60000]`.                                           |
| `baseUrl`          | admin        | `https://nyaa.si`      | Override base URL — useful for mirrors. Trailing slashes are trimmed.                              |

## Limitations

- **One source row, many uploaders.** The plan called for one source row per uploader subscription, but the host has no admin endpoint for creating `release_sources` rows yet. Until that ships, all uploader subscriptions ride a single source row's poll cadence and ETag bucket. With daily polls the difference is academic; if you're adding many uploaders or want per-uploader poll intervals, this will need revisiting.
- **ETag is single-bucket.** The source row stores one ETag — the plugin uses it on the *first* uploader fetched and walks subsequent uploaders unconditionally. Daily polls + small RSS bodies make this acceptable; per-subscription ETags would need per-(source, subscription) state.
- **Language is hardcoded to English.** Nyaa releases don't carry a language tag, and 99% of the uploaders this plugin targets release English-language scans. Admins who add non-English uploaders should configure tracked series' `languages` accordingly so the host's `latest_known_*` advance gate doesn't pollute the high-water mark with releases the user can't read.
- **Title parsing is best-effort.** The corpus covers the common 1r0n / TankobonBlur shapes plus generic `Volume NN` / `Chapter NNN` forms. Edge-case titles (e.g. unusual punctuation, missing separators) may parse with an empty `seriesGuess`; the matcher silently rejects those entries (no false positives).
- **No per-uploader confidence weighting in v1.** Every matched candidate gets the same confidence based on the alias match alone. Adding per-uploader trust scores (downgrade an uploader after N user dismissals) is on the roadmap but not load-bearing at v1's tracked-series scale.

## Risks

- **Rate limits.** Nyaa serves RSS publicly without API keys, but it's a small site and aggressive polling is unwelcome. The plugin uses a daily default cadence and per-host backoff (driven by the host) to back off on 429 / 5xx responses. Don't reduce the interval below the default unless you have a specific reason.
- **Title-parsing false positives.** Alias-only matching is fundamentally fuzzier than the external-ID match used by MangaUpdates. The matcher's 0.85 Dice floor + 0.95 exact-confidence give the host's threshold (default 0.7) enough headroom to drop bad matches, but watch the inbox for the first few days after enabling and dismiss anything mis-matched. Repeated dismissals tell you which series need additional aliases.
- **Quality varies by uploader.** This is *acquisition pointer* data. The plugin doesn't validate that the underlying torrent is what its title claims to be; that's why the user maintains the uploader allowlist.
