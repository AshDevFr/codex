---
sidebar_position: 1
---

# Scanning Strategies

Codex supports multiple scanning strategies configurable per-library, giving you flexibility in how your content is organized and detected. Unlike other media servers that enforce a single organizational pattern, Codex lets you choose the strategy that best fits each library's content.

## Overview

When creating a library, you configure two independent strategies:

1. **Series Strategy** - How to detect and group series from your folder structure
2. **Book Strategy** - How to determine book titles and extract volume/chapter numbers

:::warning Strategy Immutability
**Strategies cannot be changed after library creation.** If you need a different strategy, you must delete the library and recreate it. This prevents data integrity issues with read progress and book organization.
:::

## Quick Start

### Default (Komga-compatible)

If your content is organized with each series in its own folder:

```
/library/
  ├── Batman/
  │   ├── Batman #001.cbz
  │   └── Batman #002.cbz
  └── Spider-Man/
      └── Spider-Man #001.cbz
```

Use the defaults: `series_volume` + `filename`. This matches Komga's behavior.

### Chapter-based Manga

If you have volume subfolders containing chapters:

```
/library/
  └── One Piece/
      ├── Volume 01/
      │   ├── Chapter 001.cbz
      │   └── Chapter 002.cbz
      └── Volume 02/
          └── Chapter 003.cbz
```

Use: `series_volume_chapter` + `smart`

## Choosing the Right Strategy

```
How is your content organized?

├─ "Each series in its own folder with books inside"
│  └─ series_volume (default)
│
├─ "Series folders contain volume/chapter subfolders"
│  └─ series_volume_chapter
│
├─ "Everything in one big folder"
│  └─ flat
│
├─ "Organized by publisher, then series"
│  └─ publisher_hierarchy
│
├─ "Calibre library (Author/Book folders)"
│  └─ calibre
│
└─ "Custom structure"
   └─ custom (regex)
```

## Strategy Types

| Series Strategy | Best For |
|----------------|----------|
| [Series-Volume](./series-strategies#series-volume-default) | Western comics, simple folders |
| [Series-Volume-Chapter](./series-strategies#series-volume-chapter) | Manga with volume subfolders |
| [Flat](./series-strategies#flat-structure) | Single folder collections |
| [Publisher Hierarchy](./series-strategies#publisher-hierarchy) | Publisher/series organization |
| [Calibre](./series-strategies#calibre) | Calibre library imports |
| [Custom](./series-strategies#custom) | Regex-based detection |

| Book Strategy | Best For |
|--------------|----------|
| [Filename](./book-strategies#filename-default) | Predictable, Komga-compatible |
| [Metadata First](./book-strategies#metadata-first) | Rich metadata collections |
| [Smart](./book-strategies#smart) | Best of both worlds |
| [Series Name](./book-strategies#series-name) | Uniform generated titles |
| [Custom](./book-strategies#custom) | Regex-based extraction |

## In This Section

- [Series Strategies](./series-strategies) - How series are detected from folders
- [Book Strategies](./book-strategies) - How book titles and numbers are determined
- [Configuration Examples](./examples) - Ready-to-use configurations
