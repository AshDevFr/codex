---
---

# Plugins

Codex supports metadata plugins that can automatically fetch and enrich your library's metadata from external sources. Plugins use a JSON-RPC protocol and can provide metadata for both series and individual books.

## Available Plugins

| Plugin | Description | Source |
|--------|-------------|--------|
| [Open Library](./open-library) | Fetch book metadata from Open Library using ISBN or title search | Free, no API key required |
| Echo (built-in) | Development/testing plugin that echoes back sample metadata | Included with Codex |

## Plugin Capabilities

Plugins can provide metadata for:

- **Series** - Title, summary, genres, tags, publisher, status, and more
- **Books** - Title, subtitle, authors, subjects, awards, ISBNs, covers, and more

## Managing Plugins

### Installation

1. Place the plugin folder in Codex's plugins directory
2. Restart Codex or reload plugins from **Settings > Plugins**
3. Grant the requested permissions

### Permissions

Each plugin requests specific permissions for the metadata fields it can write. You can review and manage permissions in **Settings > Plugins**.

### Metadata Locks

If you've manually edited a metadata field, you can lock it to prevent plugins from overwriting your changes. Toggle the lock icon next to any field in the metadata editor.

## Plugin Development

Codex provides a TypeScript SDK for building metadata plugins. Plugins implement a JSON-RPC interface with methods for searching, matching, and retrieving metadata.

See the Echo plugin (`plugins/metadata-echo/`) as a reference implementation.
