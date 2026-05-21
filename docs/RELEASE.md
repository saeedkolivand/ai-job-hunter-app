# Release Guide — AI Job Hunter

Releases are **fully automated**. Push a qualifying commit to `main` and the pipeline does the rest.

---

## How Releases Work

```
push to main
    ↓
release.yml
    semantic-release analyzes commits
    → creates GitHub Release + git tag
    → updates CHANGELOG.md + bumps package.json version
    → commits [skip ci] release commit
    ↓
build.yml (triggered by release.yml)
    build-windows   → NSIS .exe installer
    build-linux     → AppImage + .deb
    build-macos     → .dmg + .zip
    ↓
attach-to-release
    uploads all installers to the GitHub Release
```

---

## Commit Types That Trigger a Release

| Commit prefix                                                      | Version bump          | Example                           |
| ------------------------------------------------------------------ | --------------------- | --------------------------------- |
| `feat:`                                                            | **minor** (1.**x**.0) | `feat(jobs): add date filter`     |
| `fix:`, `perf:`                                                    | **patch** (1.0.**x**) | `fix(ui): correct button padding` |
| `BREAKING CHANGE` in footer                                        | **major** (**x**.0.0) | `feat!: redesign IPC contract`    |
| `refactor:`, `docs:`, `chore:`, `ci:`, `test:`, `style:`, `build:` | **no release**        | `chore(deps): upgrade tauri`      |

---

## Rules

- **Never manually tag releases** — let semantic-release control the version
- **Never manually edit CHANGELOG.md or bump package.json version** — semantic-release does this automatically via the `[skip ci]` release commit
- **Never rewrite history on `main`** — semantic-release relies on the full commit history

---

## GitHub Secrets Required

| Secret         | Purpose                                                            |
| -------------- | ------------------------------------------------------------------ |
| `GITHUB_TOKEN` | Auto-provided by GitHub Actions — creates releases, uploads assets |

No additional secrets are required for unsigned builds. For macOS code signing, you would add `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID` secrets — but these are optional (unsigned builds work; users bypass Gatekeeper).

---

## Manual Release (emergency only)

If you need to trigger a release manually without a new commit:

```bash
# Run semantic-release locally (dry run first to check what would happen)
pnpm exec semantic-release --dry-run

# Only do the real run if you're sure
GITHUB_TOKEN=<your-pat> pnpm exec semantic-release
```

Or use the GitHub Actions UI:

1. Go to **Actions → Release**
2. Click **Run workflow** → `main`

---

## Build Artifacts

After a release, installers appear on the [GitHub Releases](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases) page:

| File                            | Platform                 |
| ------------------------------- | ------------------------ |
| `AI-Job-Hunter-Setup-x.y.z.exe` | Windows (NSIS installer) |
| `AI-Job-Hunter-x.y.z.AppImage`  | Linux (portable)         |
| `ai-job-hunter_x.y.z_amd64.deb` | Linux (Debian/Ubuntu)    |
| `AI-Job-Hunter-x.y.z.dmg`       | macOS (disk image)       |
| `AI-Job-Hunter-x.y.z-mac.zip`   | macOS (zip archive)      |

---

## Tauri Build Config

`apps/tauri/src-tauri/tauri.conf.json` controls packaging:

- `bundle.identifier` — app bundle ID
- `bundle.targets` — `nsis`, `msi` (Windows), `dmg`, `app` (macOS), `deb`, `appimage` (Linux)
- `plugins.updater` — GitHub release endpoint for auto-updates

The `package` script at root runs:

```bash
pnpm build:packages && pnpm --filter @ajh/tauri package
```
