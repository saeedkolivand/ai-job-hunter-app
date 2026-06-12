# Deployment — AI Job Hunter

Last updated: 2026-06-03

AI Job Hunter is distributed as a native desktop installer built by [Tauri][tauri]. There is no server to deploy — the entire app runs on the end user's machine.

---

## Build Targets

| Platform | Output Format                          | Location                                    |
| -------- | -------------------------------------- | ------------------------------------------- |
| Windows  | NSIS installer (`.exe`) + MSI (`.msi`) | `src-tauri/target/release/bundle/nsis/`     |
| macOS    | App bundle (`.app`) + DMG (`.dmg`)     | `src-tauri/target/release/bundle/macos/`    |
| Linux    | AppImage (`.AppImage`) + DEB (`.deb`)  | `src-tauri/target/release/bundle/appimage/` |

---

## Building Locally

### Prerequisites

Same as [DEVELOPMENT.md](DEVELOPMENT.md), plus platform-specific:

**Windows**: Visual Studio Build Tools + WebView2 Runtime  
**macOS**: Xcode Command Line Tools (`xcode-select --install`)  
**Linux**: `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libayatana-appindicator3-dev`

```bash
# Ubuntu/Debian
sudo apt-get install libwebkit2gtk-4.1-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Build all packages then package

```bash
# 1. Build all workspace packages
pnpm build

# 2. Create platform-specific installers
pnpm package
```

Or combined:

```bash
pnpm build && pnpm package
```

Outputs land in `apps/tauri/src-tauri/target/release/bundle/`.

### Debug vs Release

```bash
# Debug build (faster, larger, unoptimized — for testing only)
cd apps/tauri
pnpm tauri build --debug

# Release build (optimized, signed if certificates configured)
pnpm tauri build
```

---

## Release Pipeline

Releases are **automated** via [semantic-release][semantic-release] on push to `main`: a release commit versions the app, drafts the notes, and syncs version files. Building the cross-platform **installers is a separate, manual step** — the compiles are slow, so they're decoupled from the every-merge release flow. Run **Actions ▸ "🚀 Release" ▸ "Run workflow"** when you want installers for a tag (see [CI/CD Pipeline](#cicd-pipeline)).

### Commit → Version mapping

| Commit prefix                                  | Version bump    | Release notes |
| ---------------------------------------------- | --------------- | ------------- |
| `feat:`                                        | minor (`1.x.0`) | Yes           |
| `fix:`, `perf:`                                | patch (`1.0.x`) | Yes           |
| `BREAKING CHANGE` footer                       | major (`x.0.0`) | Yes           |
| `refactor:`, `docs:`, `chore:`, `ci:`, `test:` | none            | No            |

### Release configuration

`.releaserc.json` controls semantic-release behavior:

```json
{
  "branches": ["main"],
  "plugins": [
    "@semantic-release/commit-analyzer",
    "@semantic-release/release-notes-generator",
    "@semantic-release/github"
  ]
}
```

### Version sync

When semantic-release creates a new tag, a CI step automatically syncs the version to:

- `package.json` (root)
- `apps/tauri/package.json`
- `apps/tauri/src-tauri/Cargo.toml`
- `apps/tauri/src-tauri/Cargo.lock` (the `ajh-tauri` package entry — kept in lockstep so local builds don't drift)
- `apps/tauri/src-tauri/tauri.conf.json`

**Never manually bump versions.** Commit with the correct prefix and the pipeline handles it.

---

## CI/CD Pipeline

```mermaid
graph LR
    Push["git push to main"] --> Analysis["semantic-release\nanalyzes commits"]
    Analysis --> Tag["git tag + GitHub\nrelease notes"]
    Tag --> Sync["sync version files\n(commit to main)"]
    Dispatch["Manual: Actions ▸\nRun workflow\n(version or latest)"] --> Build["build matrix"]
    Build --> Windows["Windows\nNSIS + MSI"]
    Build --> Mac["macOS\nDMG + APP"]
    Build --> Linux["Linux\nAppImage + DEB"]
    Windows & Mac & Linux --> Upload["Upload installers\nto the release"]
    Upload --> UpdateServer["Tauri Updater\nlatest.json published"]
