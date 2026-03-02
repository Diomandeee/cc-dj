#!/usr/bin/env bash
# Validate that all command IDs in commands.yaml are unique.
# Usage: ./configs/validate_commands.sh [path/to/commands.yaml]
#
# Exit codes:
#   0 — all IDs unique
#   1 — duplicate IDs found

set -euo pipefail

FILE="${1:-configs/commands.yaml}"

if [ ! -f "$FILE" ]; then
    echo "ERROR: $FILE not found"
    exit 1
fi

# Extract id fields and check for duplicates
DUPES=$(grep -E '^\s*id:\s*' "$FILE" | sed 's/.*id:\s*//' | tr -d '"' | tr -d "'" | sort | uniq -d)

if [ -n "$DUPES" ]; then
    echo "ERROR: Duplicate command IDs found in $FILE:"
    echo "$DUPES"
    exit 1
fi

COUNT=$(grep -cE '^\s*id:\s*' "$FILE" || true)
echo "OK: $COUNT command IDs, all unique"
