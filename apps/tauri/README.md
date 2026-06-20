# Desktop App — AI Job Hunter

<p align="center">
  <strong>Local-first Tauri shell: Rust core + React renderer.</strong>
</p>

<p align="center">
  <a href="https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white"><img alt="Tauri 2.x" src="https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white"></a>
  <a href="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white"><img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white"></a>
  <a href="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black"><img alt="React 19" src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black"></a>
</p>

The **AI Job Hunter** desktop application — a Tauri 2 shell wrapping a Rust core (scraping, login, documents, AI inference, vector search, export) with a React 19 + TanStack Router renderer. Runs fully offline with Ollama or plugs in OpenAI, Anthropic, Gemini, and any OpenAI-compatible provider.

---

## Quick Start

From the **repository root**:

```bash
pnpm dev
```

This starts the full Tauri dev server (Vite HMR on the renderer, Rust core with hot reload). The app window opens when ready.

**Prerequisites:** Node 20+, pnpm 11+, Rust stable, Ollama (optional for local AI).

For a production build:

```bash
pnpm --filter @ajh/tauri tauri build
```

Installers are generated per platform (macOS, Windows, Linux AppImage).

---

## Architecture at a Glance

```
React Renderer (TypeScript)
        ↓
  Service Hooks (React Query)
        ↓
   Tauri IPC (typed via Zod schemas)
        ↓
Rust Core (in-process commands)
        ↓
 Scraping · Documents · AI · Jobs · Autopilot · Export
        ↓
SQLite · Ollama/Cloud AI · Chromium · OS Keychain
```

**The golden rule:** React never talks to the OS directly. It routes everything through `AppClient` (tauri-client) service hooks, which marshal to Rust commands. The IPC contract is a single source of truth: Zod schemas in `packages/shared/src/ipc/contracts/` → both sides stay in sync.

---

## Directory Map

### `src-tauri/src/` — Rust Core

| Directory               | Responsibility                                                                                          |
| ----------------------- | ------------------------------------------------------------------------------------------------------- |
| `commands/`             | IPC endpoints — handlers for all renderer invocations                                                   |
| `platform/`             | Centralized config — env vars, data-dir, Ollama host, extension dev-origins                             |
| `net/`                  | Pooled HTTP client, SSRF guards, timeout handling                                                       |
| `error/`                | Unified `AppError` + `AppResult` — all fallible operations use this                                     |
| `observability/`        | Timed trace `Span`s for AI, scraping, autopilot (logged to console in dev)                              |
| `scraping/`             | Board scrapers (20 boards) — chromiumoxide for browser boards, HTTP for APIs; registry-driven discovery |
| `documents/`            | Document import, OCR dispatch, SQLite storage, full-text indexing                                       |
| `jobs/`                 | Job tracker state machine (queued → running → done/failed)                                              |
| `credentials/`          | OS keychain integration — AI keys + board session auth                                                  |
| `autopilot/`            | Job-discovery agent + scheduler — finds, ranks, notifies; crash reconciliation                          |
| `applications/`         | Application store aggregate (Saved / Applied / Rejected) — dedup by URL, track generations              |
| `postings/`             | Job posting index — vector + keyword search                                                             |
| `commands/ai_provider/` | Provider adapters (Ollama, OpenAI, Anthropic, Gemini, CLI agents) — streaming, reasoning, token limits  |
| `extension_bridge/`     | WebSocket bridge for browser extension — job import, pairing, token rotation                            |
| `export/`               | DOCX + PDF rendering — Typst engine (PDF) + docx-rs (DOCX), glyph subsetting                            |
| `updater/`              | Auto-update state machine — check, download, install                                                    |
| `tray/`                 | System-tray integration — dynamic "New jobs: N" badge, pause all autopilots                             |
| `deeplink/`             | Deep-link guard (`ajh://autopilot/<id>`) — strict allowlist validation                                  |
| `ai_generations/`       | Metadata tracking for generated résumés / cover letters / answers                                       |

### `src/renderer/` — React Renderer

| Directory     | Responsibility                                                                                          |
| ------------- | ------------------------------------------------------------------------------------------------------- |
| `features/`   | Components owned by a single route (e.g. job scraper UI)                                                |
| `routes/`     | TanStack Router pages (file-based routing)                                                              |
| `services/`   | React Query hooks for every IPC namespace                                                               |
| `components/` | Shared UI (re-exports from `@ajh/ui`) + layout (PageShell, Sidebar, Titlebar)                           |
| `lib/`        | Utilities: `generate` (streaming UI), `motion` (tokens), `i18n`, `machine` (state machines), `greeting` |
| `hooks/`      | Shared React hooks (`use-machine`, `use-menu-intents`)                                                  |
| `providers/`  | React context providers (theme, locale, app state)                                                      |
| `store/`      | Zustand stores (UI client state, not remote)                                                            |

**Do not import across feature directories** — features are scoped. Shared components go to `packages/ui` or `components/layout/`. New feature? Create `features/<name>/components/` and a route under `routes/`.

