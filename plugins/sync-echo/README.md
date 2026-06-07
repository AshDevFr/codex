# @ashdev/codex-plugin-sync-echo

A minimal test/debug **sync** plugin for the Codex plugin system. It talks to no
external service: it accepts any push, returns deterministic reading entries on
pull, and records every request and response to files for inspection.

## Purpose

1. **Protocol validation** - a reference implementation of the sync provider
   protocol (`getUserInfo`, `pushProgress`, `pullProgress`, `status`).
2. **Debugging** - declares `wantsDetailedProgress`, so it receives the per-book
   detailed progress payload, and writes every request/response to JSON files so
   you can see exactly what the host sends without trawling server logs.

## Behavior

- **`sync/getUserInfo`** - returns a fixed fake identity (`echo_user`).
- **`sync/pushProgress`** - logs the inbound entries and echoes them all back as
  successes, alternating `created` / `updated`; nothing is ever rejected.
- **`sync/pullProgress`** - returns `pullCount` deterministic entries with every
  `SyncEntry` / `SyncProgress` field populated (status, chapters/volumes, pages,
  `maxVolume`/`maxChapter`, per-book `readBooks`, score, dates, notes). Respects
  the request `limit` (capped at `pullCount`).
- **`sync/status`** - returns canned counts.

## Payload recording

When `recordPayloads` is enabled (default), each call writes two JSON files to
`{dataDir}/payloads/` (the plugin's host-provided data directory):

```
yyyy-MM-dd-HH-mm-ss-{id}-{method}-request.json
yyyy-MM-dd-HH-mm-ss-{id}-{method}-response.json
```

Timestamps are UTC, so the files sort chronologically. Each file is a JSON
envelope holding the payload plus a snapshot of the active config. **Credentials
are never written**; secret-like config keys are redacted. The number of files
is bounded by `maxPayloadFiles` (oldest pruned). If no data directory is
available, files fall back to the OS temp dir.

## Configuration

| Key               | Default | Description                                                        |
| ----------------- | ------- | ------------------------------------------------------------------ |
| `pullCount`       | `3`     | How many deterministic entries `pullProgress` returns (1-50).      |
| `recordPayloads`  | `true`  | Write request/response files for debugging.                        |
| `maxPayloadFiles` | `500`   | Maximum recorded files to keep; oldest are pruned.                 |

## Development

```bash
npm install
npm run build       # bundle to dist/index.js
npm run typecheck
npm run lint
npm test
```

## License

MIT
