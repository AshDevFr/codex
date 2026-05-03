---
sidebar_position: 3
---

# Book Strategies

Book strategies determine how Codex names individual books and extracts volume/chapter numbers.

## Filename (Default)

**Rule:** Book title = filename without extension. **Volume** and **chapter** are extracted from canonical filename markers (`v01`, `c042`, etc.); ComicInfo is ignored.

This is the Komga-compatible default behavior.

```
File: Batman #003.cbz
Title: "Batman #003"

File: One Piece v01.cbz
Title: "One Piece v01"   →   volume: 1

File: One Piece c042.cbz
Title: "One Piece c042"  →   chapter: 42

File: One Piece v15 - c126 (2023).cbz
Title: "One Piece v15 - c126 (2023)"  →  volume: 15, chapter: 126
```

See [Volume and Chapter Numbers](../book-metadata#volume-and-chapter-numbers) for the full filename-pattern reference.

**Pros:**
- Predictable: what you see on disk = what you see in UI
- Komga-compatible
- No surprises from bad metadata

**Cons:**
- Ignores potentially good metadata
- Users must rename files to change display

**Configuration:**
```json
{
  "book_strategy": "filename"
}
```

---

## Metadata First

**Rule:** Use ComicInfo `<Title>` if present, fallback to filename. For **volume** and **chapter**, ComicInfo is the only source; the filename is never parsed for these fields.

```
File: +Anima #03.cbz
ComicInfo.xml: <Title>Cooro's Journey</Title>
Title: "Cooro's Journey"

File: Batman #001.cbz
ComicInfo.xml: (no title)
Title: "Batman #001"
```

**Pros:**
- Uses rich metadata when available
- Supports chapter titles and issue names

**Cons:**
- Unreliable metadata leads to poor display
- Inconsistent across different metadata sources

**Configuration:**
```json
{
  "book_strategy": "metadata_first"
}
```

---

## Smart

**Rule:** Use metadata only if it's meaningful, otherwise use filename. For **volume** and **chapter**, ComicInfo is consulted first; if it doesn't carry the field, the canonical filename markers are used as fallback.

```
File: +Anima #03.cbz
ComicInfo.xml: <Title>Vol. 3</Title>
Title: "+Anima #03" (rejected "Vol. 3" as generic)

File: +Anima #03.cbz
ComicInfo.xml: <Title>Cooro's Journey</Title>
Title: "Cooro's Journey" (meaningful title used)
```

**Generic patterns that are rejected:**
- `Vol. 3`, `Volume 1`
- `Chapter 5`, `Issue #3`
- `#3`, `3`

**Pros:**
- Best of both worlds
- Uses good metadata, ignores bad

**Cons:**
- More complex logic
- Edge cases possible

**Configuration:**
```json
{
  "book_strategy": "smart",
  "book_config": {
    "generic_patterns": ["^Vol\\.?\\s*\\d+$", "^Volume\\s*\\d+$"]
  }
}
```

---

## Series Name

**Rule:** Generate title from series name + position with automatic padding

**Format depends on series strategy:**
- `series_volume`: `{series} v.{volume_number}`
- `series_volume_chapter`: `{series} v.{volume_number} c.{chapter_number}`

```
/One Piece/
  ├── random_file_a.cbz  → "One Piece v.01"
  ├── random_file_b.cbz  → "One Piece v.02"
  └── ...                → "One Piece v.45"
```

**With series_volume_chapter:**
```
/One Piece/
  ├── Volume 01/
  │   ├── ch001.cbz  → "One Piece v.01 c.001"
  │   └── ch002.cbz  → "One Piece v.01 c.002"
  └── Volume 12/
      └── ch150.cbz  → "One Piece v.12 c.150"
```

**Padding scales with book count:**
- Volumes: 2 digits for 0-99, 3 for 100-999, 4 for 1000+
- Chapters: 3 digits for 0-999, 4 for 1000+

**Pros:**
- Clean, uniform naming across library
- Independent of messy filenames
- Predictable sort order

**Cons:**
- Loses original filename information
- Requires accurate series detection

**Configuration:**
```json
{
  "book_strategy": "series_name"
}
```

---

## Custom

**Rule:** User-defined regex patterns for title/volume/chapter extraction

**Best for:** Non-standard naming conventions, advanced users

**Example filenames and extractions:**
```
One_Piece_v012_c145.cbz  → volume: 12, chapter: 145, title: "One Piece v.12 c.145"
OP - 012x145 - Romance Dawn.cbz  → volume: 12, chapter: 145, title: "Romance Dawn"
Series [V01] [C003].cbz  → volume: 1, chapter: 3, title: "Series v.01 c.003"
```

**Configuration:**
```json
{
  "book_strategy": "custom",
  "book_config": {
    "pattern": "(?P<series>.+?)_v(?P<volume>\\d+)_c(?P<chapter>\\d+)",
    "title_template": "{series} v.{volume} c.{chapter}",
    "fallback": "filename"
  }
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `pattern` | Regex with named capture groups | Required |
| `title_template` | How to construct display title | Uses `{title}` group or filename |
| `fallback` | Strategy if pattern doesn't match | `"filename"` |

**Named groups:**

| Group | Purpose | Example |
|-------|---------|---------|
| `(?P<volume>...)` | Extract volume number (integer; persisted as the book's `volume` field) | `v(?P<volume>\d+)` matches "v12" → 12 |
| `(?P<chapter>...)` | Extract chapter number (decimal; persisted as the book's `chapter` field) | `c(?P<chapter>\d+(?:\.\d+)?)` matches "c145.5" → 145.5 |
| `(?P<title>...)` | Extract title portion | `- (?P<title>.+)$` matches "- Romance Dawn" |
| `(?P<series>...)` | Extract series (for template) | `^(?P<series>.+?)_` |

The captured `volume` and `chapter` values populate the per-book metadata fields and feed the series-level `local_max_volume` / `local_max_chapter` aggregates and "behind by N" comparisons. See [Volume and Chapter Numbers](../book-metadata#volume-and-chapter-numbers) for the canonical patterns the default `Filename` and `Smart` strategies recognize without custom regex.

**Common patterns:**

| Naming Convention | Pattern |
|-------------------|---------|
| `Series_v01_c001.cbz` | `(?P<series>.+?)_v(?P<volume>\d+)_c(?P<chapter>\d+)` |
| `Series - 01x05 - Title.cbz` | `^(?P<series>.+?) - (?P<volume>\d+)x(?P<chapter>\d+) - (?P<title>.+)$` |
| `[Group] Series v01 c001.cbz` | `\] (?P<series>.+?) v(?P<volume>\d+) c(?P<chapter>\d+)` |
| `Series Vol.1 Ch.5.cbz` | `(?P<series>.+?) Vol\.(?P<volume>\d+) Ch\.(?P<chapter>\d+)` |

**Template placeholders:**
- `{series}` - Captured series name
- `{volume}` - Volume number (auto-padded)
- `{chapter}` - Chapter number (auto-padded)
- `{title}` - Captured title
- `{filename}` - Original filename

:::caution
Test custom patterns with Preview Scan before creating the library.
:::
