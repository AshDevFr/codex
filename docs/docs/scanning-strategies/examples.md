---
sidebar_position: 4
---

# Configuration Examples

Ready-to-use configurations for common use cases.

## Western Comics (Komga-compatible)

Simple folder structure where each series has its own folder.

```
/library/
  ├── Batman/
  │   ├── Batman #001.cbz
  │   └── Batman #002.cbz
  └── Spider-Man/
      └── Spider-Man #001.cbz
```

```json
{
  "series_strategy": "series_volume",
  "book_strategy": "filename"
}
```

This is the default configuration.

---

## Chapter-based Manga

Manga organized with volume subfolders containing individual chapters.

```
/library/
  └── One Piece/
      ├── Volume 01/
      │   ├── Chapter 001.cbz
      │   └── Chapter 002.cbz
      └── Volume 02/
          └── Chapter 003.cbz
```

```json
{
  "series_strategy": "series_volume_chapter",
  "book_strategy": "smart"
}
```

---

## Flat Collection with Bracket Naming

All files in one folder with series names in brackets.

```
/library/
  ├── [One Piece] v01.cbz
  ├── [One Piece] v02.cbz
  ├── [Naruto] Chapter 001.cbz
  └── [Bleach] Vol 01.cbz
```

```json
{
  "series_strategy": "flat",
  "series_config": {
    "filename_patterns": ["\\[([^\\]]+)\\]"],
    "require_metadata": false
  },
  "book_strategy": "filename"
}
```

---

## Flat Collection with Metadata

All files in one folder, relying on embedded metadata for series detection.

```
/library/
  ├── file1.cbz  (ComicInfo.xml has Series="One Piece")
  ├── file2.cbz  (ComicInfo.xml has Series="One Piece")
  └── file3.cbz  (ComicInfo.xml has Series="Naruto")
```

```json
{
  "series_strategy": "flat",
  "series_config": {
    "require_metadata": true
  },
  "book_strategy": "metadata_first"
}
```

---

## Publisher-organized Comics

Comics organized by publisher, then series.

```
/library/
  ├── Marvel/
  │   ├── Spider-Man/
  │   │   └── Amazing Spider-Man #001.cbz
  │   └── X-Men/
  │       └── X-Men #001.cbz
  └── DC/
      ├── Batman/
      │   └── Batman #001.cbz
      └── Superman/
          └── Action Comics #001.cbz
```

```json
{
  "series_strategy": "publisher_hierarchy",
  "series_config": {
    "skip_depth": 1,
    "store_skipped_as": "publisher"
  },
  "book_strategy": "filename"
}
```

---

## Publisher + Year Hierarchy

Comics organized by publisher, then year, then series.

```
/library/
  ├── Marvel/
  │   ├── 2023/
  │   │   └── Spider-Man/
  │   │       └── Spider-Man #001.cbz
  │   └── 2024/
  │       └── X-Men/
  │           └── X-Men #001.cbz
  └── DC/
      └── 2024/
          └── Batman/
              └── Batman #001.cbz
```

```json
{
  "series_strategy": "publisher_hierarchy",
  "series_config": {
    "skip_depth": 2,
    "store_skipped_as": "publisher"
  },
  "book_strategy": "filename"
}
```

---

## Calibre Ebook Library

Direct import from a Calibre library folder.

```
/library/
  ├── Brandon Sanderson/
  │   ├── Mistborn (45)/
  │   │   ├── Mistborn - Brandon Sanderson.epub
  │   │   └── metadata.opf
  │   └── The Well of Ascension (46)/
  │       └── The Well of Ascension - Brandon Sanderson.epub
  └── George R. R. Martin/
      └── A Game of Thrones (208)/
          └── A Game Of Thrones.epub
```

```json
{
  "series_strategy": "calibre",
  "series_config": {
    "strip_id_suffix": true,
    "series_mode": "from_metadata",
    "read_opf_metadata": true,
    "author_from_folder": true
  },
  "book_strategy": "metadata_first"
}
```

---

## Calibre with Author-based Series

Group all books by the same author into a series.

```json
{
  "series_strategy": "calibre",
  "series_config": {
    "strip_id_suffix": true,
    "series_mode": "by_author",
    "author_from_folder": true
  },
  "book_strategy": "metadata_first"
}
```

---

## Custom: Scanlation Group Format

Files with scanlation group tags.

```
/library/
  ├── [GroupName] One Piece v01 c001.cbz
  ├── [GroupName] One Piece v01 c002.cbz
  └── [GroupName] Naruto v01 c001.cbz
```

```json
{
  "series_strategy": "flat",
  "series_config": {
    "filename_patterns": ["\\] ([^v]+?) v"]
  },
  "book_strategy": "custom",
  "book_config": {
    "pattern": "\\] (?P<series>.+?) v(?P<volume>\\d+) c(?P<chapter>\\d+)",
    "title_template": "{series} v.{volume} c.{chapter}",
    "fallback": "filename"
  }
}
```

---

## Custom: TV-style Episode Numbering

Files with SxxExx or seasonXepisode format.

```
/library/
  ├── Series Name - S01E01 - Episode Title.cbz
  ├── Series Name - S01E02 - Another Title.cbz
  └── Series Name - S02E01 - Season Two.cbz
```

```json
{
  "series_strategy": "flat",
  "series_config": {
    "filename_patterns": ["^([^-]+) -"]
  },
  "book_strategy": "custom",
  "book_config": {
    "pattern": "^(?P<series>.+?) - S(?P<volume>\\d+)E(?P<chapter>\\d+) - (?P<title>.+)$",
    "title_template": "{title}",
    "fallback": "filename"
  }
}
```

---

## Creating via API

### Basic Creation

```bash
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Comics",
    "path": "/library/comics",
    "series_strategy": "series_volume",
    "book_strategy": "filename"
  }'
```

### With Configuration

```bash
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Calibre Library",
    "path": "/library/calibre",
    "series_strategy": "calibre",
    "series_config": {
      "strip_id_suffix": true,
      "series_mode": "from_metadata"
    },
    "book_strategy": "metadata_first"
  }'
```

### Preview Before Creating

```bash
curl -X POST http://localhost:8080/api/v1/libraries/preview-scan \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "/library/manga",
    "series_strategy": "series_volume_chapter"
  }'
```

Response:
```json
{
  "detected_series": [
    {
      "name": "One Piece",
      "path": "/library/manga/One Piece",
      "book_count": 150,
      "sample_books": ["Chapter 001.cbz", "Chapter 002.cbz"]
    }
  ]
}
```

---

## Troubleshooting

### Series Not Grouped Correctly

1. Use **Preview Scan** to test before creating
2. Verify folder structure matches the strategy
3. For flat strategy, check filename patterns

### Books Missing Numbers

1. Try `smart` or `metadata_first` book strategy
2. Add ComicInfo.xml to your files
3. Use `custom` book strategy with appropriate regex

### Want to Change Strategy

Strategies are immutable. To change:

1. Delete the library (files stay on disk)
2. Create new library with desired strategy
3. Run scan

:::note
Read progress is lost when deleting a library.
:::
