---
---

# Supported Formats

Codex supports multiple digital book and comic formats. This guide details each format's capabilities, metadata support, and best practices.

## Comic Formats

### CBZ (Comic Book ZIP)

**Status**: Fully Supported

CBZ files are ZIP archives containing image files, the standard format for digital comics.

| Feature | Support |
|---------|---------|
| Page extraction | Full |
| Metadata (ComicInfo.xml) | Full |
| Cover detection | Automatic |
| Processing speed | Fast |

**Structure:**

```
comic.cbz (ZIP archive)
├── page001.jpg
├── page002.jpg
├── page003.png
├── ...
└── ComicInfo.xml (optional)
```

**Best Practices:**
- Use sequential page numbering
- Include ComicInfo.xml for rich metadata
- Use JPEG for photos, PNG for art with transparency
- Keep file sizes reasonable (< 500MB recommended)

### CBR (Comic Book RAR)

**Status**: Supported (Optional Feature)

CBR files are RAR archives containing image files.

| Feature | Support |
|---------|---------|
| Page extraction | Full |
| Metadata (ComicInfo.xml) | Full |
| Cover detection | Automatic |
| Processing speed | Moderate |

:::note CBR Licensing
CBR support requires the UnRAR library, which uses a **proprietary license**. Pre-built binaries include CBR support. To build without it:

```bash
cargo build --release --no-default-features
```
:::

**Why CBZ over CBR:**
- CBZ uses open ZIP format
- Faster extraction
- No proprietary dependencies
- Better tool support

## Ebook Formats

### EPUB

**Status**: Fully Supported

EPUB is the standard ebook format, supported in versions 2.0 and 3.0.

| Feature | Support |
|---------|---------|
| Metadata extraction | Full |
| Cover extraction | Automatic |
| Chapter structure | Partial |
| Text content | Preserved |

**Supported Metadata:**
- Title
- Author(s)
- Publisher
- Publication date
- ISBN
- Description/Summary
- Series information
- Cover image

**EPUB Structure:**

```
book.epub (ZIP archive)
├── META-INF/
│   └── container.xml
├── OEBPS/
│   ├── content.opf (metadata)
│   ├── toc.ncx (table of contents)
│   ├── cover.jpg
│   ├── chapter1.xhtml
│   └── ...
└── mimetype
```

### PDF

**Status**: Supported

PDF files are supported for both comics and ebooks.

| Feature | Support |
|---------|---------|
| Page count detection | Full |
| Metadata extraction | Full |
| Cover extraction | Automatic (first page) |
| Page rendering | Full |

**Supported Metadata:**
- Title
- Author
- Subject
- Creator (application)
- Creation date
- Page count

**Limitations:**
- Large PDFs may process slowly
- Scanned PDFs (image-only) work but lack text search
- Memory usage scales with file size

## Image Formats

Within archives, Codex supports these image formats:

| Format | Extension | Best For |
|--------|-----------|----------|
| JPEG | `.jpg`, `.jpeg` | Photos, color comics |
| PNG | `.png` | Art with transparency, line art |
| WebP | `.webp` | Modern compression |
| GIF | `.gif` | Simple graphics |

### Image Handling

- **Automatic orientation**: EXIF data respected
- **Color profiles**: sRGB conversion for consistency
- **Thumbnails**: Generated for fast previews
- **On-demand resizing**: Reduces bandwidth

## Metadata Support

### ComicInfo.xml (Comics)

ComicInfo.xml is the standard metadata format for comic archives:

```xml
<?xml version="1.0" encoding="utf-8"?>
<ComicInfo>
  <Title>Amazing Spider-Man #300</Title>
  <Series>Amazing Spider-Man</Series>
  <Number>300</Number>
  <Count>700</Count>
  <Volume>1</Volume>
  <AlternateSeries>Spider-Man: Birth of Venom</AlternateSeries>
  <AlternateNumber>1</AlternateNumber>
  <AlternateCount>5</AlternateCount>
  <Summary>First appearance of Venom...</Summary>
  <Notes>Key issue</Notes>
  <Year>1988</Year>
  <Month>5</Month>
  <Day>10</Day>
  <Writer>David Michelinie</Writer>
  <Penciller>Todd McFarlane</Penciller>
  <Inker>Todd McFarlane</Inker>
  <Colorist>Bob Sharen</Colorist>
  <Letterer>Rick Parker</Letterer>
  <CoverArtist>Todd McFarlane</CoverArtist>
  <Editor>Jim Salicrup</Editor>
  <Publisher>Marvel</Publisher>
  <Imprint></Imprint>
  <Genre>Superhero, Action</Genre>
  <Tags>Venom, Key Issue, First Appearance</Tags>
  <Web>https://marvel.com</Web>
  <PageCount>32</PageCount>
  <LanguageISO>en</LanguageISO>
  <Format>Standard</Format>
  <AgeRating>Teen</AgeRating>
</ComicInfo>
```

