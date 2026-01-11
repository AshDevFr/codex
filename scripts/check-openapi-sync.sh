#!/bin/bash
# Pre-commit hook to ensure OpenAPI files are in sync with the backend
#
# This script regenerates the OpenAPI spec and TypeScript types, then checks
# if git detects any changes. If they differ, the commit is aborted.

set -e

OPENAPI_JSON="web/openapi.json"
OPENAPI_TYPES="web/src/types/api.generated.ts"

echo "Checking OpenAPI spec synchronization..."

# Regenerate OpenAPI files
echo "Regenerating OpenAPI spec and TypeScript types..."
make openapi-all > /dev/null 2>&1

# Check if git detects any changes to the generated files
if ! git diff --quiet -- "$OPENAPI_JSON" "$OPENAPI_TYPES"; then
    echo ""
    echo "ERROR: OpenAPI files are out of sync with the backend."
    echo ""
    git diff --stat -- "$OPENAPI_JSON" "$OPENAPI_TYPES"
    echo ""
    echo "Please stage the updated files:"
    echo ""
    echo "  git add $OPENAPI_JSON $OPENAPI_TYPES"
    echo ""
    echo "Then try committing again."
    exit 1
fi

echo "OpenAPI files are in sync."
exit 0
