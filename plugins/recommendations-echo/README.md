# @ashdev/codex-plugin-recommendations-echo

A minimal test/debug **recommendations** plugin for the Codex plugin system. It
talks to no external service: it echoes your library seeds back as
recommendations and records every request and response to files for inspection.

## Purpose

1. **Protocol validation** - a reference implementation of the recommendation
   provider protocol (`get`, `updateProfile`, `clear`, `dismiss`).
2. **Debugging** - writes every request/response to JSON files so you can see
   exactly what the host sends (the library seeds, limit, excluded IDs) without
   trawling server logs.

## Behavior

- **`recommendations/get`** - returns one fully-populated recommendation per
  library seed, echoing the seed title into `reason` / `basedOn`. When the
  library is empty, returns a few generic recommendations so the result is never
  empty. Respects the request `limit` and skips any `excludeIds`.
- **`recommendations/updateProfile`** - reports the number of entries processed.
- **`recommendations/dismiss`** - acknowledges the dismissal.
- **`recommendations/clear`** - acknowledges the clear.

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

| Key               | Default | Description                                        |
| ----------------- | ------- | -------------------------------------------------- |
| `recordPayloads`  | `true`  | Write request/response files for debugging.        |
| `maxPayloadFiles` | `500`   | Maximum recorded files to keep; oldest are pruned. |

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
