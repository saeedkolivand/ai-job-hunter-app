# Contributing to AI Job Hunter

Thank you for your interest in contributing. This document covers everything you need to get started.

---

## Table of Contents

- [Development Setup](#development-setup)
- [Monorepo Structure](#monorepo-structure)
- [Commit Conventions](#commit-conventions)
- [Branching Strategy](#branching-strategy)
- [Pull Request Process](#pull-request-process)
- [Testing](#testing)
- [Code Style](#code-style)
- [Release Process](#release-process)

---

## Development Setup

### Prerequisites

| Tool    | Version   | Notes                                                                            |
| ------- | --------- | -------------------------------------------------------------------------------- |
| Node.js | ≥ 20.11.0 | Use [nvm](https://github.com/nvm-sh/nvm) or [fnm](https://github.com/Schniz/fnm) |
| pnpm    | ≥ 11      | `npm install -g pnpm`                                                            |
| Ollama  | Latest    | [ollama.ai](https://ollama.ai) — required to run AI features                     |
| Git     | Any       | With commit message hook support                                                 |

### First-time setup

```bash
# 1. Clone the repo
git clone https://github.com/your-org/ai-job-hunter.git
cd ai-job-hunter

# 2. Install dependencies (installs git hooks automatically via husky)
pnpm install

# 3. Build all packages
pnpm build

# 4. Start the development app
pnpm dev
```

The Tauri app opens automatically. Hot-reload is enabled for the renderer.

### Available scripts

| Command              | Description                                     |
| -------------------- | ----------------------------------------------- |
| `pnpm dev`           | Build packages then start Tauri with hot-reload |
| `pnpm build`         | Build all packages                              |
| `pnpm typecheck`     | Run TypeScript type check across all packages   |
| `pnpm lint`          | Run ESLint                                      |
| `pnpm lint:fix`      | Run ESLint with auto-fix                        |
| `pnpm format`        | Format all files with Prettier                  |
| `pnpm format:check`  | Check formatting without writing                |
| `pnpm test`          | Run unit tests (Vitest)                         |
| `pnpm test:watch`    | Run tests in watch mode                         |
| `pnpm test:coverage` | Run tests with coverage report                  |
| `pnpm package`       | Build and package the Tauri app                 |
| `pnpm clean`         | Remove all build artifacts                      |

---

## Monorepo Structure

```
ai-job-hunter/
├── apps/
│   ├── tauri/                  # Tauri app (Rust core + React renderer)
│   │   ├── src-tauri/          # Rust core (commands, menu, tray, updater)
│   │   └── src/                # React UI + tauri-client.ts
│   └── scraper-runtime/        # Node.js HTTP sidecar
│
├── packages/
│   ├── shared/                 # Types, Zod schemas, IPC contracts
│   ├── core/                   # EventBus, JobQueue, TaskScheduler, logger
│   ├── ai/                     # Ollama client, inference wrappers
│   ├── data/                   # Scrapers, appliers, DB, vector store
│   └── workers/                # Background workers
│
├── .github/
│   ├── workflows/              # CI/CD pipelines
│   └── ISSUE_TEMPLATE/
│
├── docs/                       # Architecture and design documentation
├── eslint.config.mjs           # ESLint (flat config)
├── .prettierrc.json            # Prettier
├── commitlint.config.mjs       # Commit linting
├── vitest.config.ts            # Unit tests
└── .releaserc.json             # semantic-release config
```

### Package dependency graph

```
@ajh/tauri (renderer)
  └── @ajh/shared
  └── @ajh/ui
@ajh/data
  └── @ajh/shared
  └── @ajh/core
@ajh/ai
  └── @ajh/shared
  └── @ajh/core
@ajh/core
  └── @ajh/shared
```

Build order: `shared → core → ai → data → tauri`

---

## Commit Conventions

This project uses [Conventional Commits](https://www.conventionalcommits.org). Commit messages are enforced by commitlint (runs on every commit via husky).

### Format

```
<type>(<scope>): <subject>

[optional body]

[optional footer]
```

### Types

| Type       | When to use                    | Triggers release |
| ---------- | ------------------------------ | ---------------- |
| `feat`     | New feature                    | Minor            |
| `fix`      | Bug fix                        | Patch            |
| `perf`     | Performance improvement        | Patch            |
| `ui`       | UI/UX change                   | Patch            |
| `refactor` | Code refactor (no feature/fix) | No               |
| `docs`     | Documentation only             | No               |
| `test`     | Tests only                     | No               |
| `build`    | Build system                   | No               |
| `ci`       | CI/CD                          | No               |
| `chore`    | Maintenance                    | No               |
| `revert`   | Revert a commit                | Patch            |

Breaking changes: add `!` after the type or `BREAKING CHANGE:` in the footer → triggers a **major** release.

### Examples

```bash
feat(autopilot): add work type filter to wizard
fix(scraper): handle rate limit errors from LinkedIn
ui(analyze): improve streaming progress readability
docs: update contributing guide with monorepo structure
feat!: redesign IPC contract for campaign → autopilot rename
```

---

## Branching Strategy

- `main` — always releasable. All releases come from here.
- Feature branches: `feat/my-feature`
- Bug fixes: `fix/issue-description`
- Documentation: `docs/topic`

Merge via Pull Request only. Direct pushes to `main` are restricted.

---

## Pull Request Process

1. Fork or branch from `main`
2. Make your changes
3. Run `pnpm typecheck && pnpm lint && pnpm test` locally
4. Open a PR using the template
5. CI must pass before merge
6. At least one review approval required

---

## Testing

Tests live next to the code they test: `src/foo/bar.ts` → `src/foo/bar.test.ts`.

```bash
pnpm test              # Run all tests once
pnpm test:watch        # Watch mode
pnpm test:coverage     # With HTML coverage report → ./coverage/
```

### What to test

- **Always**: pure utility functions, Zod schema validation, matching logic
- **When practical**: state store reducers, data transforms
- **Skip**: Tauri IPC glue, Playwright browser automation (covered by e2e), UI components

---

## Code Style

ESLint and Prettier are enforced on every commit (lint-staged). CI also checks formatting.

Key rules:

- Use `type` imports: `import type { Foo } from './foo'`
- No `console.log` in source (use `createLogger` from `@ajh/core`)
- Prefer `const` over `let`
- No implicit `any`
- Named exports preferred over default exports (except React components and route files)

---

## Release Process

Releases are **fully automated** via [semantic-release](https://semantic-release.gitbook.io).

1. Merge a PR to `main`
2. semantic-release analyses commit messages since the last release
3. If a releasable commit is detected, it:
   - Bumps version in `package.json`
   - Generates/updates `CHANGELOG.md`
   - Creates a GitHub Release with release notes
   - Triggers the build workflow
4. The build workflow produces:
   - Windows: NSIS installer (`.exe`)
   - Linux: AppImage + `.deb`
5. Artifacts are attached to the GitHub Release automatically

No manual tagging or version bumping needed.
