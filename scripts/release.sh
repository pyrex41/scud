#!/bin/bash
# Full release script - bumps version, commits, tags, and publishes
# Usage: ./scripts/release.sh <new-version>
# Example: ./scripts/release.sh 1.22.0

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 1.22.0"
    exit 1
fi

NEW_VERSION="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --staged --quiet; then
    echo "❌ Error: You have uncommitted changes. Please commit or stash them first."
    exit 1
fi

echo "🚀 Starting release process for v$NEW_VERSION"
echo ""

# 1. Bump version
echo "📦 Step 1: Bumping version..."
"$SCRIPT_DIR/bump-version.sh" "$NEW_VERSION"

# 2. Run tests
echo ""
echo "🧪 Step 2: Running tests..."
(cd scud-cli && cargo test --quiet)
echo "✅ Tests passed"

# 3. Run clippy
echo ""
echo "📋 Step 3: Running clippy..."
(cd scud-cli && cargo clippy --all-targets --all-features -- -D warnings)
echo "✅ Clippy passed"

# 4. Format
echo ""
echo "🎨 Step 4: Formatting..."
(cd scud-cli && cargo fmt)
echo "✅ Formatted"

# 5. Commit
echo ""
echo "📝 Step 5: Committing..."
git add -A
git commit -m "chore: bump version to $NEW_VERSION"

# 6. Push
echo ""
echo "⬆️  Step 6: Pushing to origin..."
git push origin master

# 7. Tag
echo ""
echo "🏷️  Step 7: Creating tag..."
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"
git push origin "v$NEW_VERSION"

# 8. Wait for CI
echo ""
echo "⏳ Step 8: Waiting for CI/CD..."
echo "   Checking test workflow..."
sleep 5

MAX_WAIT=300  # 5 minutes
WAITED=0
while [ $WAITED -lt $MAX_WAIT ]; do
    STATUS=$(gh run list --workflow=test.yml --limit 1 --json status --jq '.[0].status' 2>/dev/null || echo "unknown")
    if [ "$STATUS" = "completed" ]; then
        CONCLUSION=$(gh run list --workflow=test.yml --limit 1 --json conclusion --jq '.[0].conclusion' 2>/dev/null || echo "unknown")
        if [ "$CONCLUSION" = "success" ]; then
            echo "✅ CI tests passed"
            break
        else
            echo "❌ CI tests failed! Check: gh run list --limit 1"
            exit 1
        fi
    fi
    echo "   Waiting... ($WAITED/$MAX_WAIT seconds)"
    sleep 10
    WAITED=$((WAITED + 10))
done

if [ $WAITED -ge $MAX_WAIT ]; then
    echo "⚠️  CI timeout - check manually: gh run list --limit 1"
    read -p "Continue with publish anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# 9. Publish to crates.io
echo ""
echo "📦 Step 9: Publishing to crates.io..."
(cd scud-cli && cargo publish)
echo "✅ Published to crates.io"

# 10. Publish to npm
echo ""
echo "📦 Step 10: Publishing to npm..."
npm publish
echo "✅ Published to npm"

echo ""
echo "🎉 Release v$NEW_VERSION complete!"
echo ""
echo "Users can now update with:"
echo "  cargo install scud-cli --force"
echo "  # or"
echo "  pnpm install -g scud-task@latest --force"
