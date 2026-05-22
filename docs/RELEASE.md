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

| Secret                               | Purpose                                                            |
| ------------------------------------ | ------------------------------------------------------------------ |
| `GITHUB_TOKEN`                       | Auto-provided by GitHub Actions — creates releases, uploads assets |
| `TAURI_SIGNING_PRIVATE_KEY`          | Minisign private key for signing Tauri artifacts (optional)        |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for the private key (empty string if none)                |
| `TAURI_SIGNING_PUBLIC_KEY`           | Minisign public key for verifying updates                          |
| `DISCORD_WEBHOOK_URL`                | Optional: Discord webhook for release notifications                |

### Setting Up Tauri Signing (Optional but Recommended)

To enable artifact signing for secure updates:

1. Generate a minisign key pair:

   ```bash
   bash scripts/generate-tauri-signing-key.sh
   ```

2. Add the following secrets to your GitHub repository:
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.tauri/ajh.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — password you entered (or empty string)
   - `TAURI_SIGNING_PUBLIC_KEY` — contents of `~/.tauri/ajh.key.pub`

3. The CI pipeline automatically injects the public key into `tauri.conf.json` during builds.

**Note:** The workflow will work without signing secrets (unsigned builds), but signed builds provide better security for auto-updates.

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
