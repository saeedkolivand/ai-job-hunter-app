# Tauri Updater Setup Guide

## Problem

The updater is failing with two errors:

1. **"check failed: missing field 'signature'"** - When checking for updates
2. **"invalid encoding in minisign data"** - When downloading updates

## Root Cause

Tauri v2's updater plugin requires:

1. A properly formatted `latest.json` file in GitHub releases
2. `.sig` signature files for each platform installer
3. The public key must match the private key used to sign releases

## Solution

### Step 1: Generate Signing Keys (One-Time Setup)

```bash
# Install tauri-cli if not already installed
cargo install tauri-cli

# Generate a new key pair (only do this ONCE)
cd apps/tauri/src-tauri
tauri signer generate -w ~/.tauri/myapp.key

# This creates:
# - Private key: ~/.tauri/myapp.key (KEEP SECRET!)
# - Public key: printed to console (add to tauri.conf.json)
```

**Important:** Save the private key securely! You'll need it for every release.

### Step 2: Update tauri.conf.json

Replace the `pubkey` in `apps/tauri/src-tauri/tauri.conf.json` with your new public key:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "YOUR_NEW_PUBLIC_KEY_HERE",
      "endpoints": [
        "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/latest/download/latest.json"
      ]
    }
  }
}
```

### Step 3: Sign Your Releases

When building for release, sign each platform installer:

```bash
# Build the app
pnpm tauri build

# Sign each installer (do this for EVERY platform)
# Windows (.msi)
tauri signer sign \
  "apps/tauri/src-tauri/target/release/bundle/msi/AI Job Hunter Assistant_1.3.1_x64_en-US.msi" \
  -k ~/.tauri/myapp.key

# Windows (.exe)
tauri signer sign \
  "apps/tauri/src-tauri/target/release/bundle/nsis/AI Job Hunter Assistant_1.3.1_x64-setup.exe" \
  -k ~/.tauri/myapp.key

# macOS (.dmg)
tauri signer sign \
  "apps/tauri/src-tauri/target/release/bundle/dmg/AI Job Hunter Assistant_1.3.1_x64.dmg" \
  -k ~/.tauri/myapp.key

# macOS (.app)
tauri signer sign \
  "apps/tauri/src-tauri/target/release/bundle/macos/AI Job Hunter Assistant.app" \
  -k ~/.tauri/myapp.key
```

This creates `.sig` files next to each installer:

- `AI Job Hunter Assistant_1.3.1_x64_en-US.msi.sig`
- `AI Job Hunter Assistant_1.3.1_x64-setup.exe.sig`
- etc.

### Step 4: Create latest.json

Create a `latest.json` file with this format:

```json
{
  "version": "1.3.1",
  "notes": "Release notes go here",
  "pub_date": "2026-05-22T09:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "CONTENT_OF_MSI_SIG_FILE",
      "url": "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/download/v1.3.1/AI_Job_Hunter_Assistant_1.3.1_x64_en-US.msi"
    },
    "darwin-x86_64": {
      "signature": "CONTENT_OF_DMG_SIG_FILE",
      "url": "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/download/v1.3.1/AI_Job_Hunter_Assistant_1.3.1_x64.dmg"
    },
    "darwin-aarch64": {
      "signature": "CONTENT_OF_AARCH64_DMG_SIG_FILE",
      "url": "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/download/v1.3.1/AI_Job_Hunter_Assistant_1.3.1_aarch64.dmg"
    }
  }
}
```

**To get signature content:**

```bash
cat "AI Job Hunter Assistant_1.3.1_x64_en-US.msi.sig"
# Copy the entire output into the "signature" field
```

### Step 5: Upload to GitHub Release

1. Create a new GitHub release (e.g., `v1.3.1`)
2. Upload ALL files:
   - `AI Job Hunter Assistant_1.3.1_x64_en-US.msi`
   - `AI Job Hunter Assistant_1.3.1_x64_en-US.msi.sig`
   - `AI Job Hunter Assistant_1.3.1_x64-setup.exe`
   - `AI Job Hunter Assistant_1.3.1_x64-setup.exe.sig`
   - `AI Job Hunter Assistant_1.3.1_x64.dmg`
   - `AI Job Hunter Assistant_1.3.1_x64.dmg.sig`
   - `latest.json`
3. Publish the release

## Alternative: Disable Signature Verification (NOT RECOMMENDED)

If you want to disable signature verification for testing:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "",
      "endpoints": [
        "https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/latest/download/latest.json"
      ]
    }
  }
}
```

**Warning:** This makes your app vulnerable to man-in-the-middle attacks!

## Automated Release Script

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        platform: [macos-latest, ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies
        run: pnpm install

      - name: Build Tauri app
        run: pnpm tauri build
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}

      - name: Upload Release Assets
        uses: softprops/action-gh-release@v1
        with:
          files: |
            apps/tauri/src-tauri/target/release/bundle/**/*.msi
            apps/tauri/src-tauri/target/release/bundle/**/*.exe
            apps/tauri/src-tauri/target/release/bundle/**/*.dmg
            apps/tauri/src-tauri/target/release/bundle/**/*.app
            apps/tauri/src-tauri/target/release/bundle/**/*.sig
```

Add secrets to GitHub repository settings:

- `TAURI_SIGNING_PRIVATE_KEY` - Content of `~/.tauri/myapp.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` - Password if you set one

## Testing

```bash
# Test update check locally
pnpm tauri dev

# In the app, go to Settings → Check for Updates
```

## References

- [Tauri Updater Plugin Docs](https://v2.tauri.app/plugin/updater/)
- [Tauri Signer CLI](https://v2.tauri.app/reference/cli/#signer)
- [GitHub Actions for Tauri](https://tauri.app/v1/guides/building/cross-platform/)
