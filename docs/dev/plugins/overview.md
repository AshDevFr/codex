# Plugins Overview

Codex supports a plugin system that allows external processes to provide metadata, sync reading progress, and more. Plugins communicate with Codex via JSON-RPC 2.0 over stdio.

## What Are Plugins?

Plugins are external processes that Codex spawns and communicates with. They can be written in any language (TypeScript, Python, Rust, etc.) and provide various capabilities:

- **Metadata Providers**: Search and fetch metadata from external sources like MangaBaka, AniList, ComicVine
- **Sync Providers** (coming soon): Sync reading progress with external services
- **Recommendation Providers** (coming soon): Provide personalized recommendations

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                  CODEX SERVER                                    │
├──────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌───────────────────────────────────────────────────────────────────────────┐   │
│  │                            Plugin Manager                                 │   │
│  │                                                                           │   │
│  │  • Spawns plugin processes (command + args)                               │   │
│  │  • Communicates via stdio/JSON-RPC                                        │   │
│  │  • Enforces RBAC permissions on writes                                    │   │
│  │  • Monitors health, auto-disables on failures                             │   │
│  └─────────────────────────────────┬─────────────────────────────────────────┘   │
│                                    │                                             │
│             ┌──────────────────────┼──────────────────────┐                      │
│             ▼                      ▼                      ▼                      │
│     ┌──────────────┐  ┌────────────────────┐  ┌─────────────────┐                │
│     │  MangaBaka   │  │   Open Library     │  │     Custom      │                │
│     │   Plugin     │  │     Plugin         │  │     Plugin      │                │
│     │  (series)    │  │  (books, no key)   │  │                 │                │
│     │ stdin/stdout │  │   stdin/stdout     │  │  stdin/stdout   │                │
│     └──────────────┘  └────────────────────┘  └─────────────────┘                │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Available Plugins

### Official Plugins

| Plugin                | Package                                     | Description                                              | Status    |
| --------------------- | ------------------------------------------- | -------------------------------------------------------- | --------- |
| MangaBaka Metadata    | `@ashdev/codex-plugin-metadata-mangabaka`   | Aggregated manga metadata from multiple sources          | Available |
| Open Library Metadata | `@ashdev/codex-plugin-metadata-openlibrary` | Book metadata from Open Library via ISBN or title search | Available |
| Echo Metadata         | `@ashdev/codex-plugin-metadata-echo`        | Test plugin for development                              | Available |

### Community Plugins

Coming soon! See [Writing Plugins](./writing-plugins.md) to create your own.

## Getting Started

### Using npx (Recommended)

The easiest way to run plugins is via `npx`, which downloads and runs the plugin automatically:

1. Navigate to **Admin Settings** → **Plugins**
2. Click **Add Plugin**
3. Configure:
   - **Command**: `npx`
   - **Arguments** (one per line):
     ```
     -y
     @ashdev/codex-plugin-metadata-mangabaka@1.0.0
     ```
     Or for the Open Library plugin (no API key required):
     ```
     -y
     @ashdev/codex-plugin-metadata-openlibrary@1.0.0
     ```
4. Add your credentials (API keys, etc.) — not required for Open Library
5. Click **Save** and **Enable**

:::warning Arguments Format
**Each argument must be on its own line.** Do NOT combine arguments like `-y @package@1.0.0` on one line.

✅ Correct:

```
-y
@ashdev/codex-plugin-metadata-mangabaka@1.0.0
```

❌ Wrong:

```
-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0
```

:::

### npx Options

| Option           | Arguments (one per line)                                                        | Description                   |
| ---------------- | ------------------------------------------------------------------------------- | ----------------------------- |
| Latest version   | `-y`<br/>`@ashdev/codex-plugin-metadata-mangabaka`                              | Always uses latest            |
| Specific version | `-y`<br/>`@ashdev/codex-plugin-metadata-mangabaka@1.0.0`                        | Pins to exact version         |
| Version range    | `-y`<br/>`@ashdev/codex-plugin-metadata-mangabaka@^1.0.0`                       | Allows compatible updates     |
| Faster startup   | `-y`<br/>`--prefer-offline`<br/>`@ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Skips version check if cached |

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
  @ashdev/codex-plugin-metadata-mangabaka@1.0.0
```

You can pre-warm the npx cache in your Dockerfile:

```dockerfile
# Pre-cache plugin during image build
RUN npx -y @ashdev/codex-plugin-metadata-mangabaka@1.0.0 --version || true
```

### Manual Installation

For maximum performance, install globally and reference directly:

```bash
npm install -g @ashdev/codex-plugin-metadata-mangabaka
```

Then configure:

- **Command**: `codex-plugin-metadata-mangabaka`
- **Arguments**: (none needed)

## Plugin Lifecycle

1. **Spawn**: When a plugin is needed, Codex spawns it as a child process
2. **Initialize**: Codex sends an `initialize` request, plugin responds with its manifest
3. **Requests**: Codex sends requests (search, get, match), plugin responds
4. **Health Monitoring**: Failed requests are tracked; plugins auto-disable after repeated failures
5. **Shutdown**: On server shutdown or plugin disable, Codex sends `shutdown` request

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

See [Handlebars Helpers](#handlebars-helpers) for available template functions.

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
      "field": "external_ids.plugin:mangabaka",
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
- **Process Isolation**: Plugins run as separate processes
- **Health Monitoring**: Failing plugins are automatically disabled
- **Credential Encryption**: API keys are encrypted at rest

## Next Steps

- [Writing Plugins](./writing-plugins.md) - Create your own plugin
- [Plugin Protocol](./protocol.md) - Technical protocol specification
- [Plugin SDK](./sdk.md) - TypeScript SDK documentation
- [Preprocessing Rules Guide](/docs/preprocessing-rules) - Configure search preprocessing and conditions
