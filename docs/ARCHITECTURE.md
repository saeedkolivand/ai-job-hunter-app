# Architecture — AI Job Hunter

> Local-first, AI-native desktop application. pnpm workspaces monorepo.

---

## Process Model

```
┌──────────────────────────────────────────────────────────────┐
│                     Electron Process Tree                     │
│                                                              │
│  ┌─────────────┐   IPC    ┌──────────────────────────────┐  │
│  │  Renderer   │◄────────►│         Main Process          │  │
│  │ (React UI)  │          │                              │  │
│  │ TanStack    │  events  │  ┌──────────┐ ┌───────────┐  │  │
│  │ React Query │◄─────────┤  │ JobQueue │ │ Scheduler │  │  │
│  └─────────────┘          │  └──────────┘ └───────────┘  │  │
│        ▲                  │  ┌──────────┐ ┌───────────┐  │  │
│        │ contextBridge    │  │AiRuntime │ │DataRuntime│  │  │
│  ┌─────┴──────┐           │  └──────────┘ └───────────┘  │  │
│  │  Preload   │           └──────────────────────────────┘  │
│  └────────────┘                                             │
└──────────────────────────────────────────────────────────────┘

External dependencies (all local — no cloud):
  Ollama     ←→  AiRuntime      (local LLM inference)
  LanceDB    ←→  DataRuntime    (vector store)
  NeDB       ←→  DataRuntime    (document store)
  Electron   ←→  ScraperRuntime (browser automation via ElectronBrowserController)
```

---

## Repository Structure

```
apps/desktop/src/
  main/        Main process — runtimes, job handlers, IPC router
  preload/     Context bridge — window.api surface
  renderer/
    features/  Feature-scoped components (owned by one route)
    routes/    TanStack Router file-based routes
    services/  React Query hooks (only place calling window.api.*)
    store/     Zustand stores
    lib/       Pure utilities (cn, motion, i18n, machines)
    providers/ React context providers
    hooks/     Shared React hooks

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
apps/desktop
  ├── packages/shared, ui, prompts   (renderer-safe)
  ├── packages/core, ai, data        (main process only)
  └── packages/workers               (main process only)

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
- `ipc/contracts.ts` — `IPC_CHANNELS` + typed `window.api` surface

### `@ajh/core`

Main-process infrastructure.

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

| Export             | Role                                              |
| ------------------ | ------------------------------------------------- |
| `DataRuntime`      | SQLite + LanceDB vector store (both lazy-opened)  |
| `ScraperRegistry`  | Board scrapers (HTTP + ElectronBrowserController) |
| `ApplierRegistry`  | Auto-appliers per board                           |
| `MatchingEngine`   | Keyword + semantic scoring                        |
| `AutopilotStore`   | NeDB CRUD for autopilot configs                   |
| `InMemoryJobStore` | Ephemeral live scrape results                     |
| `VectorStore`      | LanceDB wrapper                                   |

### `@ajh/prompts`

Pure TS prompt builders. Zero deps. `buildResumeSystemPrompt`, `buildCoverLetterSystemPrompt`,
`buildMetadataPrompt`, `MODES`, `GenerationMode`, `GenerationMeta`.

### `@ajh/ui`

React component library and design system. Tailwind v4 tokens, motion utilities, shared CSS classes.

### `apps/desktop` — Three Electron contexts

| Context  | Path            | Role                                  |
| -------- | --------------- | ------------------------------------- |
| Main     | `src/main/`     | Runtimes, job handlers, IPC router    |
| Preload  | `src/preload/`  | `contextBridge` exposing `window.api` |
| Renderer | `src/renderer/` | React + TanStack Router SPA           |

---

## Data Flows

### AI generation

```
Renderer → window.api.ai.generate(req) → jobs.enqueue('ai.generate')
  → generateStream() yields { delta, done }
  → ctx.stream() → EventBus → webContents.send('ai:stream')
  → window.api.ai.onStream(chunk) → UI updates token by token
```

### Scraping

```
window.api.scrape.board(req) → jobs.enqueue('scrape.board')
  → InProcessScraperRuntime.scrapeBoard()
  → scraper.search() → onItem() streams JobPosting items to renderer
  → results persisted to NeDB
```

### Autopilot

```
Scheduler tick OR window.api.autopilot.run(id)
  → jobs.enqueue('autopilot.run')
  → runAutopilot(): scrape → filter (keyword + score) → apply
  → ctx.stream({ kind: 'autopilot.*' }) → renderer live feed
```

---

## IPC Contract

All renderer↔main communication is typed end-to-end and Zod-validated:

```
window.api.X.method(payload)
  → ipcRenderer.invoke(channel, payload)
  → Zod.parse(payload)   ← rejects bad payloads
  → handler(validPayload)
```

- Channel names come from `IPC_CHANNELS` — never hardcode strings
- Renderer never touches `ipcRenderer` directly
- `services/` are the only place that calls `window.api.*`

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
3. Implement in `apps/desktop/src/main/ipc/router.ts`
4. Expose in `apps/desktop/src/preload/index.ts`
5. Create React Query hook in `apps/desktop/src/renderer/services/`

### New scraper

1. `packages/data/src/scraping/boards/myboard.ts` — implement `BaseScraper`
2. Register in `packages/data/src/scraping/registry.ts`
3. Add board ID to `packages/shared/src/schemas/index.ts`

### New route

1. `apps/desktop/src/renderer/routes/mypage.tsx` with `createFileRoute`
2. Add nav item in `Sidebar.tsx`
3. Add i18n key to all locale files in `apps/desktop/src/renderer/i18n/locales/`
