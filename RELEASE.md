# Release Process

This document describes how to release a new version of scud-task.

## Prerequisites

1. Ensure all changes are committed and pushed
2. Update version in `package.json` and `scud-cli/Cargo.toml`
3. Update CHANGELOG.md (if exists)

## Release Steps

### 1. Create a Git Tag

```bash
# For version 1.0.0
git tag v1.0.0
git push origin v1.0.0
```

### 2. GitHub Actions Builds Binaries

When you push a tag starting with `v`, GitHub Actions will automatically:
- Build Rust binaries for all platforms:
  - macOS x64
  - macOS ARM64 (Apple Silicon)
  - Linux x64
  - Linux ARM64
  - Windows x64
- Create a GitHub Release
- Upload all binaries as release assets

**Check progress:** https://github.com/pyrex41/scud/actions

### 3. Publish to npm

Once the GitHub Release is created and binaries are uploaded:

```bash
npm login              # If not already logged in
npm publish
```

The postinstall script will automatically download the appropriate binary for each user's platform.

## Manual Release (if needed)

If you need to trigger a release without a tag:

1. Go to: https://github.com/pyrex41/scud/actions/workflows/release.yml
2. Click "Run workflow"
3. Select the branch
4. Click "Run workflow"

## Testing Before Release

Test the installation locally:

```bash
# Pack without publishing
npm pack

# Install locally in another directory
cd /tmp
npm install -g /path/to/scud-task-1.0.0.tgz

# Test commands
scud help
scud init
scud tags
```

## Troubleshooting

### Binary Download Fails

If users can't download binaries, they can build from source:
```bash
cd node_modules/scud-task/scud-cli
cargo build --release
```

### GitHub Actions Build Fails

Check the Actions tab: https://github.com/pyrex41/scud/actions

Common issues:
- Rust compilation errors (check scud-cli/src code)
- Cross-compilation tools not installed (check workflow file)
- Missing permissions (check GitHub repo settings)

## Platform Support

Currently supported platforms:
- macOS (Intel & Apple Silicon)
- Linux (x64 & ARM64)
- Windows (x64)

To add more platforms, update `.github/workflows/release.yml` matrix.
