---
sidebar_position: 2
---

# Series Strategies

Series strategies determine how Codex detects and groups series from your folder structure.

## Series-Volume (Default)

**Rule:** Direct child folders of library = series, files in folders = books

**Best for:** Western comics, simple folder structures, Komga-style organization

```
/library/
  ├── Batman/                 ← Series: "Batman"
  │   ├── Batman #001.cbz     ← Book
  │   ├── Batman #002.cbz     ← Book
  │   └── Batman #003.cbz     ← Book
  ├── Spider-Man/             ← Series: "Spider-Man"
  │   └── Amazing Spider-Man #001.cbz
  └── One Piece/              ← Series: "One Piece"
      ├── One Piece v01.cbz
      └── One Piece v02.cbz
```

**How it works:**
- Scan depth: 1 level from library root
- Each folder containing files = 1 series
- Folder name = series name (overridable by metadata)

**Configuration:**
```json
{
  "series_strategy": "series_volume"
}
```

---

## Series-Volume-Chapter

**Rule:** Parent folder = series, child folders = volumes/arcs, files = chapters

**Best for:** Chapter-based manga, web comics, serialized content with volume organization

```
/library/
  └── One Piece/                    ← Series: "One Piece"
      ├── Volume 01/                ← Organizational container (not a series)
      │   ├── Chapter 001.cbz       ← Book in "One Piece"
      │   ├── Chapter 002.cbz       ← Book in "One Piece"
      │   └── Chapter 003.cbz       ← Book in "One Piece"
      ├── Volume 02/
      │   ├── Chapter 004.cbz
      │   └── Chapter 005.cbz
      └── Extras/
          └── Colorspread 01.cbz
```

**How it works:**
- Scan depth: 2 levels from library root
- Level 1 folders = series
- Level 2 folders = organizational containers (ignored for series detection)
- All files under a series folder = books in that series
- Volume folder name is stored as book metadata

**Configuration:**
```json
{
  "series_strategy": "series_volume_chapter"
}
```

---

## Flat Structure

**Rule:** All files at library root level, series detected from filename or metadata

**Best for:** Single large folder, metadata-rich collections, automated downloaders

```
/library/
  ├── [One Piece] v01.cbz          ← Series: "One Piece"
  ├── [One Piece] v02.cbz          ← Series: "One Piece"
  ├── [Naruto] Chapter 001.cbz     ← Series: "Naruto"
  ├── [Naruto] Chapter 002.cbz     ← Series: "Naruto"
  └── Bleach - Vol 01.cbz          ← Series: "Bleach"
```

**How it works:**
1. Parse series name from filename patterns:
   - `[Series Name] file.cbz`
   - `Series Name - file.cbz`
   - `Series Name_file.cbz`
2. Fall back to ComicInfo.xml `<Series>` field
3. Fall back to EPUB/PDF embedded metadata
4. Last resort: Create series from first word(s) of filename

**Configuration:**
```json
{
  "series_strategy": "flat",
  "series_config": {
    "filename_patterns": [
      "\\[([^\\]]+)\\]",
      "^([^-]+) -",
      "^([^_]+)_"
    ],
    "require_metadata": false
  }
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `filename_patterns` | Regex patterns to extract series name | Built-in patterns |
| `require_metadata` | Only use metadata, skip filename parsing | `false` |

---

## Publisher Hierarchy

**Rule:** Skip first N levels as organizational containers, then apply series-volume rules

**Best for:** Collections organized by publisher, imprint, or year

```
/library/
  ├── Marvel/                       ← Publisher (ignored)
  │   ├── Spider-Man/               ← Series: "Spider-Man"
  │   │   └── Amazing Spider-Man #001.cbz
  │   └── X-Men/                    ← Series: "X-Men"
  │       └── X-Men #001.cbz
  └── DC/                           ← Publisher (ignored)
      ├── Batman/                   ← Series: "Batman"
      │   └── Batman #001.cbz
      └── Superman/                 ← Series: "Superman"
          └── Action Comics #001.cbz
