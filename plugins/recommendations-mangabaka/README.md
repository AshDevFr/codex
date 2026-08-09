# MangaBaka Recommendations Plugin

Manga recommendations for [Codex](https://github.com/AshDevFr/codex), powered by
[MangaBaka](https://mangabaka.org). **No account, no OAuth, and no API key required.**

> **Status: in development.** The client and manifest are in place; recommendation generation is
> not implemented yet.

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
