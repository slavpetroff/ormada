# Publishing Guide

This document describes the automated publishing workflow for Ormada.

## Automated Publishing

The project uses GitHub Actions to automatically publish new versions to crates.io when changes are merged to the `main` branch.

### How It Works

1. **Version Detection**: When code is pushed to `main`, the workflow checks if the version in `Cargo.toml` has changed
2. **Testing**: If version changed, runs full test suite (formatting, clippy, tests)
3. **Publishing**: If tests pass, publishes both `ormada-derive` and `ormada` to crates.io
4. **Release**: Creates a GitHub release with the version tag

### Required Setup

To enable automated publishing, you need to set up a GitHub secret:

1. Go to your repository settings → Secrets and variables → Actions
2. Create a new repository secret named `CARGO_REGISTRY_TOKEN`
3. Set the value to your crates.io API token (get it from https://crates.io/settings/tokens)

### Publishing a New Version

1. Update the version in all `Cargo.toml` files:
   - `Cargo.toml` (main ormada crate)
   - `derive/Cargo.toml`
   - `schema/Cargo.toml`
   - `cli/Cargo.toml`
2. Commit the changes: `git commit -am "Bump version to X.Y.Z"`
3. Push to main (or merge a PR): `git push origin main`
4. The workflow will automatically:
   - Run all tests
   - Publish to crates.io (in dependency order)
   - Create a GitHub release

### Manual Publishing

If you need to publish manually, follow the dependency order:

```bash
# 1. Publish ormada-derive first (no dependencies)
cargo publish -p ormada-derive
sleep 30

# 2. Publish ormada-schema (no dependencies)
cargo publish -p ormada-schema
sleep 30

# 3. Publish ormada (depends on ormada-derive)
cargo publish -p ormada
sleep 30

# 4. Publish ormada-cli (depends on ormada-schema)
cargo publish -p ormada-cli
```

## Continuous Integration

Every pull request and push to **any branch** triggers:

- **Format Check**: Ensures code is properly formatted
- **Clippy**: Runs linter checks
- **Tests**: Runs full test suite on stable and beta Rust
- **Coverage**: Generates code coverage report
- **Documentation**: Verifies docs build without warnings
- **MSRV**: Checks compatibility with Rust 1.75

## Security

Security audits run automatically on:
- **Weekly schedule**: Every Monday at 9:00 AM UTC
- **Pull requests**: On all PRs
- **Pushes to main**: After merges

Checks performed:
- **cargo-audit**: Checks for known security vulnerabilities in dependencies
- **cargo-deny**: Validates licenses and checks for security advisories

Note: The workflow automatically generates `Cargo.lock` if needed and handles workspace configurations.

## Workflows

- **`ci.yml`**: Runs on all PRs and pushes to any branch
- **`publish.yml`**: Publishes to crates.io when version changes on main
- **`security.yml`**: Security audits (weekly + on PRs and main pushes)

## Version Bumping Strategy

Follow semantic versioning:

- **Patch** (0.1.X): Bug fixes, no breaking changes
- **Minor** (0.X.0): New features, backward compatible
- **Major** (X.0.0): Breaking changes

Update all files when bumping version:
- `/Cargo.toml`
- `/derive/Cargo.toml`
- `/schema/Cargo.toml`
- `/cli/Cargo.toml`
