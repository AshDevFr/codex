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

## Security Model

Codex applies multiple layers of security to ensure plugins operate safely and user data is protected.

### Credential & Token Encryption

All sensitive data — OAuth tokens, API keys, and plugin credentials — is encrypted at rest using **AES-256-GCM** (authenticated encryption). Each value is encrypted with a random 96-bit nonce, ensuring identical plaintext produces different ciphertext. The encryption key is derived from the `CODEX_ENCRYPTION_KEY` environment variable (a base64-encoded 32-byte key).

### Data Isolation

Plugin data is isolated per user. Each user-plugin connection has a unique `user_plugin_id`, and all storage operations — reads, writes, and deletes — are scoped to that ID at the database level. A plugin connected by one user cannot access another user's storage, tokens, or configuration.

### Plugin Process Sandboxing

Plugins run as **child processes** spawned by Codex, communicating over stdin/stdout via JSON-RPC. This provides process-level isolation:

- **Command allowlist**: Only approved commands can be used to launch plugins (`node`, `npx`, `python`, `python3`, `uv`, `uvx`, and paths under `/opt/codex/plugins/`). Custom commands can be allowed via the `CODEX_PLUGIN_ALLOWED_COMMANDS` environment variable.
- **Environment variable blocklist**: Dangerous environment variables are stripped before spawning plugins, including `LD_*`, `DYLD_*`, `PATH`, `HOME`, `PYTHONPATH`, `NODE_PATH`, and others that could enable library injection or path manipulation.
- **Request timeout**: Every JSON-RPC request has a **30-second timeout**. If a plugin hangs or becomes unresponsive, the request fails gracefully rather than blocking the server.
- **Health monitoring**: Failed requests are tracked, and plugins that fail repeatedly are automatically disabled.

### OAuth Security

OAuth connections (used by sync and recommendation plugins) are protected by:

- **CSRF state tokens**: Each OAuth flow generates a cryptographically random 32-byte state parameter. State tokens are single-use (consumed on callback) and expire after **5 minutes**.
- **PKCE (S256)**: When the external service supports it, Codex uses Proof Key for Code Exchange with SHA-256 challenge method to prevent authorization code interception.
- **Rate limiting**: Each user is limited to **3 concurrent pending OAuth flows**. Additional attempts return HTTP 429 until existing flows complete or expire.
- **Automatic cleanup**: Expired OAuth state entries are periodically removed from memory by a background cleanup task.

### Storage Quotas

Plugin storage is subject to per-plugin limits to prevent abuse:

- **Maximum 100 keys** per user-plugin connection
- **Maximum 1 MB** per stored value

These limits are enforced on writes only — existing data is not affected. Updating an existing key (upsert) does not count against the key limit.

## Privacy & Data Handling

### What Data Leaves Codex

The data sent to external services depends on the plugin type:

| Plugin Type | Data Sent | Destination |
|-------------|-----------|-------------|
| **Metadata** | Series titles, ISBNs, search queries | Metadata provider API (e.g., Open Library) |
| **Sync** | Series titles, reading progress (books read), scores, dates, reading status | Tracking service API (e.g., AniList) |
| **Recommendations** | Library series titles (used as "seed" entries) | Recommendation service API (e.g., AniList) |

Codex never sends file contents, file paths, or raw images to external services.

### What Data Is Stored Locally

- **OAuth tokens**: Encrypted at rest in the Codex database (AES-256-GCM)
- **API keys / credentials**: Encrypted at rest in the Codex database
- **Plugin configuration**: Stored in the database, scoped per user-plugin connection
- **Plugin storage**: Key-value data stored by plugins (e.g., sync state, caches), scoped per user-plugin connection
- **Cached recommendations**: Stored locally in the database, refreshed on demand

### Disconnecting a Plugin

To remove all data associated with a plugin connection:

1. Go to **Settings** > **Integrations**
2. Click **Disconnect** on the plugin
3. This deletes: OAuth tokens, stored credentials, plugin configuration, and all plugin storage data

The external service retains any data already synced to it (e.g., your AniList reading list). To remove that data, use the external service's own settings.

## Troubleshooting OAuth Connections

### Popup Blocked

**Symptom**: Clicking "Connect" opens nothing, or the browser blocks the popup.

**Fix**: Allow popups for your Codex URL in your browser settings, then try again.

### Redirect URI Mismatch

**Symptom**: The external service shows "redirect_uri mismatch" or a similar error.

**Fix**: Ensure the OAuth redirect URI configured in the external service matches your Codex URL exactly. For AniList, the redirect URL should be set in your [AniList Developer Settings](https://anilist.co/settings/developer). The correct redirect URL is shown in the plugin's OAuth configuration panel in **Settings** > **Plugins**.

### Token Expired / "Not Connected"

**Symptom**: A plugin that was previously connected now shows as disconnected or fails with authentication errors.

**Fix**: OAuth tokens can expire. Click **Connect** again to re-authorize. Your plugin configuration and storage data are preserved — only the token is refreshed.

### Rate Limited by External Service

**Symptom**: Sync or recommendations fail with errors mentioning "rate limit", "429", or "too many requests".

**Fix**: Wait a few minutes before retrying. AniList has a rate limit of approximately 90 requests per minute. If syncing a large library, the plugin automatically retries once on rate-limit responses. Repeated failures may require waiting longer.

### "Connection Failed" or Timeout

**Symptom**: OAuth flow completes but Codex shows "Connection failed", or the popup hangs.

**Fix**:

1. Check that your Codex server can reach the external service (network/firewall).
2. Ensure you completed the OAuth flow within 5 minutes — state tokens expire after that.
3. Try disconnecting and reconnecting the plugin.
4. Check the Codex server logs for detailed error messages.

### Too Many Connection Attempts

**Symptom**: Clicking "Connect" returns a "Too Many Requests" error.

**Fix**: You have 3 or more pending OAuth flows. Wait for them to expire (5 minutes) or complete one of them, then try again.

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
