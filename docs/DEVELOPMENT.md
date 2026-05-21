# Development Guide — AI Job Hunter

---

## Prerequisites

| Tool    | Version   | Install                                                   |
| ------- | --------- | --------------------------------------------------------- |
| Node.js | ≥ 20.11.0 | [nodejs.org](https://nodejs.org)                          |
| pnpm    | ≥ 11      | `npm install -g pnpm`                                     |
| Rust    | stable    | [rustup.rs](https://rustup.rs)                            |
| Ollama  | Latest    | [ollama.ai](https://ollama.ai) — required for AI features |
| Git     | Any       | With commit hook support                                  |

---

## First-Time Setup

```bash
# 1. Clone
git clone https://github.com/saeedkolivand/ai-job-hunter-assistant-app.git
cd ai-job-hunter-assistant-app

# 2. Install dependencies (also installs git hooks via husky)
pnpm install

# 3. Pull an Ollama model
ollama pull llama3.2
```

---

## Daily Commands

### Development

```bash
# Start the Tauri app in dev mode (hot reload on renderer changes)
pnpm dev

# Start only the renderer (Vite dev server, no Tauri shell)
pnpm --filter @ajh/tauri dev:renderer
```

### Build

```bash
# Build all packages then the Tauri app
pnpm build

# Build only the packages (Tauri must already have packages built)
pnpm build:packages
```

### Type checking

```bash
# Type-check the entire monorepo
pnpm typecheck

# Type-check one package
pnpm --filter @ajh/tauri typecheck
pnpm --filter @ajh/shared typecheck
```

### Linting

```bash
# Lint all packages
pnpm lint

# Lint with auto-fix
pnpm lint --fix
```

### Tests

```bash
# Run all package tests
pnpm -r test

# Run tests for one package
pnpm --filter @ajh/data test

# Watch mode
pnpm --filter @ajh/data test -- --watch
```

### Storybook (UI package)

```bash
pnpm --filter @ajh/ui storybook
```

---

## Monorepo Scripts

All scripts are defined at the root `package.json` and delegate to packages via `pnpm -r` (recursive) or `--filter`.

| Script           | What it does                      |
| ---------------- | --------------------------------- |
| `pnpm dev`       | Start Tauri app + Vite dev server |
| `pnpm build`     | Build all packages then Tauri     |
| `pnpm typecheck` | Type-check all packages           |
| `pnpm lint`      | ESLint across all packages        |
| `pnpm -r test`   | Run Vitest in all packages        |

---

## Package Builds

Packages must be built before the Tauri app can import them. The root `pnpm build` handles ordering automatically. If you change a package during development, rebuild it:

```bash
pnpm --filter @ajh/shared build   # most common — after changing IPC contracts
pnpm --filter @ajh/prompts build  # after changing prompt templates
pnpm --filter @ajh/ui build       # after changing design tokens
```

In dev mode (`pnpm dev`), Vite watches `packages/` so most changes are picked up automatically.

---

## Environment & Configuration

The app is fully local — no `.env` file is required for basic development.

| Setting                                  | Location                                           |
| ---------------------------------------- | -------------------------------------------------- |
| User preferences (language, model, etc.) | `localStorage` key `ai-job-hunter-preferences`     |
| Ollama endpoint                          | Hardcoded to `http://localhost:11434` in `@ajh/ai` |
| App data (DB, vector store)              | Tauri `appData` directory                          |
| Encrypted credentials                    | OS keychain via keyring crate                      |

**App data paths:**

| OS      | Path                                                        |
| ------- | ----------------------------------------------------------- |
| Windows | `%APPDATA%\ai-job-hunter-assistant-app`                     |
| macOS   | `~/Library/Application Support/ai-job-hunter-assistant-app` |
| Linux   | `~/.config/ai-job-hunter-assistant-app`                     |

---

## Git Hooks (Husky)

Hooks run automatically on commit and push:

| Hook         | Runs                                                |
| ------------ | --------------------------------------------------- |
| `pre-commit` | `lint-staged` — Prettier + ESLint on staged files   |
| `commit-msg` | `commitlint` — enforces Conventional Commits format |
| `pre-push`   | `pnpm lint` + `pnpm -r test`                        |

### Commit format

```
type(scope): lowercase description

feat(ui): add dark mode toggle
fix(ipc): handle missing payload in listInteractions
docs(arch): update architecture for onboarding wizard
chore(deps): upgrade tauri to latest
```

Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `style`, `perf`, `ci`.  
Subject must be **all lowercase** (commitlint enforces `subject-case`).

---

## Debugging

### Renderer (DevTools)

In dev mode, DevTools open automatically. Press `Ctrl+Shift+I` / `Cmd+Option+I` to toggle.

### Tauri / Rust side

Logs go to stdout via pino — structured JSON by default, pretty-printed in dev.

### IPC tracing

The Tauri command layer logs every invocation at `debug` level. Set `LOG_LEVEL=debug` to see all IPC traffic.

### Resetting app state

```bash
# Clear localStorage (renderer state + preferences)
# Open DevTools → Application → Local Storage → clear

# Clear app data (DB, vector store, credentials)
# Windows: rm -rf %APPDATA%\ai-job-hunter-assistant-app
# macOS:   rm -rf ~/Library/Application\ Support/ai-job-hunter-assistant-app
```

**Re-trigger the onboarding wizard:**  
Settings → General → "Replay wizard"  
Or in DevTools console: `localStorage.setItem('ai-job-hunter-preferences', JSON.stringify({state:{onboardingCompleted:false}}))`

---

## Adding Dependencies

Always add to the right package, not the root:

```bash
# Renderer-only dependency
pnpm --filter @ajh/tauri add some-package

# Shared across packages
pnpm add -w some-package          # workspace root

# Dev tool (linters, build tools)
pnpm add -wD some-dev-tool
```

Never add Node.js-only packages to `packages/ui`, `packages/shared`, or `packages/prompts` — they run in browser/renderer context too.

---

## CI

GitHub Actions runs on every push and PR:

| Job                | Command                                 | Runs on           |
| ------------------ | --------------------------------------- | ----------------- |
| Lint               | `pnpm lint:strict` (zero warnings)      | push + PR         |
| Format             | `pnpm format:check`                     | push + PR         |
| Typecheck          | `pnpm typecheck`                        | push + PR         |
| Tests              | `pnpm test:coverage` + `pnpm -r test`   | push + PR         |
| Build verification | `pnpm build` + dist output check        | push + PR         |
| Storybook          | `pnpm --filter @ajh/ui build-storybook` | push to main only |

The pre-push hook runs `pnpm lint` + `pnpm -r test` locally.
TypeScript type-checking and full build verification run only in CI.

---

## Release

See `docs/RELEASE.md` for the full release pipeline.

**Short version:** push a `feat:` or `fix:` commit to `main` and the release happens automatically — semantic-release creates the GitHub Release and the build workflow attaches Windows, Linux, and macOS Tauri installers.
