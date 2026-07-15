# Release Process

This document describes how to publish a new release of SPMP8000Emu.

## Prerequisites

- Push access to the `master` branch
- Permission to create tags and releases on GitHub

## Steps

### 1. Update version numbers

Version numbers must be updated in the workspace `Cargo.toml`:

```bash
# Example: bumping to 0.2.0
sed -i 's/^version = "0.1.0"/version = "0.2.0"/' Cargo.toml
```

### 2. Commit the version bump

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.2.0"
git push origin master
```

### 3. Create and push a tag

The release workflow triggers on tags matching `v*` (e.g. `v0.2.0`).

```bash
git tag v0.2.0
git push origin v0.2.0
```

### 4. CI builds and creates a release

Pushing the tag triggers the CI workflow, which:

1. Builds standalone binaries for Windows, macOS, and Linux
2. Builds libretro cores for all supported platforms
3. Creates a GitHub Release with build artifacts

### 5. Review and publish the release

1. Go to [Releases](https://github.com/jiangxincode/SPMP8000Emu/releases)
2. Find the release created by CI
3. Review the auto-generated changelog — edit if needed
4. Verify all expected artifacts are attached
5. Click **Publish release**

## Troubleshooting

### CI build fails

- Check the [Actions](https://github.com/jiangxincode/SPMP8000Emu/actions) tab
  for the failed run

### Re-triggering a release

The release workflow only runs on tag pushes. To re-trigger:

```bash
# 1. Delete the tag locally and remotely
git tag -d v0.2.0
git push origin --delete v0.2.0

# 2. Re-push the tag
git push origin v0.2.0
```
