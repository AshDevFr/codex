#!/bin/bash
set -e

# Install dependencies if node_modules is empty (first run with volume mount)
if [ ! -d "/work/node_modules/tsx" ]; then
    echo "Installing dependencies..."
    npm install
fi

# Execute the command passed to the container
exec "$@"
