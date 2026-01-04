---
sidebar_position: 4
---

# Installation

Codex can be installed using a pre-built binary or Docker. Choose the method that works best for your setup.

## Option 1: Pre-built Binary

### Download

Download the latest release binary for your platform from the [releases page](https://github.com/yourusername/codex/releases).

### Install

#### Linux/macOS

```bash
# Download and extract
tar -xzf codex-x.x.x-linux-x86_64.tar.gz

# Make executable
chmod +x codex

# Move to PATH (optional)
sudo mv codex /usr/local/bin/
```

#### Windows

1. Download the `.exe` file
2. Extract to a folder in your PATH (e.g., `C:\Program Files\Codex\`)
3. Add the folder to your system PATH

### Verify Installation

```bash
codex --help
```

You should see the command-line help output.

## Option 2: Docker

The easiest way to run Codex is with Docker.

### Quick Start

```bash
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/config:/app/config \
  codex:latest
```

### Docker Compose

For a complete setup with PostgreSQL:

```bash
git clone https://github.com/yourusername/codex.git
cd codex
docker compose up -d
```

See the [Deployment Guide](./deployment#docker) for detailed Docker setup instructions.

## Next Steps

1. Create a [configuration file](./configuration)
2. Set up your [database](./configuration#database-configuration)
3. Start the [server](./configuration#application-configuration)
4. Follow the [Getting Started](./getting-started) guide

## Building from Source

If you need to build from source (for development or custom builds), see the [Development Guide](./development).