```

**How it works:**
- Skip first N levels (configurable)
- Apply series-volume rules at the series level
- Skipped folder names stored as metadata (e.g., publisher field)

**Configuration:**
```json
{
  "series_strategy": "publisher_hierarchy",
  "series_config": {
    "skip_depth": 1,
    "store_skipped_as": "publisher"
  }
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `skip_depth` | Number of levels to skip | `1` |
| `store_skipped_as` | Metadata field for skipped folder names | `"publisher"` |

---

## Calibre

**Rule:** Author folder → Book title folder (with optional ID suffix) → book files

**Best for:** Calibre library imports, ebook collections organized by author

```
/library/
  ├── George R. R. Martin/                    ← Author (stored as metadata)
  │   ├── A Clash of Kings (211)/             ← Book title folder
  │   │   ├── A Clash of Kings - George R. R. Martin.epub
  │   │   ├── cover.jpg
  │   │   └── metadata.opf
  │   └── A Game of Thrones (208)/
  │       └── A Game Of Thrones - George R. R. Martin.epub
  ├── Brandon Sanderson/
  │   ├── Mistborn (45)/
  │   │   └── Mistborn - Brandon Sanderson.epub
  │   └── The Well of Ascension (46)/
  │       └── The Well of Ascension - Brandon Sanderson.epub
  └── metadata.db                              ← Calibre database (ignored)
```

**How it works:**
- Scan depth: 2 levels from library root
- Level 1 folders = authors (stored as metadata)
- Level 2 folders = book titles (Calibre ID suffix stripped)
- Each book folder = 1 book
- Series detection from `metadata.opf` or embedded metadata

**Series grouping modes:**
- `standalone`: Each book is its own "series" of 1 (default)
- `by_author`: Group all books by same author into a series
- `from_metadata`: Use series field from OPF/embedded metadata

**Configuration:**
```json
{
  "series_strategy": "calibre",
  "series_config": {
    "strip_id_suffix": true,
    "series_mode": "from_metadata",
    "read_opf_metadata": true,
    "author_from_folder": true
  }
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `strip_id_suffix` | Remove ` (123)` from folder names | `true` |
| `series_mode` | How to group into series | `"standalone"` |
| `read_opf_metadata` | Parse metadata.opf files | `true` |
| `author_from_folder` | Use folder name as author | `true` |

---

## Custom

**Rule:** User-defined regex patterns for series detection

**Best for:** Unique organizational patterns, advanced users

**Configuration:**
```json
{
  "series_strategy": "custom",
  "series_config": {
    "pattern": "^(?P<publisher>[^/]+)/(?P<series>[^/]+)/(?P<book>.+)\\.(cbz|cbr|epub|pdf)$",
    "series_name_template": "{publisher} - {series}"
  }
}
```

**How it works:**
- Pattern matched against relative path from library root
- Named groups extract metadata
- Template constructs final series name from captured groups

**Named groups:**

| Group | Purpose | Required |
|-------|---------|----------|
| `(?P<series>...)` | Series name | Yes |
| `(?P<publisher>...)` | Publisher metadata | No |
| `(?P<book>...)` | Book filename portion | No |

**Example patterns:**

| Structure | Pattern |
|-----------|---------|
| `Publisher/Series/Book.cbz` | `^(?P<publisher>[^/]+)/(?P<series>[^/]+)/(?P<book>.+)\\.` |
| `Year/Series/Book.cbz` | `^(?P<year>\\d{4})/(?P<series>[^/]+)/` |
| `Genre/Publisher/Series/Book.cbz` | `^[^/]+/(?P<publisher>[^/]+)/(?P<series>[^/]+)/` |

:::caution
Custom patterns require regex knowledge. Test with Preview Scan before creating the library.
:::
