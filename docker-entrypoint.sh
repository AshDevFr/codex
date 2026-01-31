#!/bin/sh
set -e

# Default to UID/GID 1000 if not specified
PUID=${PUID:-1000}
PGID=${PGID:-1000}

# Only modify user/group if running as root
if [ "$(id -u)" = "0" ]; then
    # Update codex group GID if different
    if [ "$(id -g codex)" != "$PGID" ]; then
        echo "Changing codex group GID to $PGID"
        delgroup codex 2>/dev/null || true
        addgroup -g "$PGID" codex
    fi

    # Update codex user UID if different
    if [ "$(id -u codex)" != "$PUID" ]; then
        echo "Changing codex user UID to $PUID"
        deluser codex 2>/dev/null || true
        adduser -D -u "$PUID" -G codex codex
    fi

    # Ensure ownership of app directories
    # Include .npm for npx plugin cache
    chown -R codex:codex /app/data /app/config /app/.npm 2>/dev/null || true

    echo "Running as codex (UID=$PUID, GID=$PGID)"
    exec su-exec codex "$@"
else
    # Not running as root, just execute the command
    exec "$@"
fi
