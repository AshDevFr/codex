---
---

# Plugins

Codex supports metadata plugins that can automatically fetch and enrich your library's metadata from external sources. Plugins use a JSON-RPC protocol and can provide metadata for both series and individual books.

## Available Plugins

### Metadata Plugins

| Plugin | Description | Source |
|--------|-------------|--------|
| [Open Library](./open-library) | Fetch book metadata from Open Library using ISBN or title search | Free, no API key required |
| Echo (built-in) | Development/testing plugin that echoes back sample metadata | Included with Codex |

### Sync Plugins

| Plugin | Description | Source |
|--------|-------------|--------|
| [AniList Sync](./anilist-sync) | Sync manga reading progress between Codex and AniList | Free, requires AniList account |

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

## Codex Sync Settings

When using sync plugins (like AniList Sync), Codex provides a set of **generic sync settings** that apply to all sync plugins. These settings control which entries the server sends to the plugin.

The settings are stored in the user's plugin config under the `_codex` namespace:

```json
{
  "_codex": {
    "includeCompleted": true,
    "includeInProgress": true,
    "countPartialProgress": false,
    "syncRatings": true
  }
}
```

| Key | Default | Description |
|-----|---------|-------------|
| `includeCompleted` | `true` | Include series where all local books are marked as read |
| `includeInProgress` | `true` | Include series where at least one book has been started |
| `countPartialProgress` | `false` | Count partially-read books in the progress count |
| `syncRatings` | `true` | Include scores and notes in push/pull operations |

These are **server-interpreted** — the server reads them to filter and build sync entries. Plugins never read `_codex` keys. Plugin-specific settings (like `progressUnit` for AniList) live in the plugin's own `userConfigSchema` and are only read by the plugin.

## Plugin Development

Codex provides a TypeScript SDK for building metadata plugins. Plugins implement a JSON-RPC interface with methods for searching, matching, and retrieving metadata.

See the Echo plugin (`plugins/metadata-echo/`) as a reference implementation.

### Protocol Versioning

Plugins declare their protocol version via `protocolVersion: "1.0"` in the manifest. The versioning contract:

- **Additive changes** (new optional fields, new methods) do NOT bump the protocol version. Plugins should ignore unknown fields.
- **Breaking changes** (removed fields, changed semantics, required field changes) bump the major version.
- **No runtime negotiation** — the server checks the plugin's declared version and rejects incompatible plugins.
- **Old methods are preserved** within a major version. If a method signature changes in a backward-incompatible way, a new method name is used.

This means plugins built for protocol `1.x` will continue to work as long as the server supports major version `1`. New optional fields (like `latestUpdatedAt` on `SyncEntry` or `totalVolumes` on `SyncProgress`) are additive and do not require a version bump.
