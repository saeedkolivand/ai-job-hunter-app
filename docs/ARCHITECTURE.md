# Architecture — AI Job Hunter

> Local-first, AI-native desktop application. pnpm workspaces monorepo.

---

## Process Model

```
┌──────────────────────────────────────────────────────────────┐
│                        Tauri Process Tree                     │
│                                                              │
│  ┌─────────────┐  invoke  ┌──────────────────────────────┐  │
│  │  Renderer   │◄────────►│      Tauri Core (Rust)        │  │
│  │ (React UI)  │          │                              │  │
│  │ TanStack    │  events  │  ┌──────────┐ ┌───────────┐  │  │
│  │ React Query │◄─────────┤  │ JobQueue │ │ Scheduler │  │  │
│  └─────────────┘          │  └──────────┘ └───────────┘  │  │
│                           │  ┌──────────────────────────┐ │  │
│                           │  │  ScraperEngine (Rust)    │ │  │
│                           │  │  chromiumoxide (Rust)    │ │  │
│                           │  └──────────────────────────┘ │  │
│                           └──────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘

External dependencies (all local — no cloud):
  Ollama          ←→  AI inference (local LLM)
  SQLite          ←→  Job storage, documents, vectors
  Chromiumoxide   ←→  Browser automation (in-process)
```

---

## Repository Structure

```
apps/tauri/
  src-tauri/
    src/
      commands.rs         Tauri commands (IPC handlers)
      autopilot.rs        Autopilot data model + store
      documents.rs        Resume/cover letter parsing, vector embeddings
      scraping/           In-process Rust scrapers
        engine.rs         ScraperEngine orchestrator
        boards/           Board-specific HTTP scrapers
        board_login.rs    Browser-based board authentication
        scrape_url.rs     URL → JobPosting resolver
      applying/           Browser automation for job applications
        runtime.rs        ApplySession (chromiumoxide wrapper)
        boards/           Board-specific apply flows
        form_filler.rs    Generic form-filling helpers
      main.rs             Tauri app entry point
  src/
    tauri-client.ts       AppClient implementation over @tauri-apps/api
    renderer/
      features/           Feature-scoped components
      routes/             TanStack Router file-based routes
      services/           React Query hooks (only place calling AppClient)
      store/              Zustand stores
      lib/                Pure utilities (cn, motion, i18n, machines)

packages/
  shared/   IPC contracts, Zod schemas, cross-process types
  ui/       React component library + design tokens
  prompts/  AI prompt templates (zero deps)
```

---

## Dependency Direction

Strict one-way flow. Lower layers never import from upper layers.

```
apps/tauri/src-tauri/   Rust core (all scraping, applying, AI, data)
apps/tauri/src/         TypeScript renderer + AppClient

packages/shared         IPC contracts (used by both Rust and TS)
packages/ui             React components (renderer only)
packages/prompts        Prompt templates (renderer only)
```

**Renderer code never calls Rust directly — only via `AppClient` → Tauri `invoke`.**

---

## Package Roles

### `@ajh/shared`

Cross-process contracts. Safe anywhere.

- `types/index.ts` — `JobPosting`, `JobRecord`, `Autopilot`, `BootMetrics`, `AppMetrics`, etc.
- `schemas/index.ts` — Zod schemas for IPC payload validation
- `ipc/contracts.ts` — `IPC_CHANNELS` + typed `AppClient` surface

### Rust Core (`apps/tauri/src-tauri/src/`)

All scraping, applying, AI, and data logic runs in-process in Rust.

| Module                 | Role                                                   |
| ---------------------- | ------------------------------------------------------ |
| `scraping/engine`      | ScraperEngine — orchestrates board scrapers            |
| `scraping/boards/*`    | Board-specific HTTP scrapers (LinkedIn, Indeed, etc.)  |
| `scraping/board_login` | Browser-based login flows, cookie persistence          |
| `scraping/scrape_url`  | URL → JobPosting resolver (Greenhouse, Lever, etc.)    |
| `applying/runtime`     | ApplySession — chromiumoxide wrapper                   |
| `applying/boards/*`    | Board-specific apply flows (LinkedIn Easy Apply, etc.) |
| `applying/form_filler` | Generic form-filling helpers                           |
| `autopilot`            | Autopilot data model + JSON store                      |
| `documents`            | Resume/cover letter parsing, vector embeddings         |
| `commands`             | Tauri command handlers (IPC entry points)              |

