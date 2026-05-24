# macOS Build Configuration

## Current Status: No Private APIs on macOS

The app uses **standard macOS APIs** to avoid Apple Developer Program requirements and code signing complexity.

## Platform-Specific Configuration

### macOS

- **Private APIs**: Disabled (`macOSPrivateApi: false`)
- **Window decorations**: Standard macOS title bar
- **Code signing**: Ad-hoc (no Apple Developer certificate required)
- **Entitlements**: Not needed
- **Distribution**: Can be distributed without Apple notarization

### Windows & Linux

- **Private APIs**: Enabled (via target-specific Cargo features)
- **Window decorations**: Custom title bar with overlay style
- **Code signing**: Standard platform signing (optional)

## Technical Details

### Cargo.toml Configuration

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "image-ico", "image-png"] }

[target.'cfg(windows)'.dependencies]
tauri = { version = "2", features = ["macos-private-api"] }

[target.'cfg(target_os = "linux")'.dependencies]
tauri = { version = "2", features = ["macos-private-api"] }
```

### tauri.conf.json Configuration

```json
{
  "app": {
    "macOSPrivateApi": false,
    "windows": [
      {
        "decorations": false,
        "titleBarStyle": "Overlay"
      }
    ]
  }
}
```

**Note**: On macOS, `decorations: false` with `macOSPrivateApi: false` will use standard window decorations (native title bar).

## Building for macOS

### Local Development

#### Intel (x86_64)

```bash
cd apps/tauri
pnpm tauri build --target x86_64-apple-darwin
```

#### Apple Silicon (ARM64)

```bash
cd apps/tauri
rustup target add aarch64-apple-darwin
pnpm tauri build --target aarch64-apple-darwin
```

The resulting `.app` bundle can be opened directly on your Mac without any special signing.

### CI/CD Builds

The release workflow builds **separate artifacts** for both architectures and renames them with the version:

- **Intel (x86_64)**: `AI-Job-Hunter-Assistant-{version}-intel.dmg`
- **Apple Silicon (ARM64)**: `AI-Job-Hunter-Assistant-{version}-apple-silicon.dmg`

Example for version 0.2.13:

- `AI-Job-Hunter-Assistant-0.2.13-intel.dmg`
- `AI-Job-Hunter-Assistant-0.2.13-apple-silicon.dmg`

Both are uploaded to GitHub Releases. Users download the appropriate version for their Mac:

- Intel Macs (2019 and earlier) → `intel` version
- Apple Silicon Macs (M1/M2/M3 and later) → `apple-silicon` version

**No Apple Developer secrets required** - builds use standard ad-hoc signing.

## Future: Enabling Custom UI on macOS

If you want custom window decorations on macOS in the future, you'll need to:

1. Join Apple Developer Program ($99/year)
2. Enable `macos-private-api` feature for macOS target
3. Set `macOSPrivateApi: true` in tauri.conf.json
4. Create entitlements.plist file
5. Add Apple Developer certificate and notarization to CI workflow
6. Configure code signing identity

See the original commit history for the removed entitlements and workflow configuration if needed.

## References

- [Tauri macOS Private API](https://tauri.app/v2/guides/features/private-api/)
- [Apple Developer Program](https://developer.apple.com/programs/)
