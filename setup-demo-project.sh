#!/usr/bin/env bash
# Reset /tmp/descartes-test from the descartes-test/ template directory
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="$SCRIPT_DIR/descartes-test"
DEST="/tmp/descartes-test"

rm -rf "$DEST"
cp -R "$SRC" "$DEST"

echo "Demo project reset at $DEST"
ls -la "$DEST"
