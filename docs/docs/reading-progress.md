---
---

# Reading Progress & Re-read History

Codex tracks two separate things about what you read, and it helps to know which
is which:

| | Holds | Changed by |
|---|---|---|
| **Reading progress** | where you are *right now* in a book | reading, marking read/unread |
| **Read history** | every time you have *finished* a book | finishing a book, or clearing history |

Progress is current state. History is a permanent log. Marking a book unread
resets the first and leaves the second alone, which is what lets you re-read a
favourite series without pretending you never read it.

## Reading progress

Your position is saved as you read, and syncs across devices and clients:

- The **Continue** button on a book, and the **Keep Reading** and **On Deck**
  rows on the home page, all come from this.
- A book counts as **read** when you reach the last page, or when you use
  **Mark as Read**.
- **Mark as Unread** discards your position entirely, putting the book back to
  never-opened.

Progress arriving from a Komga-compatible app, a KOReader device or an external
tracker sync is treated exactly the same as progress from the built-in reader.

## Read history

Every time you finish a book, Codex records the completion: when that pass
started and when it finished. Book and series pages show the result:

> You've finished this book 2 times, last on 5 Mar 2025

Expand the section to see each read-through with its dates. The section is hidden
entirely on anything you have never finished, so it only appears once there is
something to show.

### What counts as finishing again

A new entry is recorded when you finish a book **after marking it unread**.
Marking unread is the signal that a new read-through has started.

That distinction matters because of what it excludes:

- **Tapping back a page and forward again does not count twice.** Turning back
  from the last page technically un-finishes the book, and turning forward
  finishes it again, but it is obviously the same read-through and Codex treats
  it as one.
- **Re-sending "completed" does not count twice.** Some clients re-assert the
  read state on every sync; that never inflates the count.
- **Re-reading without marking unread is not counted.** If you scroll back to
  page 1 while your progress is still there, Codex cannot tell that apart from
  browsing backwards through a long volume, so it stays one read-through. Mark
  the book unread first if you want the re-read recorded.

### Series history

A series counts as read once **every** book in it has been read. So the series
count is the *lowest* count across its volumes, not the highest:

- Read all six volumes: the series has been read once.
- Re-read volume 1 only: still once, because volumes 2-6 have only had one pass.
- Re-read all six: twice.

Each series entry spans from the earliest volume start of that pass to the last
volume finish, so it describes when you were reading the series as a whole.

Adding a **new volume** to a series you had finished drops the series count to
zero, because the series is no longer fully read. That is intended, and the
earlier full-series completions stay listed on the page so the history does not
appear to have vanished.

## Clearing history

History can be cleared at three scopes. **None of them touch your reading
progress**: books stay read or unread exactly as they are.

| Scope | Where | Clears |
|-------|-------|--------|
| One book | the history section on a book page | that book's completions |
| One series | the history section on a series page | every book in that series |
| Everything | **Settings → Profile → Account → Reading History** | your whole library |

Each asks for confirmation first. Clearing is per-user: it never affects anyone
else's history, and no user can see or clear another's.

## What is not recorded

- **Abandoned reads leave no trace.** Marking a book unread at page 50 without
  finishing records nothing. A completion log should not contain "gave up at
  page 50".
- **Nothing appears on library grid cards.** Re-read counts show on book and
  series detail pages only, so browsing a library does not have to compute them
  for every card.
- **Read lists have no completion count.** History is tracked for books, and
  rolled up to series.

## For existing libraries

Books you had already finished before this feature existed are counted: the
first upgrade records one completion for every book currently marked read, dated
from its completion time where that was stored and from its last-updated time
otherwise. You do not need to re-read anything to get a starting count.
