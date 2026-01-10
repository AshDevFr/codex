---
---

# Libraries & Scanning

Libraries are the foundation of Codex. This guide covers how to set up libraries, configure scanning, and organize your media collection.

## Understanding Libraries

A library is a folder on your server containing your digital media files (comics, manga, ebooks). Codex scans these folders to discover and catalog your content.

### Library Structure

Codex expects your media to be organized in folders:

```
/library/
├── Comics/
│   ├── Batman/
│   │   ├── Batman 001.cbz
│   │   ├── Batman 002.cbz
│   │   └── Batman 003.cbz
│   └── Spider-Man/
│       ├── Spider-Man v01.cbz
│       └── Spider-Man v02.cbz
├── Manga/
│   ├── One Piece/
│   │   ├── One Piece v01.cbz
│   │   └── One Piece v02.cbz
│   └── Naruto/
│       └── ...
└── Ebooks/
    ├── Fiction/
    │   ├── Novel.epub
    │   └── Another Novel.epub
    └── Non-Fiction/
        └── ...
```

### Series Detection

Codex automatically creates series from:

1. **Folder structure**: Each subfolder becomes a series
2. **Filename parsing**: Extracts series name, volume, and number
3. **Metadata**: ComicInfo.xml or EPUB metadata takes priority

## Creating a Library

### Via Web Interface

1. Log in as an admin
2. Go to **Settings** > **Libraries**
3. Click **Add Library**
4. Fill in the details:
   - **Name**: Display name for the library
   - **Path**: Filesystem path to the folder
   - **Scanning Options**: Configure automatic scanning

### Via API

```bash
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Comics",
    "path": "/library/comics",
    "scanning_config": {
      "enabled": true,
      "cron_schedule": "0 0 * * *",
      "default_mode": "normal",
      "scan_on_start": true
    }
  }'
```

### Via CLI (Initial Setup)

During initial setup, create a library after seeding the admin user:

```bash
# After running codex seed
# Use the API or web interface to create libraries
```

## Scanning

Codex scans libraries to discover and catalog your media files.

### Scan Modes

| Mode | Description | Speed | Use Case |
|------|-------------|-------|----------|
| **Normal** | Only processes new or changed files | Fast | Daily scans |
| **Deep** | Re-analyzes all files | Slow | Metadata fixes |

#### Normal Scan

- Checks file timestamps and hashes
- Only processes new or modified files
- Skips unchanged files
- Recommended for scheduled scans

#### Deep Scan

- Re-processes every file
- Updates all metadata
- Useful after:
  - Changing metadata in files
  - Fixing ComicInfo.xml
  - Upgrading Codex (new parser features)

### Triggering Scans

#### Via Web Interface

1. Go to the library
2. Click the **Scan** button
3. Choose **Normal** or **Deep** scan

#### Via API

```bash
# Normal scan
curl -X POST "http://localhost:8080/api/v1/libraries/{id}/scan?mode=normal" \
  -H "Authorization: Bearer $TOKEN"

# Deep scan
curl -X POST "http://localhost:8080/api/v1/libraries/{id}/scan?mode=deep" \
  -H "Authorization: Bearer $TOKEN"

# Check scan status
curl http://localhost:8080/api/v1/libraries/{id}/scan-status \
  -H "Authorization: Bearer $TOKEN"
```

### Automatic Scanning

Configure automatic scanning with cron schedules:

```json
{
  "scanning_config": {
    "enabled": true,
    "cron_schedule": "0 0 * * *",
    "default_mode": "normal",
    "scan_on_start": true
  }
}
```

| Field | Description | Example |
|-------|-------------|---------|
| `enabled` | Enable automatic scanning | `true` |
| `cron_schedule` | Cron expression | `0 0 * * *` (daily at midnight) |
| `default_mode` | Scan mode to use | `normal` or `deep` |
| `scan_on_start` | Scan when Codex starts | `true` |

#### Cron Expression Examples

| Expression | Schedule |
|------------|----------|
| `0 0 * * *` | Daily at midnight |
| `0 */6 * * *` | Every 6 hours |
| `0 0 * * 0` | Weekly on Sunday |
| `0 0 1 * *` | Monthly on the 1st |
| `*/30 * * * *` | Every 30 minutes |

### Scan Progress

Track scan progress in real-time:

#### Via SSE Stream

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H "Accept: text/event-stream" \
  http://localhost:8080/api/v1/scans/stream
```

Events include:
- Files discovered
- Files processed
- Series created
- Books added
- Errors encountered

#### Via Web Interface

The UI shows real-time progress with:
- Progress bar
- Current file being processed
- Statistics (new books, series, errors)

## Library Settings

### Path Configuration

The library path must be:
- An absolute path
- Readable by the Codex process
- For Docker: mounted as a volume

```yaml
# Docker volume mount
volumes:
  - /mnt/media/comics:/library/comics:ro
