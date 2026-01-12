# Publishing to crates.io

## Automated CI/CD

SCUD uses GitHub Actions for automatic publishing to [crates.io](https://crates.io/crates/scud-cli) when version tags are created.

## Setup

1. **Create a crates.io API token:**
   - Go to [crates.io/me](https://crates.io/me) → API Tokens
   - Create a new token with publish permissions
   - Copy the token

2. **Add to GitHub secrets:**
   - Go to your GitHub repository → Settings → Secrets and variables → Actions
   - Add a new repository secret named `CARGO_REGISTRY_TOKEN`
   - Paste your crates.io API token as the value

## Publishing Process

1. **Update version in `scud-cli/Cargo.toml`:**
   ```toml
   version = "1.32.0"
   ```

2. **Commit the version bump:**
   ```bash
   git add scud-cli/Cargo.toml
   git commit -m "chore: bump version to 1.32.0"
   ```

3. **Create and push a version tag:**
   ```bash
   git tag v1.32.0
   git push origin v1.32.0
   ```

4. **CI/CD will automatically:**
   - Verify the tag version matches Cargo.toml
   - Build and test the crate
   - Publish to crates.io

## Manual Publishing (if needed)

If you need to publish manually:

```bash
cd scud-cli
cargo publish --token YOUR_API_TOKEN
```

## Version Format

- Tags must follow the format: `v{major}.{minor}.{patch}` (e.g., `v1.32.0`)
- Version in `Cargo.toml` must match the tag (without the `v` prefix)
- Follow [semantic versioning](https://semver.org/)

## Troubleshooting

- **Version mismatch:** Ensure the tag version matches `Cargo.toml` exactly
- **Build failures:** Check that all tests pass locally with `cargo test`
- **Publishing fails:** Verify your API token has publish permissions