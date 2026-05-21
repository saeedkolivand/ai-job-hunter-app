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
│                           │  │   Sidecar (scraper-rt)   │ │  │
│                           │  └──────────────────────────┘ │  │
│                           └──────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘

External dependencies (all local — no cloud):
  Ollama     ←→  AiRuntime      (local LLM inference)
  LanceDB    ←→  DataRuntime    (vector store)
  NeDB       ←→  DataRuntime    (document store)
  Sidecar    ←→  ScraperRuntime (browser automation)
```

---

## Repository Structure

```
apps/tauri/
  src-tauri/   Rust core — commands, menu, tray, updater, sidecar launcher
  src/
    tauri-client.ts   AppClient implementation over @tauri-apps/api invoke/listen
    renderer/
      features/  Feature-scoped components (owned by one route)
      routes/    TanStack Router file-based routes
      services/  React Query hooks (only place calling AppClient methods)
      store/     Zustand stores
      lib/       Pure utilities (cn, motion, i18n, machines)
      providers/ React context providers
      hooks/     Shared React hooks

apps/scraper-runtime/
  src/         Node.js HTTP sidecar — scraping, login, documents, AI

packages/
  shared/   IPC contracts, Zod schemas, cross-process types
  ui/       React component library + design tokens
  prompts/  AI prompt templates (zero deps)
  core/     EventBus, JobQueue, Logger, RuntimeManager
  ai/       Ollama client, AI runtime, streaming
  data/     DB, scraping, matching, applying, vector search
  workers/  Worker thread pool
```

---

## Dependency Direction

Strict one-way flow. Lower layers never import from upper layers.

```
apps/tauri
  ├── packages/shared, ui, prompts   (renderer-safe)
  ├── packages/core, ai, data        (sidecar / Rust side only)
  └── packages/workers               (sidecar / Rust side only)

packages/data   → shared, core, workers
packages/ai     → shared, core
packages/core   → shared
packages/ui     (no internal imports)
packages/prompts (zero deps)
```

**Renderer code never imports `packages/core`, `packages/ai`, `packages/data`, or `packages/workers`.**

---

## Package Roles

### `@ajh/shared`

Cross-process contracts. Safe anywhere.

- `types/index.ts` — `JobPosting`, `JobRecord`, `Autopilot`, `BootMetrics`, `AppMetrics`, etc.
- `schemas/index.ts` — Zod schemas for IPC payload validation
- `ipc/contracts.ts` — `IPC_CHANNELS` + typed `AppClient` surface

### `@ajh/core`

Infrastructure used by the sidecar.

| Export             | Role                                                       |
| ------------------ | ---------------------------------------------------------- |
| `EventBus`         | Typed pub/sub                                              |
| `JobQueue`         | Async task queue — concurrency, retry, progress, streaming |
| `TaskScheduler`    | Interval/timeout for recurring tasks                       |
| `StateCoordinator` | Persisted key/value with change events                     |
| `RuntimeManager`   | Lifecycle manager for registered runtimes                  |
| `createLogger`     | pino structured logger                                     |

### `@ajh/ai`

Ollama client and inference. `AiRuntime` (lazy start, idle model unload), `generateStream`.

### `@ajh/data`

Data persistence, scraping, matching.

| Export             | Role                                             |
| ------------------ | ------------------------------------------------ |
| `DataRuntime`      | SQLite + LanceDB vector store (both lazy-opened) |
| `ScraperRegistry`  | Board scrapers (HTTP + browser controller)       |
| `ApplierRegistry`  | Auto-appliers per board                          |
| `MatchingEngine`   | Keyword + semantic scoring                       |
| `AutopilotStore`   | NeDB CRUD for autopilot configs                  |
| `InMemoryJobStore` | Ephemeral live scrape results                    |
| `VectorStore`      | LanceDB wrapper                                  |

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
  → sidecar: generateStream() yields { delta, done }
  → Tauri listen('ai:stream') → UI updates token by token
```

### Scraping

```
AppClient.scrape.board(req) → Tauri invoke('scrape_board')
  → sidecar: ScraperRuntime.scrapeBoard()
  → scraper.search() → onItem() streams JobPosting items to renderer
  → results persisted to SQLite
```

### Autopilot

```
Scheduler tick OR AppClient.autopilot.run(id)
  → sidecar: runAutopilot(): scrape → filter (keyword + score) → apply
  → Tauri listen('autopilot:*') → renderer live feed
```

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

1. `packages/data/src/scraping/boards/myboard.ts` — implement `BaseScraper`
2. Register in `packages/data/src/scraping/registry.ts`
3. Add board ID to `packages/shared/src/schemas/index.ts`

### New route

1. `apps/tauri/src/renderer/routes/mypage.tsx` with `createFileRoute`
2. Add nav item in `Sidebar.tsx`
3. Add i18n key to all locale files in `apps/tauri/src/renderer/i18n/locales/`
