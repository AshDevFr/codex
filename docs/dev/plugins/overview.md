# Plugins Overview

Codex supports a plugin system that allows external processes to provide metadata, sync reading progress, and more. Plugins communicate with Codex via JSON-RPC 2.0 over stdio.

## What Are Plugins?

Plugins are external processes that Codex spawns and communicates with. They can be written in any language (TypeScript, Python, Rust, etc.) and provide various capabilities:

- **Metadata Providers**: Search and fetch metadata from external sources like MangaBaka, AniList, ComicVine
- **Sync Providers** (coming soon): Sync reading progress with external services
- **Recommendation Providers** (coming soon): Provide personalized recommendations

## Architecture

```
┌───────────────────────────────────────────────────────────────────┐
│                          CODEX SERVER                             │
├───────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                      Plugin Manager                         │  │
│  │                                                             │  │
│  │  • Spawns plugin processes (command + args)                 │  │
│  │  • Communicates via stdio/JSON-RPC                          │  │
│  │  • Enforces RBAC permissions on writes                      │  │
│  │  • Monitors health, auto-disables on failures               │  │
│  └──────────────────────────┬──────────────────────────────────┘  │
│                             │                                     │
│             ┌───────────────┼───────────────┐                     │
│             ▼               ▼               ▼                     │
│     ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│     │  MangaBaka  │  │   AniList   │  │   Custom    │             │
│     │   Plugin    │  │   Plugin    │  │   Plugin    │             │
│     │             │  │             │  │             │             │
│     │ stdin/stdout│  │ stdin/stdout│  │ stdin/stdout│             │
│     └─────────────┘  └─────────────┘  └─────────────┘             │
└───────────────────────────────────────────────────────────────────┘
```

## Available Plugins

### Official Plugins

| Plugin | Package | Description | Status |
|--------|---------|-------------|--------|
| MangaBaka Metadata | `@ashdev/codex-plugin-metadata-mangabaka` | Aggregated manga metadata from multiple sources | Available |
| Echo Metadata | `@ashdev/codex-plugin-metadata-echo` | Test plugin for development | Available |

### Community Plugins

Coming soon! See [Writing Plugins](./writing-plugins.md) to create your own.

## Getting Started

### Using npx (Recommended)

The easiest way to run plugins is via `npx`, which downloads and runs the plugin automatically:

1. Navigate to **Admin Settings** → **Plugins**
2. Click **Add Plugin**
3. Configure:
   - **Command**: `npx`
   - **Arguments**: `-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0`
4. Add your credentials (API keys, etc.)
5. Click **Save** and **Enable**

### npx Options

| Option | Example | Description |
|--------|---------|-------------|
| Latest version | `-y @ashdev/codex-plugin-metadata-mangabaka` | Always uses latest |
| Specific version | `-y @ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Pins to exact version |
| Version range | `-y @ashdev/codex-plugin-metadata-mangabaka@^1.0.0` | Allows compatible updates |
| Faster startup | `-y --prefer-offline @ashdev/codex-plugin-metadata-mangabaka@1.0.0` | Skips version check if cached |

**Flags explained:**
- `-y` (or `--yes`): Auto-confirms installation, required for non-interactive environments like containers
- `--prefer-offline`: Uses cached version without checking npm registry, faster startup

### Container Deployment

For containers, use `--prefer-offline` with a pinned version for fast, predictable startup:

```
Command: npx
Arguments: -y --prefer-offline @ashdev/codex-plugin-metadata-mangabaka@1.0.0
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

## Security

- **RBAC**: Plugins have configurable permissions (what metadata they can write)
- **Process Isolation**: Plugins run as separate processes
- **Health Monitoring**: Failing plugins are automatically disabled
- **Credential Encryption**: API keys are encrypted at rest

## Next Steps

- [Writing Plugins](./writing-plugins.md) - Create your own plugin
- [Plugin Protocol](./protocol.md) - Technical protocol specification
- [Plugin SDK](./sdk.md) - TypeScript SDK documentation