```

:::tip Read-Only Mount
Mount libraries as read-only (`:ro`) to prevent accidental modifications. Codex only needs read access.
:::

### Multiple Libraries

Create separate libraries for different content types:

| Library | Path | Content |
|---------|------|---------|
| Comics | `/library/comics` | Western comics |
| Manga | `/library/manga` | Japanese manga |
| Ebooks | `/library/ebooks` | EPUB/PDF books |

Benefits:
- Independent scan schedules
- Separate organization
- Different access permissions (future)

## Series Organization

### Automatic Series Detection

Codex creates series from:

1. **Folder names**: Each folder containing books becomes a series
2. **Filename patterns**: Extracts series name from common patterns

### Filename Patterns

Codex recognizes common naming conventions:

| Pattern | Extracted |
|---------|-----------|
| `Series Name v01.cbz` | Series: "Series Name", Volume: 1 |
| `Series Name #001.cbz` | Series: "Series Name", Number: 1 |
| `Series-Name-001.cbz` | Series: "Series Name", Number: 1 |
| `Series Name (2024) 001.cbz` | Series: "Series Name", Year: 2024, Number: 1 |

### Metadata Priority

Metadata sources (highest to lowest priority):

1. **ComicInfo.xml** - In CBZ/CBR files
2. **EPUB Metadata** - OPF file in EPUBs
3. **PDF Metadata** - Document properties
4. **Filename** - Parsed from file name
5. **Folder Name** - Parent folder name

## File Management

### Adding New Files

1. Add files to your library folder
2. Trigger a scan (or wait for automatic scan)
3. Codex discovers and catalogs the new files

### Removing Files

1. Delete files from your library folder
2. Run a scan
3. Codex marks the books as deleted (soft delete)

### Soft Deletes

Deleted files are soft-deleted in the database:
- Removed from library views
- Reading progress preserved
- Can be restored if file returns
- Permanent deletion available via API

### Moving Files

If you move files:
1. Codex detects the file is missing (soft delete)
2. Codex discovers the file in new location (new entry)
3. File hash matching can detect this as a move (preserves metadata)

## Duplicate Detection

Codex can detect duplicate books across libraries:

### Enable Duplicate Scanning

```bash
curl -X POST http://localhost:8080/api/v1/duplicates/scan \
  -H "Authorization: Bearer $TOKEN"
```

### View Duplicates

```bash
curl http://localhost:8080/api/v1/duplicates \
  -H "Authorization: Bearer $TOKEN"
```

Duplicates are detected by:
- File hash (exact duplicates)
- Metadata matching (same series/number)

## Troubleshooting

### Scan Not Finding Files

1. **Check path**: Verify the library path exists
2. **Check permissions**: Ensure Codex can read the directory
3. **Check file types**: Only supported formats are scanned
4. **Check logs**: Look for errors in Codex logs

```bash
# Docker
docker compose logs codex | grep -i "scan\|error"

# Systemd
journalctl -u codex | grep -i "scan\|error"
```

### Series Not Grouped Correctly

1. **Check folder structure**: Books in same folder = same series
2. **Check filenames**: Consistent naming helps parsing
3. **Add ComicInfo.xml**: Explicit metadata overrides parsing
4. **Re-scan with deep mode**: Forces metadata re-extraction

### Metadata Not Updating

1. **Run deep scan**: Normal scan skips unchanged files
2. **Check ComicInfo.xml**: Ensure it's valid XML
3. **Check file timestamps**: Touch files to mark as changed

### Scan Taking Too Long

1. **Check concurrent scans setting**: Lower if system is overloaded
2. **Use normal mode**: Skip unchanged files
3. **Check disk I/O**: Slow storage affects scanning
4. **Check worker count**: Adjust based on CPU cores

## Best Practices

### Folder Organization

```
/library/
├── Comics/           # One library for western comics
│   └── [Series]/     # Each series in its own folder
│       └── files...
├── Manga/           # Separate library for manga
│   └── [Series]/
│       └── files...
└── Ebooks/          # Separate library for books
    └── [Category]/
        └── files...
```

### File Naming

Consistent naming helps Codex parse metadata:

```
# Good
Batman 001.cbz
Batman 002.cbz
One Piece v01.cbz
One Piece v02.cbz

# Less ideal (but works)
batman_issue_1.cbz
onepiece-vol-1-chapter-1-10.cbz
```

### ComicInfo.xml

For best results, include ComicInfo.xml in your comics:

```xml
<?xml version="1.0"?>
<ComicInfo>
  <Title>Issue Title</Title>
  <Series>Batman</Series>
  <Number>1</Number>
  <Writer>Author Name</Writer>
  <Publisher>DC Comics</Publisher>
  <Genre>Superhero</Genre>
  <Summary>Issue description...</Summary>
</ComicInfo>
```

### Scan Schedules

- **Small libraries** (< 1000 books): Daily or on-demand
- **Medium libraries** (1000-10000 books): Daily at off-peak hours
- **Large libraries** (> 10000 books): Weekly or on-demand

## Next Steps

- [Supported formats](./formats)
- [Configure OPDS](./opds)
- [API documentation](./api)
