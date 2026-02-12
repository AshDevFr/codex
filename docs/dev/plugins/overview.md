# Plugins Overview

Codex supports a plugin system that allows external processes to provide metadata, sync reading progress, and generate recommendations. Plugins communicate with Codex via JSON-RPC 2.0 over stdio.

## What Are Plugins?

Plugins are external processes that Codex spawns and communicates with. They can be written in any language (TypeScript, Python, Rust, etc.) and provide various capabilities:

- **Metadata Providers**: Search and fetch series/book metadata from external sources
- **Sync Providers**: Sync reading progress with external tracking services (AniList, MyAnimeList, etc.)
- **Recommendation Providers**: Generate personalized series recommendations based on user libraries

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              CODEX SERVER                                    │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐   │
│  │                          Plugin Manager                               │   │
│  │                                                                       │   │
│  │  • Spawns plugin processes (command + args)                           │   │
│  │  • Communicates via stdio/JSON-RPC                                    │   │
│  │  • Enforces RBAC permissions on writes                                │   │
│  │  • Monitors health, auto-disables on failures                         │   │
│  │  • Manages OAuth token refresh and storage quotas                     │   │
│  └───────────────────────────┬───────────────────────────────────────────┘   │
│                              │                                               │
│        ┌─────────────────────┼──────────────────────┐                        │
│        ▼                     ▼                      ▼                        │
│  ┌────────────┐     ┌──────────────┐     ┌───────────────────┐               │
│  │  Metadata  │     │    Sync      │     │ Recommendations   │               │
│  │  Plugins   │     │   Plugins    │     │    Plugins        │               │
│  │ stdin/out  │     │  stdin/out   │     │   stdin/out       │               │
│  └────────────┘     └──────────────┘     └───────────────────┘               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Available Plugins

### Official Plugins

| Plugin                  | Package                                        | Type           | Description                                 |
| ----------------------- | ---------------------------------------------- | -------------- | ------------------------------------------- |
| Echo Metadata           | `@ashdev/codex-plugin-metadata-echo`           | Metadata       | Test plugin for development (series + book) |
| Open Library Metadata   | `@ashdev/codex-plugin-metadata-openlibrary`    | Metadata       | Book metadata via ISBN or title search      |
| AniList Sync            | `@ashdev/codex-plugin-sync-anilist`            | Sync           | Bidirectional manga reading progress sync   |
| AniList Recommendations | `@ashdev/codex-plugin-recommendations-anilist` | Recommendation | Personalized manga recommendations          |

### Community Plugins

See [Writing Plugins](./writing-plugins.md) to create your own.

## Getting Started

### Using npx (Recommended)

The easiest way to run plugins is via `npx`, which downloads and runs the plugin automatically:

1. Navigate to **Admin Settings** > **Plugins**
2. Click **Add Plugin**
3. Configure:
   - **Command**: `npx`
   - **Arguments** (one per line):
     ```
     -y
     @ashdev/codex-plugin-metadata-echo@1.9.3
     ```
4. Add credentials if required (not needed for Echo or Open Library)
5. Click **Save** and **Enable**

:::warning Arguments Format
**Each argument must be on its own line.** Do NOT combine arguments like `-y @package@1.0.0` on one line.

✅ Correct:

```
-y
@ashdev/codex-plugin-metadata-echo@1.9.3
```

❌ Wrong:

```
-y @ashdev/codex-plugin-metadata-echo@1.9.3
```

:::

### npx Options

| Option           | Arguments (one per line)                                                   | Description                   |
| ---------------- | -------------------------------------------------------------------------- | ----------------------------- |
| Latest version   | `-y`<br/>`@ashdev/codex-plugin-metadata-echo`                              | Always uses latest            |
| Specific version | `-y`<br/>`@ashdev/codex-plugin-metadata-echo@1.9.3`                        | Pins to exact version         |
| Faster startup   | `-y`<br/>`--prefer-offline`<br/>`@ashdev/codex-plugin-metadata-echo@1.9.3` | Skips version check if cached |

**Flags explained:**

- `-y` (or `--yes`): Auto-confirms installation, required for non-interactive environments like containers
- `--prefer-offline`: Uses cached version without checking npm registry, faster startup

### Container Deployment

For containers, use `--prefer-offline` with a pinned version for fast, predictable startup:

```
Command: npx
Arguments (one per line):
  -y
  --prefer-offline
  @ashdev/codex-plugin-metadata-echo@1.9.3
```

You can pre-warm the npx cache in your Dockerfile:

