# Development Setup — AI Job Hunter

Last updated: 2026-06-01

This guide gets you from zero to a running dev environment.

---

## Prerequisites

| Tool          | Version       | Install                            |
| ------------- | ------------- | ---------------------------------- |
| Node.js       | 20.11.0+      | [nodejs.org](https://nodejs.org)   |
| pnpm          | 11+           | `npm install -g pnpm`              |
| Rust (stable) | latest stable | [rustup.rs](https://rustup.rs)     |
| Ollama        | latest        | [ollama.com](https://ollama.com)   |
| Git           | any           | [git-scm.com](https://git-scm.com) |

### Windows-specific

Install the Visual C++ Build Tools (required for Rust compilation):

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

Or install via the [VS Build Tools installer](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

Also install WebView2 (usually pre-installed on Windows 11):

```powershell
winget install Microsoft.EdgeWebView2Runtime
```

---

## Clone and Install

```bash
git clone https://github.com/saeedkolivand/ai-job-hunter-app.git
cd ai-job-hunter-app

# Install all workspace dependencies
pnpm install
```

This installs dependencies for all packages and apps via [pnpm][pnpm] workspaces.

---

## Start Ollama

[Ollama][ollama] must be running before you start the app (for AI features):

```bash
# Pull a model (first time only — choose one)
ollama pull mistral          # fast, good for most tasks
ollama pull llama3.2         # better quality
ollama pull neural-chat      # optimized for conversation

# Verify it's running
ollama list
curl http://127.0.0.1:11434/api/tags
```

On Windows, Ollama runs as a background service after installation. On macOS/Linux, run `ollama serve` in a separate terminal.

---

## Run the App

```bash
pnpm dev
```

This command (via Turbo) will:

1. Build all packages in dependency order
2. Start Vite dev server for the renderer (with HMR)
3. Launch the Tauri desktop window

The first build takes ~2 minutes (Rust compilation). Subsequent starts are fast (~5–10s).

### Frontend-only mode

If you're working only on the React UI:

```bash
pnpm dev:frontend
```

This starts the Vite dev server without Tauri. The app runs in your browser at `http://localhost:1420`. IPC calls use the mock client (`createMockClient()`), so all features work with simulated data.

---

## Project Scripts

```bash
# Development
pnpm dev              # Full Tauri app (default)
pnpm dev:frontend     # Browser-only (mock IPC)

# Building
pnpm build            # Build all packages (Turbo)
pnpm build:packages   # Build packages only (skip Tauri binary)

# Code quality
pnpm typecheck        # tsc --noEmit across all packages
pnpm lint             # ESLint across monorepo
pnpm lint:fix         # ESLint auto-fix (runs on commit via husky)
pnpm lint:strict      # --max-warnings 0 (CI mode)
pnpm format           # Prettier format all files

# Testing
pnpm test             # Run Vitest in all packages
pnpm test:watch       # Watch mode

# Cleanup
pnpm clean            # Remove dist/, .turbo/, node_modules/
```

---

## Workspace Structure

```
apps/
  tauri/
    src-tauri/          ← Rust code (Cargo.toml, commands, DB, scrapers)
    src/
      tauri-client.ts   ← Tauri invoke/listen wiring
      renderer/         ← React app (routes, features, services, stores)

packages/
  shared/               ← IPC contracts + Zod schemas + shared types
  ui/                   ← @ajh/ui component library
  prompts/              ← AI prompt templates
  test-ids/             ← @ajh/test-ids — centralized test-id constants (drift-proof data-testid)
```

---

## Working on Packages

### Modifying the UI library (`packages/ui`)

```bash
cd packages/ui
pnpm build        # rebuild the package
# or start watcher:
pnpm dev          # watch mode (if configured)
```

The renderer imports `@ajh/ui` via the workspace symlink. After rebuilding, Vite HMR picks up changes automatically.

### Modifying IPC contracts (`packages/shared`)

1. Edit `packages/shared/src/ipc/contracts/<namespace>.ts`
2. Run `pnpm --filter @ajh/shared gen:ipc` to regenerate `ipc_contracts/*.rs` (CI runs `gen:ipc:check` to enforce this)
3. Run `pnpm typecheck` to verify no breakage
4. Update the Rust command handler in `apps/desktop/src-tauri/src/commands/`
5. Update `apps/desktop/src/tauri-client.ts`
6. Update the service hook in `apps/desktop/src/renderer/services/`

### Modifying Rust code (`apps/desktop/src-tauri`)

Tauri compiles Rust automatically during `pnpm dev`. For manual compilation:

```bash
cd apps/desktop/src-tauri
cargo build         # debug build
cargo check         # fast type check only
cargo clippy        # lint
```

---

## Adding a New Route

[TanStack Router][tanstack-router] uses file-based routing in `apps/desktop/src/renderer/routes/`:

1. Create the route file: `routes/my-page.tsx`
2. Export a default component

```typescript
// routes/my-page.tsx
import { createFileRoute } from "@tanstack/react-router";
import { PageShell } from "@/components/layout/PageShell";

export const Route = createFileRoute("/my-page")({
  component: MyPage,
});

function MyPage() {
  return (
    <PageShell title="My Page">
      {/* content */}
    </PageShell>
  );
}
```

3. Add navigation link in `components/layout/Sidebar.tsx`
4. Add i18n key in `public/locales/en/translation.json`

---

## Adding a New Feature

1. Create `renderer/features/my-feature/` directory
2. Add `components/`, `hooks/` subdirectories as needed
3. Export the top-level component from `index.tsx`
4. Import from the route file only via the index export

Never import internal feature components from outside the feature directory.

---

## Environment and Config

No `.env` files are needed. The app uses:

- **OS keychain** for secrets (API keys, board credentials)
- **SQLite** (local file) for all application data
- **Tauri conf** (`apps/desktop/src-tauri/tauri.conf.json`) for app metadata

If you need to override the Ollama host (e.g. remote Ollama):

```
Settings → AI → Ollama Host → http://your-host:11434
```

---

## Database

Storage is a set of **per-domain [SQLite][sqlite] files** (rusqlite, bundled) in the OS
app data directory — there is no single `app.db` and no separate vector store. The main
files include `documents.db` (imported documents **plus** embedding vectors — the
`vectors`, `posting_vectors`, `match_scores` tables), `jobs.db`, `applications.db`,
`ai_generations.db`, `job_preferences.db`, `contact_profile.db`, `referrals.db`, and
`pipeline_cache.db`.

App-data directory per OS (rooted at your home directory, `<HOME>`):

- **Windows**: `<HOME>\AppData\Roaming\ai-job-hunter\`
- **macOS**: `<HOME>/Library/Application Support/ai-job-hunter/`
- **Linux**: `<HOME>/.local/share/ai-job-hunter/`

Embedding vectors live in the `vectors` table of `documents.db` and cosine similarity
runs in-process in Rust — **no LanceDB, no `vectors/` directory**. To reset dev state,
delete the `*.db` files in that directory and restart the app.

---

## Commit Hooks

[Husky][husky] runs on `git commit`:

1. [lint-staged][lint-staged] — [ESLint][eslint] + [Prettier][prettier] on staged files
2. [commitlint][commitlint] — validates commit message format

If a commit is rejected, check the terminal output for the specific lint error. Never use `--no-verify`.

---

## Deterministic Review Tooling

The Stop review-gate runs **Tier-0 structural linters** to catch regressions before human review:

```bash
pnpm scan:rules              # ast-grep structural rules from .claude/review-rules/*.yml
```

These rules are zero-false-positive and **mandatory** — the repo must always scan clean. A rule that fires on `main` is either a real regression (fix the code) or a bad rule (disable/refine in the rule's YAML under `.claude/review-rules/*.yml`). Rules check for unsafe patterns (environment variables outside platform modules, Result-to-string conversions, GC infinity bugs in queries, etc.) — see `.claude/review-rules/*.yml` for the full catalog.

For **secret scanning** during local review (before push), install [gitleaks](https://github.com/gitleaks/gitleaks):

```bash
winget install gitleaks              # Windows
brew install gitleaks                # macOS
apt install gitleaks                 # Linux (Debian/Ubuntu)
scoop install gitleaks               # Windows (alternative)
```

Then run `gitleaks detect --source . -v` in the repo root to catch hardcoded credentials, API keys, etc. The reviewer falls back to a grep-based scan if gitleaks is absent.

---

## Optional: Knowledge-Graph MCP (graphify)

[graphify](https://pypi.org/project/graphifyy/) builds a queryable knowledge graph of the codebase under `graphify-out/`. It can also run as a local [MCP](https://modelcontextprotocol.io) server, exposing `query` / `explain` / `path` retrieval to AI dev tools (Claude Code, etc.) — usually far cheaper than grepping raw files.

It is **opt-in and per-developer**: the active `.mcp.json` is gitignored (it's your personal MCP config), so nothing is forced on contributors who don't use it. To enable it:

```bash
# 1. Add the `mcp` dependency to graphify's environment (absent by default)
uv tool install --with mcp graphifyy

# 2. Build the graph (AST-only — no API cost)
graphify update .

# 3. Register the server in your own MCP config, then restart your AI tool
cp .mcp.json.example .mcp.json
```

`.mcp.json` registers the graphify MCP server. See `.mcp.json.example` (`mcpServers.graphify` key) for the exact command and arguments. Skip this entirely if you don't use graphify — nothing else depends on it.

---

## Common Issues

### "cargo: command not found"

Rust is not installed or not in PATH. Run:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env  # or restart terminal
```

### "VCRUNTIME140.dll not found" (Windows)

Install Visual C++ Redistributables:

```powershell
winget install Microsoft.VCRedist.2015+.x64
```

### "Ollama connection refused"

Ollama is not running. Start it:

```bash
ollama serve
# or on Windows: check system tray for Ollama icon
```

### "pnpm: command not found"

```bash
npm install -g pnpm
```

### Build fails with TypeScript errors

```bash
pnpm clean && pnpm install && pnpm build
```

### Tauri build fails (Windows)

Ensure WebView2 is installed and run from a terminal with admin privileges if needed. Check `apps/desktop/src-tauri/` for any Rust-specific issues with `cargo check`.

---

## IDE Setup

### VS Code (Recommended)

Install extensions:

- **rust-analyzer** — Rust language support
- **Tailwind CSS IntelliSense** — class autocomplete
- **ESLint** — inline lint feedback
- **Prettier** — format on save

Workspace settings (`.vscode/settings.json`):

```json
{
  "editor.formatOnSave": true,
  "editor.defaultFormatter": "esbenp.prettier-vscode",
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "typescript.tsdk": "node_modules/typescript/lib"
}
```

### JetBrains (WebStorm / RustRover)

- Enable [ESLint][eslint] in `Preferences → Languages → JavaScript → Code Quality Tools → ESLint`
- Enable [Prettier][prettier] as formatter on save
- Use RustRover or IntelliJ + Rust plugin for `src-tauri/`

[pnpm]: https://pnpm.io
[ollama]: https://ollama.com
[tanstack-router]: https://tanstack.com/router
[sqlite]: https://www.sqlite.org
[husky]: https://typicode.github.io/husky
[lint-staged]: https://github.com/lint-staged/lint-staged
[eslint]: https://eslint.org
[prettier]: https://prettier.io
[commitlint]: https://commitlint.js.org
