---
---

# Series Metadata

Codex stores rich metadata at the **series** level: title, summary, status, publisher, ratings, alternate titles, external IDs, and **expected total counts**. This guide focuses on the count fields, since they are the most commonly misunderstood, and explains the manual edit form, locks, and how counts interact with metadata refresh.

## Volumes vs. Chapters

Different libraries organize their content differently, and Codex models this directly with **two separate count fields** rather than a single ambiguous "book count":

| Field | Type | Description |
|-------|------|-------------|
| **Total Volumes** | integer | Expected number of volumes (e.g., a 14-volume manga). |
| **Total Chapters** | decimal | Expected number of chapters (e.g., 142.5). Fractional values are allowed for special chapters like 47.5 or 100.5. |

Both fields are optional and independent. A series can have:

- **Only a volume count**: typical for volume-organized libraries (one file = one volume). Example: a complete 14-volume manga shows `14 vol`.
- **Only a chapter count**: typical for chapter-organized libraries (one file = one chapter). Example: an ongoing series with no volume releases shows `142 ch`.
- **Both**: typical for mixed libraries that own bound volumes plus loose chapters that haven't been collected yet. Example: `14 vol · 142 ch`.
- **Neither**: for series where the expected total isn't known.

The series detail header shows whichever totals are populated:

- **Both totals known**: `109/14 vol · 142 ch`
- **Volume only**: `14/14 vol`
- **Chapter only**: `109/142 ch`
- **Neither**: just the local book count.

The number on the **left** of `/` is your local book count; the number on the **right** is the expected total from metadata.

:::tip Mixed libraries
If you keep volumes 1-14 plus loose chapters 126-142 in the same series folder, set Total Volumes to 14 and Total Chapters to 142. Codex will track and display both axes correctly.
:::

## Editing Counts Manually

Open the series detail page, click the metadata edit button, and find the **Total Volumes** and **Total Chapters** inputs:

- **Total Volumes**: integer input. Leave empty if unknown.
- **Total Chapters**: decimal input. Accepts values like `142.5`. Leave empty if unknown.

Each field has its own **lock toggle** next to it. Locking a field tells metadata refresh to skip it; the value will not be overwritten by plugins.

## Locks and Metadata Refresh

When a metadata refresh runs, plugins propose values for both count fields. Codex applies them only if:

1. The plugin has the corresponding write permission (`metadata:write:total_volume_count` or `metadata:write:total_chapter_count`).
2. The corresponding lock is **off**.
3. The plugin actually returned a value for that field.

This means you can:

- **Lock the volume count** on a finished series whose volumes won't change, while letting the chapter count refresh as new chapters publish.
- **Lock the chapter count** if a provider's chapter total is wrong and you've manually corrected it, while still receiving volume updates.
- **Lock both** to freeze the displayed totals entirely.

## What Plugins Provide

Codex's first-party plugins populate both counts where the upstream API exposes them:

- **MangaBaka**: populates both Total Volumes and Total Chapters when the series page lists them. Also populates the search-result format badge (Manga / Novel / Manhwa / etc.) so visually-identical results are distinguishable in the metadata search modal.
- **AniList**: populates both Total Volumes and Total Chapters from AniList GraphQL.
- **Open Library**: populates Total Volumes only (Open Library does not expose chapter counts).

Other metadata-source plugins follow the same shape; see the [Plugin Author Guide](dev/plugins/writing-plugins.md) for what to populate.

## Migration from `totalBookCount`

Codex used to expose a single `totalBookCount` field. It is now replaced by `totalVolumeCount` and `totalChapterCount` (separate fields, separate locks).

On upgrade:

- Existing values were migrated to **Total Volumes** (since most pre-existing data was volume-shaped in practice).
- Existing locks were transferred to the **Total Volumes** lock.
- **Total Chapters** starts empty for every series. Run a metadata refresh against MangaBaka or AniList to populate it.
- The legacy `totalBookCount` field is removed from the API and template context. References in custom-metadata templates and preprocessing rules must be updated to `totalVolumeCount` and/or `totalChapterCount`.

:::caution Komga API compatibility
The Komga compatibility layer (`/{prefix}/api/v1/series/...`) continues to expose `totalBookCount` on the wire so Komga clients (e.g. Komic for iOS) keep working. The value sent over the wire comes from `totalVolumeCount` (the closest semantic match to Komga's existing field). Chapter counts are not exposed via the Komga compatibility layer because Komga itself has no equivalent field.
:::

## Other Series Metadata

Beyond counts, series carry standard metadata fields: title, summary, status (`ongoing`, `ended`, `hiatus`, `abandoned`, `unknown`), publication year, language, reading direction, genres, tags, authors, publisher, age rating, ratings, alternate titles, and external links. All of these are editable in the same modal and support per-field locks via the same lock toggle pattern.
