#!/bin/bash
# Bump version in all places consistently
# Usage: ./scripts/bump-version.sh <new-version>
# Example: ./scripts/bump-version.sh 1.22.0

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 1.22.0"
    exit 1
fi

NEW_VERSION="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Bumping version to $NEW_VERSION..."

# 1. Update Cargo.toml
echo "  → scud-cli/Cargo.toml"
sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$ROOT_DIR/scud-cli/Cargo.toml"

# 2. Update Cargo.lock (by running cargo check)
echo "  → scud-cli/Cargo.lock"
(cd "$ROOT_DIR/scud-cli" && cargo check --quiet 2>/dev/null)

# 3. Update package.json
echo "  → package.json"
sed -i '' "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" "$ROOT_DIR/package.json"

echo ""
echo "✅ Version bumped to $NEW_VERSION in:"
echo "   - scud-cli/Cargo.toml"
echo "   - scud-cli/Cargo.lock"
echo "   - package.json"
echo ""
echo "Next steps:"
echo "  1. git add -A && git commit -m 'chore: bump version to $NEW_VERSION'"
echo "  2. git push origin master"
echo "  3. git tag -a v$NEW_VERSION -m 'Release v$NEW_VERSION'"
echo "  4. git push origin v$NEW_VERSION"
echo "  5. Wait for CI/CD tests to pass"
echo "  6. cargo publish (in scud-cli/)"
echo "  7. npm publish (in root)"
