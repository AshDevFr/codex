---
---

# AniList Sync Plugin

The AniList sync plugin synchronizes manga reading progress between Codex and [AniList](https://anilist.co). It supports bidirectional sync of reading status, progress counts, scores, and dates.

## Features

- **Push** reading progress from Codex to AniList
- **Pull** reading progress from AniList to Codex
- Configurable sync direction (pull only, push only, or both)
- External ID matching via AniList media IDs (`api:anilist`)
- Highest-progress-wins conflict resolution

## Setup

### For Users

**With OAuth (if configured by admin):**

1. Go to **Settings** > **Integrations**
2. Click **Connect with AniList Sync**
3. Authorize Codex on AniList
4. You're connected!

**With Personal Access Token:**

1. Go to [AniList Developer Settings](https://anilist.co/settings/developer)
2. Create a new client with redirect URL `https://anilist.co/api/v2/oauth/pin`
3. Authorize your client and copy the token
4. In Codex, go to **Settings** > **Integrations** and paste the token

### For Admins

1. Go to **Settings** > **Plugins** > **Add Plugin**
2. Set command to `npx` with arguments `-y @ashdev/codex-plugin-sync-anilist`
3. Optionally configure OAuth (see the [plugin README](https://github.com/your-repo/plugins/sync-anilist) for details)

## How Sync Works

### Sync Modes

Configure the sync direction in **Settings** > **Integrations** > **Settings**:

| Mode                      | Description                                 |
| ------------------------- | ------------------------------------------- |
| **Pull & Push** (default) | Import from AniList, then export to AniList |
| **Pull Only**             | Import from AniList without writing back    |
| **Push Only**             | Export to AniList without importing         |

### Sync Flow

When a sync runs in **Pull & Push** mode, it executes two phases in order:

```
┌─────────────────────────────────────────────────────┐
│                    Sync Cycle                       │
├──────────────────────┬──────────────────────────────┤
│ Phase 1: Pull        │ Phase 2: Push                │
│                      │                              │
│ 1. Fetch AniList     │ 1. Read Codex progress       │
│    reading list      │ 2. Match series with         │
│ 2. Match entries to  │    external IDs              │
│    Codex series via  │ 3. Build push entries        │
│    external IDs      │ 4. Send to AniList           │
│ 3. Mark matched      │    (overwrites remote)       │
│    books as read     │                              │
│    (additive only)   │                              │
└──────────────────────┴──────────────────────────────┘
```

### External ID Matching

The plugin matches Codex series to AniList entries using external IDs stored in the `series_external_ids` table with source `api:anilist`. These IDs correspond to AniList media IDs.

Series without an AniList external ID are skipped during both pull and push.

## Conflict Resolution

Codex uses a **highest progress wins** strategy. There is no manual conflict resolution — progress can only move forward.

### How Conflicts Are Resolved

| Scenario                                                     | What happens                                                                                                 |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| **AniList ahead** — e.g., 5 vols read on AniList, 3 in Codex | Pull marks books 4–5 as read in Codex. Push sends 5 to AniList. Both agree.                                  |
| **Codex ahead** — e.g., 5 vols read in Codex, 3 on AniList   | Pull tries to mark first 3 as read — already completed, skipped. Push sends 5 to AniList. **Codex wins.**    |
| **Both changed** — different series or different progress    | Pull applies remote progress (additive), then Push sends local state. **Codex wins** because push runs last. |

### Key Behaviors

- **Pull is additive only** — it marks unread books as read, but never un-reads a book. If you lower your progress on AniList, that change is ignored in Codex.
- **Push overwrites the remote** — after pulling, Codex sends its current state to AniList. This may overwrite changes made directly on AniList.
- **Progress is monotonic** — once a book is marked as read (either locally or via pull), sync will not undo it. Progress only moves forward.

:::tip
If you want to manually manage your AniList list without Codex overwriting it, use **Pull Only** mode. If you only want to track progress from Codex to AniList without importing, use **Push Only** mode.
:::

## Completed Status

The plugin is conservative about marking series as "Completed" on AniList:

- A series is pushed as **Completed** only when **all** local books are read **and** the series metadata includes a `total_volume_count` (or `total_chapter_count`, depending on the configured Progress Unit) that matches the local count.
- Otherwise, the series is always pushed as **Reading** — even if all local books are read — because Codex can't be certain the library contains the full series.

This prevents incorrectly marking a series as finished when you may simply not have all volumes in your library yet.

## Sync Settings

Sync settings are split into two categories: **Codex Sync Settings** (shared across all sync plugins) and **Plugin-Specific Settings** (AniList-only).

### Codex Sync Settings

These settings control which entries Codex sends to the plugin. They apply to all sync plugins, not just AniList. Configure them in **Settings** > **Integrations** > **Sync Settings**:

| Option                         | Default | Description                                                                              |
| ------------------------------ | ------- | ---------------------------------------------------------------------------------------- |
| **Include Completed Series**   | On      | Include series where all local books are marked as read                                  |
| **Include In-Progress Series** | On      | Include series where at least one book has been started                                  |
| **Count Partially-Read Books** | Off     | Whether partially-read books (started but not finished) count toward the progress number |
| **Sync Ratings**               | On      | Include scores and notes in push/pull operations                                         |

These settings are stored in the user plugin config under the `_codex` namespace (e.g., `_codex.includeCompleted`). The server reads them to filter which entries to build and send — this is the server's only role. The plugin never reads these settings.

### Plugin-Specific Settings

These settings are specific to the AniList plugin and control how it interprets the data from Codex. Configure them in **Settings** > **Integrations** > **Plugin Settings**:

| Option               | Default  | Description                                                                   |
| -------------------- | -------- | ----------------------------------------------------------------------------- |
| **Progress Unit**    | Volumes  | Whether each Codex book counts as a "volume" or "chapter" on AniList          |
| **Pause After Days** | Disabled | Mark series as "Paused" on AniList if no reading progress for this many days  |
| **Drop After Days**  | Disabled | Mark series as "Dropped" on AniList if no reading progress for this many days |

### Progress Unit: Volumes vs Chapters

AniList tracks both volume and chapter progress separately. Codex always sends books-read as `volumes` in the sync data. The plugin then maps this to the correct AniList field based on your `progressUnit` setting:

- **Volumes** (default) — sends progress as `progressVolumes`, which is the natural mapping for manga volumes. AniList displays this as "Read Vol. X".
- **Chapters** — sends progress as `progress` (chapter count). Only use this if your Codex books represent individual chapters rather than collected volumes. AniList displays this as "Read Ch. X".

:::warning
Using "chapters" when your books are volumes can create misleading activity on your AniList profile (e.g., showing "Read chapter 3" when you actually read volume 3, which may contain chapters 20–30).
:::

### Staleness Detection

When `pauseAfterDays` or `dropAfterDays` is configured, the plugin checks each entry's `latestUpdatedAt` timestamp (the most recent reading progress update in Codex). If the elapsed time exceeds the threshold, the plugin overrides the entry's status before pushing to AniList:

- **Drop takes priority** — if both thresholds are met, the entry is marked as "Dropped"
- Only applies during push — staleness is not checked during pull

## Sync Results

After each sync, you can see a summary in **Settings** > **Integrations**:

| Field             | Meaning                                                    |
| ----------------- | ---------------------------------------------------------- |
| **Pulled**        | Entries fetched from AniList                               |
| **Matched**       | Pulled entries that matched a Codex series via external ID |
| **Applied**       | Books newly marked as read from pulled data                |
| **Pushed**        | Entries sent to AniList                                    |
| **Push Failures** | Entries that failed to push (e.g., invalid media ID)       |

## Next Steps

- [Plugins Overview](./index.md) — Managing plugins in Codex
- [Book Metadata](../book-metadata) — Book types and metadata fields
- [Libraries](../libraries) — Library setup and scanning