```dockerfile
# Pre-cache plugin during image build
RUN npx -y @ashdev/codex-plugin-metadata-echo@1.9.3 --version || true
```

### Manual Installation

For maximum performance, install globally and reference directly:

```bash
npm install -g @ashdev/codex-plugin-metadata-echo
```

Then configure:

- **Command**: `codex-plugin-metadata-echo`
- **Arguments**: (none needed)

## Plugin Lifecycle

1. **Spawn**: When a plugin is needed, Codex spawns it as a child process
2. **Initialize**: Codex sends an `initialize` request with config, credentials, and storage handle; plugin responds with its manifest
3. **Requests**: Codex sends capability-specific requests (search, sync, recommendations); plugin responds
4. **Health Monitoring**: Failed requests are tracked; plugins auto-disable after repeated failures
5. **Shutdown**: On server shutdown or plugin disable, Codex sends `shutdown` request

## Plugin Capabilities

| Capability      | Manifest Field                         | Factory Function             | Description                         |
| --------------- | -------------------------------------- | ---------------------------- | ----------------------------------- |
| Series Metadata | `metadataProvider: ["series"]`         | `createMetadataPlugin`       | Search and fetch series metadata    |
| Book Metadata   | `metadataProvider: ["book"]`           | `createMetadataPlugin`       | Search and fetch book metadata      |
| Both            | `metadataProvider: ["series", "book"]` | `createMetadataPlugin`       | Series and book metadata            |
| Read Sync       | `userReadSync: true`                   | `createSyncPlugin`           | Bidirectional reading progress sync |
| Recommendations | `userRecommendationProvider: true`     | `createRecommendationPlugin` | Personalized series recommendations |

## Advanced Configuration

Codex provides advanced options to customize how plugins search for metadata and when auto-matching occurs.

### Search Query Templates

Plugins can use Handlebars templates to customize the search query. Configure a template per plugin:

```
{{metadata.title}} {{metadata.year}}
```

Available template fields:

- `metadata.title` - Series title (cleaned by preprocessing)
- `metadata.year` - Publication year
- `metadata.publisher` - Publisher name
- `metadata.language` - Language code
- `book_count` - Number of books in the series

See [Available Helpers](/docs/preprocessing-rules#available-helpers) for available template functions.

### Search Preprocessing Rules

Apply regex-based rules to clean search queries before sending to the plugin:

```json
[
  {
    "pattern": "\\s*\\(Digital\\)$",
    "replacement": "",
    "description": "Remove (Digital) suffix"
  }
]
```

Preprocessing can be configured at both library and plugin levels:

- **Library rules**: Applied first, affect all plugins
- **Plugin rules**: Applied after library rules, plugin-specific

### Auto-Match Conditions

Control when auto-matching occurs using condition rules:

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "external_ids.plugin:metadata-echo",
      "operator": "is_null"
    },
    {
      "field": "book_count",
      "operator": "gte",
      "value": 1
    }
  ]
}
```

Conditions can be set at library level (applies to all plugins) and plugin level (applies to specific plugin).

### External ID Reuse

Enable `use_existing_external_id` on a plugin to skip searching when the series already has an external ID for that plugin. The plugin will directly fetch updated metadata using the existing ID.

For detailed configuration options, see the [Preprocessing Rules Guide](/docs/preprocessing-rules).

## Security

- **RBAC**: Plugins have configurable permissions (what metadata they can write)
- **Process Isolation**: Plugins run as separate child processes with a command allowlist and environment variable blocklist
- **Health Monitoring**: Failing plugins are automatically disabled after repeated failures
- **Credential Encryption**: API keys and OAuth tokens are encrypted at rest using AES-256-GCM
- **Data Isolation**: All plugin storage is scoped per user-plugin connection (`user_plugin_id`)
- **Request Timeouts**: JSON-RPC requests have a 30-second timeout to prevent hangs
- **OAuth Protection**: CSRF state tokens (single-use, 5-minute TTL) and PKCE S256 challenge
- **Storage Quotas**: 100 keys and 1 MB per value per user-plugin connection

For full details, see the [Plugin Security Model](/docs/plugins#security-model) in the user documentation.

## Next Steps

- [Writing Plugins](./writing-plugins.md) - Create your own plugin
- [Plugin Protocol](./protocol.md) - Technical protocol specification
- [Plugin SDK](./sdk.md) - TypeScript SDK documentation
- [Preprocessing Rules Guide](/docs/preprocessing-rules) - Configure search preprocessing and conditions