**Supported Fields:**

| Field | Description |
|-------|-------------|
| `Title` | Issue/book title |
| `Series` | Series name |
| `Number` | Issue number |
| `Count` | Total issues in series |
| `Volume` | Volume number |
| `Summary` | Description/synopsis |
| `Year`, `Month`, `Day` | Publication date |
| `Writer`, `Penciller`, `Inker`, `Colorist` | Credits |
| `Publisher`, `Imprint` | Publisher info |
| `Genre`, `Tags` | Categorization |
| `PageCount` | Number of pages |
| `LanguageISO` | Language code (e.g., "en") |
| `AgeRating` | Content rating |

### EPUB Metadata (OPF)

EPUB metadata is extracted from the OPF file:

```xml
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>Book Title</dc:title>
  <dc:creator>Author Name</dc:creator>
  <dc:publisher>Publisher</dc:publisher>
  <dc:date>2024-01-15</dc:date>
  <dc:identifier>isbn:978-0-123456-78-9</dc:identifier>
  <dc:description>Book summary...</dc:description>
  <meta property="belongs-to-collection">Series Name</meta>
  <meta property="group-position">1</meta>
</metadata>
```

### PDF Metadata

PDF metadata is extracted from document properties:

- Title
- Author
- Subject
- Keywords
- Creator (application)
- Creation/Modification dates

## File Organization

### Recommended Structure

```
/library/
├── Comics/
│   └── [Publisher]/
│       └── [Series Name]/
│           ├── Series Name 001.cbz
│           ├── Series Name 002.cbz
│           └── ...
├── Manga/
│   └── [Series Name]/
│       ├── Series Name v01.cbz
│       ├── Series Name v02.cbz
│       └── ...
└── Ebooks/
    └── [Author]/
        └── Book Title.epub
```

### Naming Conventions

Codex parses metadata from filenames. Consistent naming improves detection:

| Pattern | Example | Extracted |
|---------|---------|-----------|
| Series + Number | `Batman 001.cbz` | Series: Batman, #1 |
| Series + Volume | `One Piece v01.cbz` | Series: One Piece, Vol 1 |
| Series + Year + Number | `Batman (2016) 001.cbz` | Series: Batman, Year: 2016, #1 |
| Series - Number | `Spider-Man-001.cbz` | Series: Spider-Man, #1 |
| Series # Number | `X-Men #142.cbz` | Series: X-Men, #142 |

**Tips:**
- Use leading zeros for proper sorting: `001`, `002`, not `1`, `2`
- Keep series names consistent across files
- Avoid special characters in filenames
- Use underscores or hyphens instead of spaces

## Format Detection

Codex detects formats using:

1. **File extension**: Initial identification
2. **Magic bytes**: Binary signature verification
3. **Content analysis**: Structure validation

This ensures correct handling even with incorrect extensions.

## Processing Performance

### Speed Comparison

| Format | Extraction | Metadata | Overall |
|--------|------------|----------|---------|
| CBZ | Fast | Fast | Fast |
| CBR | Moderate | Fast | Moderate |
| EPUB | Fast | Fast | Fast |
| PDF | Varies | Fast | Varies |

### Memory Usage

| File Size | Memory Impact |
|-----------|---------------|
| < 50 MB | Minimal |
| 50-200 MB | Moderate |
| 200-500 MB | Significant |
| > 500 MB | High |

:::tip Large Files
For very large files (> 500MB), consider:
- Splitting into multiple volumes
- Reducing image resolution
- Using more efficient compression
:::

## Format Conversion

Codex reads files as-is and doesn't convert formats. For conversion, use external tools:

- **Calibre**: Ebook conversion
- **ComicTagger**: Comic metadata editing
- **ImageMagick**: Image processing

## Future Format Support

Planned formats:

| Format | Status | Notes |
|--------|--------|-------|
| MOBI/AZW | Planned | Kindle formats |
| CB7 | Planned | 7-Zip comics |
| CBT | Planned | TAR comics |
| DJVU | Considered | Document format |

## Troubleshooting

### Format Not Recognized

1. Verify file extension matches content
2. Check file isn't corrupted: `file filename.cbz`
3. Try opening with appropriate tool (7-Zip, Calibre)

### Metadata Not Extracted

1. Verify ComicInfo.xml exists and is valid XML
2. Check EPUB OPF file structure
3. Re-scan with deep mode
4. Check logs for parsing errors

### Slow Processing

1. Check file size (large files are slower)
2. Verify disk I/O performance
3. Consider splitting large files
4. Reduce concurrent scan limit

### Images Not Displaying

1. Verify image format is supported
2. Check image file isn't corrupted
3. Try extracting manually with 7-Zip
4. Check server logs for errors

## Next Steps

- [Configure libraries](./libraries)
- [Set up OPDS](./opds)
- [API documentation](./api)