---

## Shared Packages (Monorepo)

| Package                 | Purpose                                                                     |
| ----------------------- | --------------------------------------------------------------------------- |
| `packages/shared`       | IPC contracts (Zod), shared types, extension protocol                       |
| `packages/ui`           | React component library (`@ajh/ui`) — buttons, inputs, modal, design tokens |
| `packages/prompts`      | AI prompt templates — provider-aware, locale-driven                         |
| `packages/translations` | i18n configuration + locale strings (en, de, …)                             |

---

## Key Concepts

### Ports & Adapters

The renderer is a thin **client**: it has no business logic. All stateful decisions live in the Rust core. The renderer's job is to:

1. Call a service hook (which wraps an IPC command)
2. Render the response (or streaming updates via Tauri events)
3. Send user actions back to the core

**Example:** "Generate a résumé" → renderer calls `useGenerateResume()` → hook invokes `ai_provider::generate_resume` → Rust streams `generation:chunk` events → renderer renders live → final result is saved to `applications` store.

### IPC & Commands

All React ↔ Rust communication is typed. The contract is **Zod schemas** in `packages/shared/src/ipc/contracts/`. When you add a new capability:

1. Define the request/response shapes in the Zod contract
2. Implement the handler in `src-tauri/src/commands/`
3. Wire the invoke call in `src/tauri-client/namespaces/`
4. Create a React Query hook in `src/renderer/services/`
5. Run `pnpm gen:ipc` to regenerate Rust struct definitions

### State Machines

Complex flows (autopilot, job scraping, login) use **state machines** (xstate). Machines live in `src/renderer/lib/machines/` and are consumed via `use-machine` hooks. Reducers Rust-side stay simple; branching logic is in the machine definition, not sprinkled through handlers.

### React Query

All remote data (IPC calls) flows through React Query service hooks, never raw `useState + useEffect`. Query keys are organized per namespace (e.g. `['jobs', 'scrape']`, `['applications', 'list']`); mutations invalidate and refetch as needed.

### Design System

Color tokens: `text-brand`, `bg-brand`, `border-brand` (no hardcoded hex).  
Motion tokens: `transition.fast`, `transition.modal`, `transition.spring` (import `transition` from `@ajh/ui`).  
UI primitives: all from `@ajh/ui` (Button, Input, Modal, etc.) — no raw `<button>` or `<select>`.  
Lint guards enforce this; violations block commits.

---

## Configuration

**Environment variables** (set before `pnpm dev` or in the release build):

| Var                         | Purpose                                             | Example                                                          |
| --------------------------- | --------------------------------------------------- | ---------------------------------------------------------------- |
| `AJH_DATA_DIR`              | Override app data directory (default: `~/.ajh`)     | `AJH_DATA_DIR=/custom/path pnpm dev`                             |
| `OLLAMA_HOST`               | Ollama endpoint (default: `http://127.0.0.1:11434`) | `OLLAMA_HOST=http://ollama:11434 pnpm dev`                       |
| `AJH_EXTENSION_DEV_ORIGINS` | Comma-separated extension origins for local dev     | `AJH_EXTENSION_DEV_ORIGINS="chrome-extension://abc123" pnpm dev` |

**Credentials & API keys** are stored in the **OS keychain** (encrypted, never in `.env` or plaintext). Set them in the UI: **Settings → AI**.

---

## Scripts

```bash
# From repo root
pnpm dev                                    # Tauri app with hot reload
pnpm --filter @ajh/tauri tauri build        # Production build
pnpm --filter @ajh/tauri tauri build --help # Build options (target, split debug)
pnpm typecheck                              # TS check across monorepo
pnpm test                                   # Vitest suite
pnpm lint:strict                            # ESLint with --max-warnings 0
pnpm gen:ipc                                # Regenerate Rust IPC structs from Zod
```

---

## Further Documentation

| Document                           | Content                                                                |
| ---------------------------------- | ---------------------------------------------------------------------- |
| **Root README**                    | Installation, features, quick start                                    |
| `docs/ARCHITECTURE.md`             | System design, component breakdown, data flow diagrams                 |
| `docs/PATTERNS.md`                 | IPC patterns, state machines, AI streaming, search, export rendering   |
| `docs/API.md`                      | IPC namespace reference — commands, payloads, responses                |
| `docs/DESIGN_SYSTEM.md`            | Tokens, component library, theming, motion                             |
| `docs/DEVELOPMENT.md`              | Local dev setup, debugging, troubleshooting                            |
| `docs/DEPLOYMENT.md`               | Building and releasing installers per platform                         |
| `docs/EXPORT_TEMPLATES.md`         | Résumé templates, PDF/DOCX rendering, theming customization            |
| `docs/knowledge/decision-records/` | Architecture decisions (ADRs) — why things are built the way they are  |
| `../../apps/extension/README.md`   | Browser extension — job import via loopback WebSocket, local dev setup |
