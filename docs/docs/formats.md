---
sidebar_position: 8
---

# Supported Formats

Codex supports multiple digital book and comic formats. This page details what's supported and how each format is handled.

## Comic Formats

### CBZ (Comic Book ZIP)

**Status:** ✅ Fully Supported

CBZ files are ZIP archives containing image files (typically JPG or PNG).

**Features:**

- Automatic page extraction
- Image metadata reading (dimensions, format)
- ComicInfo.xml metadata parsing
- Fast extraction (standard ZIP)

**Example Structure:**

```
comic.cbz
├── page001.jpg
├── page002.jpg
├── page003.png
└── ComicInfo.xml (optional)
```

### CBR (Comic Book RAR)

**Status:** ✅ Supported (with optional dependency)

CBR files are RAR archives containing image files.

**Features:**

- Same features as CBZ
- Requires UnRAR library (proprietary license)

**Building:**

- **With CBR:** `cargo build --release` (default)
- **Without CBR:** `cargo build --release --no-default-features`

**Note:** The UnRAR library uses a proprietary license. See [CBR Support and Licensing](../intro#cbr-support-and-licensing) for details.

## Ebook Formats

### EPUB

**Status:** ✅ Fully Supported

EPUB is the standard ebook format.

**Features:**

- Metadata extraction (title, author, publisher, etc.)
- Chapter/page structure parsing
- Cover image extraction
- Text content extraction (for search indexing - planned)

**Supported EPUB Versions:**

- EPUB 2.0
- EPUB 3.0

### PDF

**Status:** ✅ Supported

PDF files are supported for both comics and ebooks.

**Features:**

- Page extraction
- Metadata reading (title, author, subject)
- Page count detection
- Text extraction (for search - planned)

**Limitations:**

- Large PDFs may take longer to process
- Scanned PDFs (image-only) are supported but text search won't work

## Metadata Support

### ComicInfo.xml

ComicInfo.xml is a standard metadata format for comics, supported by many comic readers.

**Supported Fields:**

- Title, Series, Number, Count
- Writer, Penciller, Inker, Colorist
- Publisher, Imprint
- Publication Date
- Genre, Tags
- Summary/Description
- Cover Artist
- And many more...

**Location:**

- CBZ/CBR: Root of archive
- EPUB: Can be embedded in metadata

**Example:**

```xml
<?xml version="1.0"?>
<ComicInfo>
  <Title>Amazing Comic #1</Title>
  <Series>Amazing Comic</Series>
  <Number>1</Number>
  <Writer>John Doe</Writer>
  <Penciller>Jane Smith</Penciller>
  <Publisher>Comic Publisher</Publisher>
  <PublicationDate>2024-01-01</PublicationDate>
</ComicInfo>
```

## Image Formats

Within archives, Codex supports:

- **JPEG/JPG** - Most common for photos
- **PNG** - Lossless format, common for digital art
- **GIF** - Animated GIFs supported
- **WebP** - Modern format with good compression

## File Organization

### Recommended Structure

For best results, organize your files:

```
library/
├── Series Name/
│   ├── Series Name v01.cbz
│   ├── Series Name v02.cbz
│   └── Series Name v03.cbz
└── Another Series/
    └── ...
```

### Naming Conventions

Codex can extract series and volume information from filenames:

- `Series Name v01.cbz` → Series: "Series Name", Volume: 1
- `Series-Name-001.cbz` → Series: "Series Name", Number: 1
- `Series Name #001.cbz` → Series: "Series Name", Number: 1

Metadata in ComicInfo.xml takes precedence over filename parsing.

## Format Detection

Codex automatically detects file formats by:

1. **File extension** - Quick initial check
2. **File signature** - Magic bytes for accurate detection
3. **Content analysis** - Validates format structure

This ensures correct handling even with incorrect extensions.

## Performance Considerations

### Processing Speed

Fastest to slowest:

1. **CBZ** - Standard ZIP, very fast
2. **CBR** - RAR extraction, slightly slower
3. **EPUB** - XML parsing, moderate speed
4. **PDF** - Can be slow for large files

### Storage

- **CBZ/CBR**: Images stored in archive, extracted on-demand
- **EPUB**: Content extracted and cached
- **PDF**: Pages extracted and cached

### Memory Usage

- Small files (< 50MB): Minimal memory
- Medium files (50-200MB): Moderate memory
- Large files (> 200MB): Higher memory usage

Consider this when configuring your server resources.

## Future Format Support

Planned formats:

- **MOBI/AZW** - Kindle formats
- **CBT** - Comic Book TAR
- **CB7** - 7z-based comics
- **DJVU** - Document format

## Troubleshooting

### Format Not Recognized

1. Check file extension matches format
2. Verify file isn't corrupted
3. Check file signature with: `file filename.cbz`

### Metadata Not Extracted

1. Ensure ComicInfo.xml exists (for comics)
2. Check XML is well-formed
3. Verify metadata fields are correct

### Slow Processing

1. Large files take time - be patient
2. Check system resources (CPU, memory, disk I/O)
3. Consider processing during off-peak hours

## Next Steps

- Learn about [scanning strategies](./getting-started#scanning)
- Configure [library settings](./configuration)
- Explore [API endpoints](./api) for format information
