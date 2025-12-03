# Releasing SCUD

SCUD has **two packages** that must be released together with matching versions:

1. **npm**: `scud-task` (JavaScript wrapper, slash commands, install script)
2. **crates.io**: `scud-cli` (Rust CLI binary)

## Version Files to Update

Both files must have the same version number:

```
package.json          → "version": "X.Y.Z"
scud-cli/Cargo.toml   → version = "X.Y.Z"
```

## Release Checklist

1. **Update versions** in both files:
   - `package.json` (npm)
   - `scud-cli/Cargo.toml` (cargo/crates.io)

2. **Commit the version bump**:
   ```bash
   git add package.json scud-cli/Cargo.toml
   git commit -m "chore: bump version to X.Y.Z"
   ```

3. **Create and push tag**:
   ```bash
   git tag vX.Y.Z
   git push origin master
   git push origin vX.Y.Z
   ```

4. **CI/CD automatically publishes**:
   - GitHub Actions `Release` workflow triggers on tag push
   - Publishes to npm (scud-task)
   - Publishes to crates.io (scud-cli)
   - Creates GitHub Release

5. **Verify both packages**:
   ```bash
   npm view scud-task version      # Should show X.Y.Z
   cargo search scud-cli           # Should show X.Y.Z
   ```

## Common Mistakes

- **Forgetting to bump `package.json`**: npm publish fails with E403 "cannot publish over previously published version"
- **Forgetting to bump `Cargo.toml`**: crates.io publish fails, or `scud --version` shows old version
- **Version mismatch**: Keep both files in sync to avoid confusion

## GitHub Secrets Required

The release workflow requires these secrets in GitHub repository settings:

- `NPM_TOKEN` - npm access token for publishing scud-task
- `CARGO_TOKEN` - crates.io API token for publishing scud-cli

## Installing After Release

```bash
# npm (installs JS wrapper + slash commands)
pnpm add -g scud-task@latest

# cargo (installs Rust CLI)
cargo install scud-cli

# Or rebuild locally
cd scud-cli && cargo install --path .
```