```

### GitHub Actions workflow

`.github/workflows/release.yml`. A release push publishes the release + version bump; installers are built only via the manual **Run workflow** dispatch.

**On `push` to `main`** — `release` + `sync-version-files`:

1. semantic-release analyzes commits → creates the `v*` tag + GitHub release notes
2. CI commits the synced version files back to `main`

**`build` + `generate-update-manifest`** — run **only** via **Actions ▸ "🚀 Release" ▸ "Run workflow"** (macOS Intel + Apple Silicon build as two parallel matrix legs, so wall-clock is roughly the slowest single platform rather than the sum of all three):

1. Resolve the version (the `version` input, or the latest tag if left blank), then checkout that tag
2. Install pnpm + Node + Rust stable; `pnpm build:packages`
3. `pnpm tauri build` — compiles Rust + bundles installers for Windows / macOS / Linux
4. Upload installers to the release, then generate + upload `latest.json` (the auto-updater manifest)

> Manual dispatch is for **rebuilding an existing tag** (e.g. a runner flaked, or you want to re-attach assets) — it does not create a new release. Leave the version blank for the latest tag, or pass one like `0.62.0`.

### Pull-request checks & review

PRs to `main` run three layers (all under [`.github/workflows/`](../.github/workflows/)):

- **Gating** — `ci-pipeline.yml` (lint, type-check, tests, build, Rust quality + architecture R1–R8, `cargo-deny`, dependency-review). The only layer that blocks merge.
- **Always-on AI review — CodeRabbit** (external SaaS, free on this public repo; config in [`.coderabbit.yaml`](../.coderabbit.yaml)). Posts a PR summary + walkthrough + line-by-line review, applies area labels, and runs ESLint / Clippy / Semgrep / secret-scan / actionlint inline. Advisory only — it never approves or blocks (only "✅ CI OK" gates). Its `path_instructions` mirror `.claude/review-routes.json` ownership + the `CLAUDE.md` conventions; the former `pr-review.yml` (reviewdog ESLint/Clippy + `dangerfile.ts`) and `labeler.yml` were retired in its favor.
- **Advisory checks** — `quality.yml` (typos/links/knip/i18n/a11y + Rust cargo-hack/cargo-mutants + the export-render benchmark) and `ui-checks.yml` (Playwright e2e + Lighthouse + Lost Pixel). Never block.
- **Security → Security tab** — `security.yml` consolidates CodeQL + Semgrep + OpenSSF Scorecard + the weekly npm/cargo audit (each job event-gated + least-privilege).
- **On-demand deep review — Claude** — comment `@claude review` on a PR (repo owner only) to run `claude-review.yml`, an agent-routed deep dive as the `.claude/agents` owner. Inert until invoked. Requires the `CLAUDE_CODE_OAUTH_TOKEN` repo secret (from `claude setup-token`); do **not** also set `ANTHROPIC_API_KEY`.

> CodeRabbit reviews **fork** PRs too (it's a GitHub App, not a `GITHUB_TOKEN` job); all PRs still hit the gating layer. CodeQL **Default setup** must stay off; the advanced CodeQL job in `security.yml` conflicts with it. See [`docs/adr/0003-consolidate-ci-workflows.md`](adr/0003-consolidate-ci-workflows.md).

---

## Auto-Update

The app checks for updates on launch via Tauri's updater plugin. The update manifest is published to GitHub Releases automatically.

### How it works

1. App starts → calls `updater.check()` via IPC
2. Tauri updater fetches the release manifest from GitHub
3. If a newer version exists → `UpdateBanner` appears in the UI
4. User clicks "Update" → `updater.downloadAndInstall()` → app restarts

### Disabling auto-update check

In `apps/tauri/src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "active": false
    }
  }
}
```

### Updater signing keys

Every release artifact the updater consumes (NSIS `.exe`, Linux `.AppImage`, macOS `.app.tar.gz`) is signed with a **minisign** key. The shipped app verifies each downloaded update against the public key baked into it.

There are exactly two halves of **one** key pair, and they must always match:

| Half        | Where it lives                                                     | Secret? |
| ----------- | ------------------------------------------------------------------ | ------- |
| Private key | GitHub secret `TAURI_SIGNING_PRIVATE_KEY` (+ `…_PASSWORD`) — signs | Yes     |
| Public key  | `plugins.updater.pubkey` in `tauri.conf.json` — verifies           | No      |

The public key is **committed in `tauri.conf.json` as the single source of truth.** CI does not inject it — `scripts/sync-tauri-version.cjs` only syncs version numbers. If the committed public key ever stops matching `TAURI_SIGNING_PRIVATE_KEY`, every shipped update fails at download with `invalid encoding in minisign data` (or a signature error), because the app cannot verify an artifact signed by an unknown key.

`scripts/verify-updater-key.cjs` runs in the release build and **fails the build before publishing** if a freshly-signed artifact's key id does not match the committed public key — so this can never silently regress.

#### Rotating the key

1. Generate a new pair: `bash scripts/generate-tauri-signing-key.sh`
2. Set the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to the new private key + password.
3. Put the matching public key (contents of `~/.tauri/ajh.key.pub`) into `plugins.updater.pubkey` in `tauri.conf.json` and commit it.
4. Cut a release. The CI guard confirms the pair matches.

> **One-time break across a rotation:** users on a build signed by the _old_ key cannot auto-update to a release signed by the _new_ key — their app only trusts the old public key. They must download and reinstall once. Every release after that auto-updates normally.

---

## Code Signing

### Windows

Signing requires a code signing certificate. Set these env vars in CI:

```
TAURI_SIGNING_PRIVATE_KEY      base64-encoded private key
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

### macOS

Requires Apple Developer certificate:

```
APPLE_CERTIFICATE           base64-encoded .p12
APPLE_CERTIFICATE_PASSWORD
APPLE_ID                    Apple ID for notarization
APPLE_PASSWORD              App-specific password
APPLE_TEAM_ID
```

### Linux

No signing required for AppImage/DEB.

---

## App Identifier

The app identifier is set in `apps/tauri/src-tauri/tauri.conf.json`:

```json
{
  "identifier": "com.ajh.desktop"
}
```

This identifier is used for:

- OS keychain credential namespacing
- App data directory location
- macOS bundle ID
- Windows registry entries

**Do not change this** in a released app — it will cause users to lose their stored data and credentials.

---

## Data Directory

The app stores all user data in the OS app data directory:

| Platform | Path                                           |
| -------- | ---------------------------------------------- |
| Windows  | `%APPDATA%\ai-job-hunter\`                     |
| macOS    | `~/Library/Application Support/ai-job-hunter/` |
| Linux    | `~/.local/share/ai-job-hunter/`                |

Contents:

```
ai-job-hunter/
├── app.db          ← SQLite database
├── vectors/        ← LanceDB vector store
└── logs/           ← Pino log files
```

---

## Diagnostics in Production

The app includes built-in diagnostic tools accessible from Settings → Support:

- **Log export**: Downloads a ZIP of recent log files
- **Health check**: Tests Ollama connectivity, DB integrity
- **Reset tools**: Clear cache, reimport documents, factory reset

These are useful for end-user support without needing a remote logging system.

[tauri]: https://tauri.app
[semantic-release]: https://github.com/semantic-release/semantic-release
