# Contracts

Behavioural contracts that more than one Codex client has to implement identically.

Unlike `docs/api/openapi.json`, nothing here is generated from the server. These files describe
behaviour that lives in the *clients* but whose output lands in shared server data, so if two
clients implement it differently the data they produce silently stops meaning one thing.

Each file is hand-authored, versioned by a `version` field, and consumed as data by a test suite in
every repo that implements it. A change to the definition is therefore a reviewable diff, and a
client that has not adopted it fails its own suite.

| File | Implemented by | Consumed by |
| --- | --- | --- |
| [`reading-sessions.json`](./reading-sessions.json) | web reader, codex-reader-ios | `web/src/lib/reading/ReadingSessionTracker.test.ts` |

## `reading-sessions.json`

The measurement rules behind `POST /api/v1/reading-sessions`. Aggregate reading time is summed
across every device a reader uses, so "active time" has to mean the same thing on all of them. If
one client idles at two minutes and another at five, the total is a blend of two metrics and means
nothing.

### What it constrains, and what it does not

**Constrained, exactly:** the state machine and its arithmetic. Given the event sequence in a case,
with those timestamps, a conformant tracker produces exactly the sessions the case lists: the same
count, the same kinds, the same order, the same `activeDurationMs`, `pagesRead`, and position. The
same holds for what survives a crash.

**Not constrained:** which platform occurrence produces which event. The web reader decides that
`visibilitychange` means `pause`; the native client decides that a `scenePhase` transition does.
That mapping is each client's business, and a native client with real lifecycle callbacks is free to
be more precise about *when* it emits `pause` than the web reader can be. Also unconstrained: how a
checkpoint is persisted, and what an id looks like beyond being unique within a case.

That line is where the earlier open question lands. Pause and resume *are* in the contract, because
they are modelled as events rather than as platform occurrences. What a client detects, and how
promptly, is latitude. What it does once it has detected it is not.

### Schema

- `version` — bump on any change to a threshold, an event's meaning, or a case's expectation.
- `subject` — the book, device, and device name every case runs against, unless a case overrides
  `bookId`.
- `thresholds` — `idleTimeoutMs` and `checkpointIntervalMs`, carried here rather than assumed, so a
  change to the definition of "active" shows up in this file.
- `invariants` — properties asserted on every case in addition to its own expectations.
- `cases[]`:
  - `name`, `group`, optional `why`.
  - `bookId` — overrides `subject.bookId`.
  - `persistence: "unavailable"` — run this case with a store that cannot be written. How the runner
    arranges that is platform-specific.
  - `seedCheckpoints[]` — `{bookId, raw}` written into the store before the case runs, to model a
    checkpoint left by an older or broken write.
  - `events[]` — `atMs` is absolute from the start of the case, so a runner drives a fake clock to
    each timestamp in turn. Kinds: `start`, `activity` (both take optional `page` or `percentage`),
    `pause`, `resume`, `stop`, `complete` (optional position), `reset`, `checkpoint` (persist
    without closing), `crash` (the process dies with no chance to close), `recover` (run orphan
    recovery, appending its result to `recoveries`).
  - `expect`:
    - `sessions[]` — emitted sessions in order, in the wire shape of the endpoint. A field set to
      `null` must be **absent** from the payload. `spanMs`, where present, is
      `clientEndedAt - clientStartedAt`. Fields not listed are not asserted.
    - `recoveries[]` — one entry per `recover` event, each an array of recovered sessions.
    - `totalActiveDurationMs` — summed across `sessions`, where the point of the case is the total.
    - `tracking` — whether a session is still open at the end.
    - `checkpointedBookIds` — book ids with a checkpoint waiting, sorted.

### Adding a case

Add it here first, then make both suites pass. A case whose events duplicate an existing case
belongs as extra expectations on that case rather than as a new row.
