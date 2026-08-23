#!/usr/bin/env bash
# Advisory: does this version bump match what actually changed in the API?
#
# Compares the freshly regenerated spec against the previous tag's and reports
# whether the bump is consistent with the changes found.
#
# Deliberately never fails. By the time this runs the work is merged, and a
# breaking change is sometimes exactly what was intended. The point is that the
# number on the tin is a decision rather than a habit.
#
# Note that oasdiff compares the *document*, not the server. Correcting a spec
# that described an endpoint wrongly reads as a breaking change here even though
# no running client changes behaviour, because a client generated from the old
# document really would see something different.

set -uo pipefail

VERSION="${1:-}"
# Optional: compare against this ref instead of the latest tag. Only needed to
# re-check a past release, or to test this script.
BASE_REF="${2:-}"
SPEC="web/openapi.json"

if [ -z "$VERSION" ]; then
    echo "usage: $0 <version> [base-ref]" >&2
    exit 0
fi

if ! command -v oasdiff >/dev/null 2>&1; then
    echo "  oasdiff not installed, skipping the API bump check (brew install oasdiff)"
    exit 0
fi

PREV_TAG="${BASE_REF:-$(git describe --tags --abbrev=0 2>/dev/null)}"
if [ -z "$PREV_TAG" ]; then
    echo "  No previous tag, skipping the API bump check"
    exit 0
fi

if ! git cat-file -e "${PREV_TAG}:${SPEC}" 2>/dev/null; then
    echo "  ${PREV_TAG} has no ${SPEC}, skipping the API bump check"
    exit 0
fi

# What kind of bump is this?
PREV_VERSION="${PREV_TAG#v}"
IFS=. read -r prev_major prev_minor prev_patch <<EOF
${PREV_VERSION}
EOF
IFS=. read -r next_major next_minor next_patch <<EOF
${VERSION}
EOF

if [ "${next_major:-0}" -gt "${prev_major:-0}" ]; then
    BUMP=major
elif [ "${next_minor:-0}" -gt "${prev_minor:-0}" ]; then
    BUMP=minor
elif [ "${next_patch:-0}" -gt "${prev_patch:-0}" ]; then
    BUMP=patch
else
    BUMP=none
fi

REPORT=$(oasdiff breaking "${PREV_TAG}:${SPEC}" "${SPEC}" 2>/dev/null)
ERRORS=$(printf '%s' "$REPORT" | grep -c '^error')
WARNINGS=$(printf '%s' "$REPORT" | grep -c '^warning')

echo "  ${PREV_TAG} -> v${VERSION} is a ${BUMP} bump"
echo "  API contract: ${ERRORS} breaking, ${WARNINGS} warnings"

if [ "$ERRORS" -gt 0 ] && [ "$BUMP" != "major" ]; then
    echo ""
    echo "  ⚠ ${ERRORS} breaking changes in a ${BUMP} release."
    echo "    A client generated against ${PREV_TAG} may stop working."
    printf '%s' "$REPORT" | grep -A1 '^error' | grep 'in API' | sed 's/^[[:space:]]*/     /' | sort -u
    echo ""
    echo "    Intentional? Nothing to do, but say so in the release notes."
    echo "    Unintentional? Consider a major bump, or revert the change."
elif [ "$ERRORS" -gt 0 ]; then
    echo "  ✓ Breaking changes present, and this is a major bump"
elif [ "$WARNINGS" -gt 0 ] && [ "$BUMP" = "patch" ]; then
    echo ""
    echo "  ⚠ ${WARNINGS} contract changes in a patch release."
    echo "    Patches should not move the API: clients decide what a server"
    echo "    supports from the release a feature first appeared in."
else
    echo "  ✓ Bump is consistent with the API changes"
fi

echo ""
echo "  Full report: oasdiff breaking ${PREV_TAG}:${SPEC} ${SPEC}"
exit 0
