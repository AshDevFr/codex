---
---

# Carrying Reading Progress Across a Library Split

Reorganising a library, whether splitting one root into several, moving files
to a new path, or moving your whole collection to a different Codex instance,
mints new series and book ids. Nothing about your reading history follows
automatically: `read_progress`, `read_completions`, `reading_sessions`, your
series ratings, and your want-to-read queue are all keyed on the ids the old
scan created.

`GET /api/v1/reading-progress/export` and `POST /api/v1/reading-progress/import`
exist to carry that state across the move. Export writes one JSON file for your
own reading state; import matches it back against whatever the library looks
like afterwards and writes the state onto the new ids.

:::danger Export before you delete anything
Take the export **before** you delete the old library or root. Sessions and
completions survive a hard-deleted book as orphaned rows (their `book_id` is
cleared, not the row), so a failed or late import is a retry, not a loss; but
nothing except an export taken beforehand can say **which book** an orphan used
to belong to. Delete first, and that attribution is gone permanently, even
though the row itself still exists.
:::

:::note Not the same as the instance-level backup
[`codex export` / `codex import`](./export-import-copy.md) move the *entire*
database between instances and require shell access to the server. This
feature moves *one user's own reading state* through the web API and is meant
for exactly this workflow: splitting or re-rooting a library without asking
every reader to re-mark their progress by hand.
:::

## The workflow

1. **Export.** From Settings → Reading Progress, download the export (or call
   `GET /api/v1/reading-progress/export` directly). This is a snapshot of your
   own `read_progress`, `read_completions`, `reading_sessions`, and series
   ratings, keyed by external ids, each series' path, and each book's
   filename and hash rather than by database id.
2. **Reorganise.** Split the library, move files, rescan under new roots,
   move to a new instance, whatever the move is. Delete the old
   library/root only after the export from step 1 is safely saved somewhere.
   On the same instance you can import before or after deleting it: a series
   whose files have all moved away is never chosen as a match, and history
   still sitting on the old, soft-deleted books is moved onto the new ones.
3. **Import.** Upload the file from Settings → Reading Progress (or call
   `POST /api/v1/reading-progress/import`). Run it once as a **dry run** first:
   it returns the identical report shape without writing anything, so you
   can check the match quality before committing.
4. **Apply.** Re-run with `dryRun: false` (the Settings page gates this
   behind a successful preview). Progress, completions, sessions, and ratings
   land on the new books and series.

Importing the same file again later is safe: `read_completions` and
`reading_sessions` keep their original ids, so a repeat import is a no-op
rather than a duplicate, and `read_progress` / ratings are upserted under
whichever conflict policy you choose.

## Matching

Each series in the file is resolved against the current library, in order,
stopping at the first step that finds anything:

1. An external id (a plugin match, ComicInfo, or a manual entry), tried in the
   order given by `sourcePreference`.
2. The series' path, relative to its library root.
3. The series' normalized name.

A series with no books left on disk (for example the old library's copy after
its files moved away) is never a candidate at any step.

Books are then resolved within that series, in order: their path relative to
the series folder, their file name, and finally their filename stem (which
survives a `.cbr` repacked to `.cbz`). A repack always changes the file's
hash, so the stem step never checks hashes. Under `hashMode: verify`, a path
or name match whose hash differs is reported as `hash_mismatch`; that includes
a file a tagging tool has rewritten since the export, so use `off` if you
re-tagged your collection between export and import.

When two entries in the file land on the same book (a file that moved inside
its series appears once under its old path and once under its new one), the
second is decided against the first under the conflict policy, exactly as if
the first were already in the database.

**Nothing is ever guessed.** If a step finds more than one candidate, that
series or book is reported as `ambiguous` and nothing is written for it. A
stem match is reported as `stem_match` and is only applied if you turn on
`acceptStemMatches`: two files can share a stem, so applying it silently
risks writing progress onto the wrong one.

A series or book you cannot see (denied by a sharing tag, or outside your
access groups) resolves as `unmatched`, the same as one that genuinely is not
in the library. It never surfaces as a permission error, which would confirm
that the content exists.

## Request flags

