# MangaBaka Recommendations Plugin

Manga recommendations for [Codex](https://github.com/AshDevFr/codex), powered by
[MangaBaka](https://mangabaka.org). **No account, no OAuth, and no API key required.**

> **Status: in development.** Content-based recommendations work end to end. The collaborative
> signal and the user-facing filter settings are not wired up yet.

## How it works

The plugin combines two signals from MangaBaka's public API:

| Signal                  | Endpoint                                  | What it gives you                                          |
| ----------------------- | ----------------------------------------- | ---------------------------------------------------------- |
| Content similarity      | `GET /v1/series/mix`                      | Series whose tag profile matches your library as a whole    |
| Collaborative filtering | `GET /v1/series/{id}/readers-also-like`   | Series that readers of your favourites also enjoy           |

Codex sends a curated set of seed titles (your highest-rated series first, then your most recent
reads). `/mix` folds every seed's tag vector into a single probe, so the whole seed set costs one
request rather than one request per title, and results come back already diversified and annotated
with which seeds produced them.

Tag similarity alone tends to return more of what you already own, so `readers-also-like` supplies
the taste signal that content matching structurally cannot.

## Keeping franchises out of the list

Tag-vector similarity ranks a series' own spin-offs and per-arc volumes as its nearest neighbours,
because they genuinely are. Left alone, seeding with Re:Zero returns five Re:Zero chapter volumes
and nothing else. The plugin therefore:

- drops anything MangaBaka flags as related to a seed, or whose own relationship data names a seed,
  or whose title matches a seed's once normalised;
- collapses each remaining franchise to its single strongest entry, which catches franchises you
  hold no seed for;
- de-ranks (but keeps) series sharing an author with a seed, since an author's *unrelated* other
  work is often exactly what you want next.

This is deliberately aggressive. On a library dominated by one franchise you will get fewer
recommendations than you asked for rather than a list padded with spin-offs.

## Requirements

**Run a metadata match with the MangaBaka Metadata plugin first.** Seeds are resolved from the
`api:mangabaka` external IDs that `metadata-mangabaka` writes onto your series. Series without one
cannot be used as seeds, and a library with none will produce no recommendations at all.

There is deliberately no title-search fallback. With a combined probe vector, a single bad title
match degrades every result rather than just one, so unmatched series are skipped instead of
guessed at.

## Limitations

- **Manga, manhwa, and manhua only.** MangaBaka does not cover western comics or general ebooks.
  Enabling this on such a library will produce nothing useful.
- **The upstream recommendation endpoints are beta.** MangaBaka marks both `/v1/series/mix` and
  `/v1/series/{id}/readers-also-like` as `x-api-stability: beta`, meaning they may change or
  disappear without notice. Responses are parsed defensively so that an upstream change degrades
  recommendation quality rather than breaking the task, but breakage is a realistic possibility.
- **Recommendations are only as good as your metadata.** The more of your library MangaBaka can
  identify, the better the probe vector.

## Development

```bash
npm install
npm run build       # bundle to dist/index.js
npm test            # vitest
npm run lint        # biome
npm run typecheck   # tsc
```

## License

MIT