### `@ajh/prompts`

Pure TS prompt builders. Zero deps. `buildResumeSystemPrompt`, `buildCoverLetterSystemPrompt`,
`buildMetadataPrompt`, `MODES`, `GenerationMode`, `GenerationMeta`.

### `@ajh/ui`

React component library and design system. Tailwind v4 tokens, motion utilities, shared CSS classes.

### `apps/tauri`

| Context  | Path                  | Role                                        |
| -------- | --------------------- | ------------------------------------------- |
| Rust     | `src-tauri/src/`      | Commands, menu, tray, updater, sidecar      |
| TS       | `src/tauri-client.ts` | `AppClient` implementation via Tauri invoke |
| Renderer | `src/renderer/`       | React + TanStack Router SPA                 |

---

## Data Flows

### AI generation

```
Renderer → AppClient.ai.generate(req) → Tauri invoke('ai_generate')
  → Rust: documents::embed() via Ollama HTTP API
  → Tauri emit('ai:stream') → UI updates token by token
```

### Scraping

```
AppClient.scrape.board(req) → Tauri invoke('scrape_board')
  → Rust: ScraperEngine.scrape_board()
  → Board scraper (HTTP or chromiumoxide)
  → onItem() emits JobPosting items to renderer
  → Results persisted to SQLite
```

### Autopilot

```
AppClient.autopilot.run(id) → Tauri invoke('autopilot_run')
  → Rust: scrape → rank by AI match score → apply to top N
  → Tauri emit('autopilot.step') → renderer live feed
```

### Browser State

```
~/.ajh/browser-state/
  linkedin/       Persistent Chromium profile for LinkedIn
  indeed/         Persistent Chromium profile for Indeed
  ...
  <board>/
    cookies.json  Exported cookies for HTTP client auth
    auth.json     Session age, staleness metadata
```

Each board gets its own Chromium profile. Cookies are exported after login and
reused by HTTP scrapers. `board_login::detect_system_chrome()` checks for system
Chrome/Edge to avoid chromiumoxide's 120 MB download.

---

## IPC Contract

All renderer↔Rust communication is typed end-to-end:

```
AppClient.X.method(payload)
  → invoke(channel, payload)
  → Zod.parse(payload)   ← rejects bad payloads
  → Rust handler(validPayload)
```

- Channel names come from `IPC_CHANNELS` — never hardcode strings
- Renderer never calls `invoke` directly
- `services/` are the only place that calls `AppClient.*`

---

## Renderer State Layers

| Layer         | Tool                              | Examples                             |
| ------------- | --------------------------------- | ------------------------------------ |
| Server state  | React Query via `services/`       | Jobs, documents, AI models           |
| Global client | Zustand                           | `usePreferencesStore`, `useAppStore` |
| Local UI      | `useState` / `useReducer`         | Form values, toggles                 |
| Complex flows | State machines in `lib/machines/` | AI wizard, autopilot wizard          |

---

## Extending the App

### New IPC channel

1. Add to `IPC_CHANNELS` + type in `packages/shared/src/ipc/contracts.ts`
2. Add Zod schema in `packages/shared/src/schemas/index.ts`
3. Implement Rust command in `apps/tauri/src-tauri/src/commands.rs`
4. Wire in `apps/tauri/src/tauri-client.ts`
5. Create React Query hook in `apps/tauri/src/renderer/services/`

### New scraper

1. `apps/tauri/src-tauri/src/scraping/boards/myboard.rs` — implement `Scraper` trait
2. Register in `apps/tauri/src-tauri/src/scraping/engine.rs` match arms
3. Add board ID to catalog in `ScraperEngine::catalog()`

### New applier

1. `apps/tauri/src-tauri/src/applying/boards/myboard.rs` — implement `Applier` trait
2. Register in `apps/tauri/src-tauri/src/applying/registry.rs`
3. Add selectors to `apps/tauri/src-tauri/src/applying/selectors.rs`

### New route

1. `apps/tauri/src/renderer/routes/mypage.tsx` with `createFileRoute`
2. Add nav item in `Sidebar.tsx`
3. Add i18n key to all locale files in `apps/tauri/src/renderer/i18n/locales/`