| Flag | Default | Effect |
|---|---|---|
| `dryRun` | `false` | Report the outcome without writing anything |
| `hashMode` | `verify` | `off` ignores hashes; `verify` rejects a path/name match whose `fileHash` disagrees; `match` additionally uses `fileHash`/`partialHash` to find a book when path and name both fail (rescues a bulk rename) |
| `sourcePreference` | every source in the file | External-id sources to try, in order, before falling back to path and name. Omit it to try each source the exported series carries, in document order. An explicit `[]` skips external ids entirely and goes straight to path matching |
| `conflictPolicy` | `newest` | How to resolve a book/rating that already has a value on this side: `newest` (later `updatedAt` wins), `furthest` (further into the book wins; a finished read always beats a partial one), `skip_existing`, or `overwrite`. A rating has no position, so `furthest` behaves like `newest` for ratings, and a file without a rating timestamp never replaces an existing rating except under `overwrite` |
| `reattachSessions` | `true` | When a session or completion in the file already exists as your own row but is not on a live book (its book was deleted, or the scanner marked it deleted after the file moved), move it onto the matched book instead of skipping it. A no-op, reported as such, when the file carries no sessions |
| `acceptStemMatches` | `false` | Apply a book match found only by filename stem |
| `restoreWantToRead` | `true` | Put queued series and books back into want-to-read. See [Want to read](#want-to-read) |
| `libraryIds` | all libraries | Which libraries a series may match into. Naming the target library is what lets an import run before the old library has been rescanned: otherwise both copies of a series are live, both match, and the series is reported `ambiguous` |

`GET /api/v1/reading-progress/export` takes two query parameters.

`includeSessions` (default `true`). Turn it off only if you specifically want
a smaller file: sessions are the only source of every reading statistic, so
leaving them out is easy to do by accident and easy not to notice until the
numbers are gone.

`libraryIds`, a comma-separated list (for example
`?libraryIds=<uuid>,<uuid>`). Omitted, the export carries every library you
have reading state for. Narrowing it to the library you are reorganising
keeps the file small and gives the import less to match against. A value that
is not a uuid is a `400` rather than a quietly wider export.

## When history is left behind

Reattachment moves a session or completion onto the matched book when its own
book is gone (`bookId` is null after a hard delete) or the scanner has marked
it deleted because the file moved. Both are the normal shapes of a library
split, so the usual sequence needs no special care: export, delete the old
library, import into the new one.

It does **not** move a row whose book is still live somewhere. Reattaching
then would strip reading history out of a library you may still be using, so
the import leaves it alone and counts it as `stranded`, both per book and as
`rowsStranded` in the summary, with a notice on the report.

This is worth watching for when you scope an import with `libraryIds` while
the old copy of a series is still on disk. The scope makes the series match
where it would otherwise be reported `ambiguous`, so the progress moves and
the history does not. The fix is the notice's advice: delete or rescan the
other library so its books are no longer live, then import again. The reused
row ids make that second import safe to run.

## Want to read

Your want-to-read queue travels with the export: a whole series you queued,
and a single book you queued on its own. A series you queued but never
started is included too, which matters, because that is the usual reason
anything is on the list, and it has no reading state to pull it in otherwise.

On import, restored entries go **after** anything already in your queue, in
the order they held on the old library. Anything already queued keeps its
place: nothing you arranged by hand is moved. Each entry keeps the date it was
first queued, so sorting the list by newest or oldest still means what it
did. Importing the same file twice adds nothing the second time.

Entries are restored only onto a series that matched, or a book whose match
was applied, so a stem match you did not accept restores nothing.

## The response

The response is the same shape whether or not `dryRun` is set: counts, plus a
per-series and per-book breakdown of what matched, what did not, and what was
(or would be) written. Each series is applied in its own transaction, so a bad
series does not cost every other series in the file its progress; the
per-series `committed` field says which ones actually landed.

### Reading the counts after a scoped import

When the import names `libraryIds`, most unmatched series are not a problem:
splitting a library exports the whole old library and imports it into one of
several new ones, so series belonging to the others are *meant* to miss. The
summary therefore also states coverage of the destination:

| Field | Meaning |
|---|---|
| `seriesInSelectedLibraries`, `booksInSelectedLibraries` | What the selected libraries hold (only what you can see). Absent when the import was not scoped |
| `seriesMatchedDistinct`, `booksMatchedDistinct` | How many of those received state. Distinct, because two series in the file can resolve to one here |
| `booksInUnmatchedSeries` | Books whose whole series did not match. After a scoped import, these belong to other libraries |

`booksUnmatched - booksInUnmatchedSeries` is the number worth looking at:
books missed inside a series that **did** match. The Settings page shows
these as "missed" and sets the rest aside, reading, for example, *32 of the
32 series in Shonen matched*.

Without `libraryIds` there is no destination to measure against, so these
fields are absent and an unmatched series means what it says.

Ratings must be between 1 and 100, the same range the rating endpoint
enforces; a file with any other value is rejected with a 400 naming the
series. Imports up to 64 MB are accepted, which is room for tens of thousands
of books with their sessions.

## Limitations

- Metadata, collections, read lists, and covers are not carried; this moves
  reading state and your want-to-read queue only. Read lists have their own
  ordering and collections can be rule-driven, so neither follows the series
  match cleanly. See [Data Exports](../exports) for a metadata export, and
  [`codex export`](./export-import-copy.md) for a full instance backup.
- A series with no external ids whose name and path both changed will not
  match. Give it an external id (or a manual one) before the move if you can.
- Cross-instance import depends on both instances recognizing the same
  external id sources.
