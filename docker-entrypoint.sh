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
    # Not running as root - container started with 'user:' directive
    CURRENT_UID=$(id -u)
    CURRENT_GID=$(id -g)
    echo "Running as UID=$CURRENT_UID, GID=$CURRENT_GID (container started with custom user)"

    # Check write permissions on required directories
    for dir in /app/data /app/config /app/.npm; do
        if [ -d "$dir" ] && [ ! -w "$dir" ]; then
            echo "WARNING: No write permission on $dir"
            echo "  Current user: UID=$CURRENT_UID, GID=$CURRENT_GID"
            echo "  Directory owner: $(stat -c '%u:%g' "$dir" 2>/dev/null || stat -f '%u:%g' "$dir" 2>/dev/null || echo 'unknown')"
            echo "  Fix: Ensure mounted volumes are owned by UID=$CURRENT_UID or use PUID/PGID without 'user:' directive"
        fi
    done

    exec "$@"
fi
