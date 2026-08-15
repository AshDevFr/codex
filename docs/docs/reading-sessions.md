---
---

# Reading Sessions (for client developers)

This page is the contract a reading client implements. If you are using Codex
rather than building a client for it, [Reading Progress](./reading-progress.md)
is the page you want.

Codex records reading as an append-only log of **sessions**. Your position and
your completion history are both derived from that log rather than stored
directly, which is what makes reading from several devices, some of them
offline, come out right.

## Why sessions instead of "set my position to page 40"

A bare position cannot be merged. If two clients both claim one, the server can
only pick a winner, and the obvious rule (whichever arrived last) is wrong:

```
iPad    read to page 12, finished 09:30, synced at 10:05
iPhone  read to page 40, finished 10:00, synced at 10:00
```

The iPad arrives last but read *earlier*. Taking the last arrival drags the
reader back 28 pages.

A session says when the reading happened, so the server can order by that
instead of by arrival. The iPhone's session is later in reading time and wins,
whichever order the two arrive in.

## Recording sessions

```
POST /api/v1/reading-sessions
```

Send up to **500** sessions per request; chunk anything larger. Sessions are
always recorded against the authenticated user.

```json
{
  "sessions": [
    {
      "id": "5e2c9c1a-....",
      "bookId": "9a11f3e2-....",
      "deviceId": "8f3d1c7a-....",
      "deviceName": "Ash's iPhone",
      "kind": "progress",
      "toPage": 42,
      "activeDurationMs": 934000,
      "pagesRead": 31,
      "clientStartedAt": "2026-08-14T19:02:11Z",
      "clientEndedAt": "2026-08-14T19:20:45Z"
    }
  ]
}
```

The response reports each entry's fate and returns current progress for every
book touched, so you can reconcile without a second request:

```json
{
  "accepted": ["5e2c9c1a-...."],
  "rejected": [{ "id": "1b7a....", "reason": "book_not_found" }],
  "progress": [{ "bookId": "9a11f3e2-....", "currentPage": 42, "completed": false }]
}
```

A bad entry never fails the batch. `book_not_found` in particular is routine:
it means the book was deleted while you were offline.

### `kind`

| Value | Meaning |
|---|---|
| `progress` | Reading happened. `toPage` or `toPercentage` is where it ended |
| `completed` | The book was finished |
| `reset` | The book was marked unread, starting a new read-through |

Use `reset` rather than deleting progress. Recording it as an event is what
lets the server order "I finished this" against "I am starting over" correctly
when both were made offline.

### `id` and replay

`id` is yours to generate. Submitting the same id twice changes nothing and
still reports as `accepted`, so if a response never arrives you can simply send
the batch again. Do not generate a fresh id on retry: that is the one thing
that will double-count a session.

### `clientEndedAt`

This orders sessions against each other, so it must be when the **reading**
happened, not when you are submitting. A session read on a plane and synced six
hours later still carries its original times.

The server stamps its own arrival time separately and uses it only to break
ties, so a device with a drifting clock cannot win ordering outright.

### Position

Position takes the value from whichever session is latest in client time. There
is no "furthest page wins" rule, deliberately: a reader who taps back from page
50 to 49 produces a later session with a lower position, and that is a real
instruction, not a stale write.

## Measuring reading time

`activeDurationMs` must be time the reader was **actually reading**. It is not
`clientEndedAt - clientStartedAt`; a book left open on the nightstand for three
hours is not three hours of reading.

Anything larger than the session's own span is clamped to that span. Omit the
field entirely if you cannot measure it honestly. Omitting it is recorded as
"unknown", which is correct; sending `0` would be a claim.

### The activity rules

Implement these exactly. If one client idles after 2 minutes and another after
5, aggregate reading time is a blend of two different metrics and means nothing.

| Rule | Value |
|---|---|
| Counts as activity | page turn, scroll of the reading surface, pinch or pan, TOC navigation, bookmark, scrub |
| Does not count | settings changes, cover browsing, anything outside a reader |
| Idle timeout | 5 minutes without activity pauses the timer |
| Pause triggers | app backgrounded, screen locked, tab hidden, reader dismissed |
| Checkpoint interval | every 30 seconds of accumulated *active* time |

### Surviving a crash

Do not rely on a clean session close. Checkpoint the running session to local
storage every 30 seconds of active time, and on startup look for an orphan left
by a previous run and close it out from its last checkpoint. Worst case you
lose one checkpoint interval instead of the whole session.

On the web, flush on `pagehide` via `navigator.sendBeacon`. `beforeunload` is
unreliable on mobile Safari.

## Keeping progress live while a sitting is open

A measured session only exists once the sitting ends, so if it were your only
write the stored position would not move until the reader closed the book. Keep
writing progress as you read:

```
PUT /api/v1/books/{id}/progress
X-Codex-Device-Id: <your deviceId>
```

**Send `X-Codex-Device-Id` on those writes, with the same value you put in
`deviceId`.** That header is what ties them to the session that will supersede
them. When your measured session arrives, the server absorbs the position-only
rows it covers from that device and deletes them, so one sitting leaves one row
rather than one per page turn.

Omit the header and progress still works exactly as before, but those writes
land on an anonymous device and stay there. Your reading then appears twice in
the statistics: once as your device's measured session, once as a scattering of
anonymous position writes.

The absorb step never moves a reader backwards. If a page turn landed after your
last measured position, the furthest position wins.

## Device identity

`deviceId` should be stable for the life of an install. It drives per-device
reading statistics and decides which sessions may merge with each other.

Clients using the OPDS, Komga-compatible or KOReader endpoints have no way to
send one. Give each such device **its own API key** and Codex will attribute
its reading to that key, which is worth doing for revocation anyway.

## Rejection reasons

| Reason | Cause |
|---|---|
| `book_not_found` | No such book, or not visible to you. Expected after a deletion |
| `invalid_time_range` | `clientEndedAt` precedes `clientStartedAt` |
| `invalid_percentage` | `toPercentage` outside 0.0 to 1.0 |
| `invalid_measurement` | `activeDurationMs` or `pagesRead` is negative |
| `duplicate_in_batch` | The same id appears twice in one request |
