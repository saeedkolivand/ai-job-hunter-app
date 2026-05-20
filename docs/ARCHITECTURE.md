# Architecture — AI Job Hunter

> Local-first, AI-native desktop application.  
> Monorepo managed with **pnpm workspaces**.

---

## Process Model

```
┌─────────────────────────────────────────────────────────────────┐
│                       Electron Process Tree                      │
│                                                                 │
│  ┌──────────────┐    IPC     ┌──────────────────────────────┐  │
│  │   Renderer   │◄──────────►│         Main Process          │  │
│  │  (React UI)  │            │                              │  │
│  │              │   events   │  ┌──────────┐ ┌───────────┐  │  │
│  │  TanStack    │◄───────────┤  │ JobQueue │ │ Scheduler │  │  │
│  │  Router      │            │  └──────────┘ └───────────┘  │  │
│  │  Zustand     │            │  ┌──────────┐ ┌───────────┐  │  │
│  │  React Query │            │  │AiRuntime │ │DataRuntime│  │  │
│  └──────────────┘            │  └──────────┘ └───────────┘  │  │
│        ▲                     └──────────────────────────────┘  │
│        │ contextBridge                                          │
│  ┌─────┴──────┐                                                │
│  │  Preload   │                                                │
│  │ (bridge)   │                                                │
│  └────────────┘                                                │
└─────────────────────────────────────────────────────────────────┘

External dependencies (all local — no cloud):
  Ollama     ←→  AiRuntime           (local LLM inference)
  LanceDB    ←→  DataRuntime         (vector store)
  NeDB       ←→  DataRuntime         (document store)
  Playwright ←→  ScraperRegistry / ApplierRegistry
```

---

## Repository Structure

```
ai-job-hunter-assistant-app/
├── apps/
│   └── desktop/                    Electron desktop application
│       └── src/
│           ├── main/               Main process — runtimes, job handlers, IPC router
│           ├── preload/            Context bridge — window.api surface
│           └── renderer/           React frontend
│               ├── components/     Shared UI primitives + layout chrome
│               ├── features/       Feature-scoped components (owned by one route)
│               ├── routes/         TanStack Router file-based routes
│               ├── services/       React Query hooks wrapping window.api.*
│               ├── store/          Zustand stores (preferences, app state)
│               ├── lib/            Pure utilities (cn, motion, i18n, machines)
│               ├── providers/      React context providers
│               └── hooks/          Shared React hooks
│
└── packages/
    ├── shared/     IPC contracts, Zod schemas, cross-process types
    ├── ui/         React component library + design tokens + Storybook
    ├── prompts/    AI prompt templates (zero dependencies, pure TS)
    ├── core/       EventBus, JobQueue, Logger, RuntimeManager
    ├── ai/         Ollama client, AI runtime, streaming
    ├── data/       DB, scraping, matching, applying, vector search
    └── workers/    Worker thread pool
```

---

## Dependency Direction

Strict one-way flow. Lower layers never import from upper layers.

```
apps/desktop
    ├── packages/shared     (IPC contracts, types, schemas)
    ├── packages/ui         (components, design tokens)
    ├── packages/prompts    (AI prompt builders)
    ├── packages/core       (main process only)
    ├── packages/ai         (main process only)
    ├── packages/data       (main process only)
    └── packages/workers    (main process only)

packages/data
    ├── packages/shared
    ├── packages/core
    └── packages/workers

packages/ai
    ├── packages/shared
    └── packages/core

packages/core
    └── packages/shared

packages/ui          (no internal package imports)
packages/prompts     (no imports — zero deps)
```

**Renderer code never imports `packages/core`, `packages/ai`, or `packages/data`.**

---

## Package Roles

### `@ajh/shared`

Single source of truth for cross-process contracts. Safe to import in any process.

- `types/index.ts` — `Autopilot`, `JobPosting`, `JobRecord`, `JobKind`, `AutopilotAction`, etc.
- `schemas/index.ts` — Zod schemas for IPC payload validation
- `ipc/contracts.ts` — `IPC_CHANNELS` constants + typed `window.api` surface

### `@ajh/core`

Infrastructure primitives for the main process.

| Export             | Role                                                      |
| ------------------ | --------------------------------------------------------- |
| `EventBus`         | Typed pub/sub for inter-service communication             |
| `JobQueue`         | Async task queue — concurrency, retry, progress streaming |
| `TaskScheduler`    | Interval/timeout wrapper for recurring tasks              |
| `StateCoordinator` | Persisted key/value state with change events              |
| `RuntimeManager`   | Lifecycle manager for registered runtimes                 |
| `createLogger`     | pino-based structured logger                              |

### `@ajh/ai`

Ollama client and inference abstractions.

- `AiRuntime` — implements `Runtime`, owns the Ollama connection and model state
- `generateStream` — async generator yielding token deltas
- Health checking and model inventory

### `@ajh/data`

All data persistence, scraping, and matching.

| Export             | Role                                                       |
| ------------------ | ---------------------------------------------------------- |
| `DataRuntime`      | Owns SQLite DB + LanceDB vector store + Playwright browser |
| `ScraperRegistry`  | Board scrapers (HTTP + Playwright)                         |
| `ApplierRegistry`  | Playwright-based auto-appliers per board                   |
| `MatchingEngine`   | Keyword + semantic scoring                                 |
| `AutopilotStore`   | NeDB CRUD for autopilot configurations                     |
| `InMemoryJobStore` | Ephemeral live scrape results streamed to renderer         |
| `VectorStore`      | LanceDB wrapper for embeddings and semantic search         |

### `@ajh/prompts`

Pure TypeScript prompt builders. Zero dependencies, no `window` access.

- `buildResumeSystemPrompt(mode)` — tailored resume system prompts by generation mode
- `buildCoverLetterSystemPrompt(mode)` — cover letter prompts
- `buildMetadataPrompt(resume, jobAd)` — JSON metadata extraction
- `MODES` — generation mode definitions (`ats`, `executive`, `creative`, `career-change`)
- `GenerationMode`, `GenerationMeta` — shared types for generation pipeline

### `@ajh/ui`

React component library and design system.

- Tailwind CSS v4 tokens (`tokens.css`) — `--color-brand`, `--color-brand-soft`, etc.
- Motion utilities (`motion.ts`) — easings, duration presets, animation variants, transition tokens
- Shared CSS classes — `glass-surface`, `glass-modal`, `input-field`, `glow-purple`

### `apps/desktop` — Three Electron contexts

| Context  | Path            | Role                                                             |
| -------- | --------------- | ---------------------------------------------------------------- |
| Main     | `src/main/`     | Bootstraps runtimes, registers job handlers, handles IPC routing |
| Preload  | `src/preload/`  | `contextBridge` exposing typed `window.api`                      |
| Renderer | `src/renderer/` | React + TanStack Router SPA                                      |

---

## Data Flows

### AI generation (resume / cover letter)

```
Renderer
  window.api.ai.generate(req)               ← via service hook
      ↓ ipcRenderer.invoke('ai:generate')
Main IPC handler
  Zod validates req
  jobs.enqueue('ai.generate', req)  →  { jobId }
      ↓
JobQueue: 'ai.generate' handler
  generateStream(ollamaClient, req) yields { delta, done }
  ctx.stream({ delta, done })       →  EventBus.emit('job.event')
      ↓
IPC router (bus listener)
  webContents.send('ai:stream', { jobId, delta, done })
      ↓
Renderer
  window.api.ai.onStream(chunk)     →  UI updates token by token
```

### Autopilot run

```
Scheduler tick  OR  window.api.autopilot.run(id)
      ↓
jobs.enqueue('autopilot.run', { autopilotId })
      ↓
'autopilot.run' handler
  1. AutopilotStore.get(id)
  2. runAutopilot(autopilot, { scrapers, appliers, jobs, ... })
        ↓
     scraper.search()         →  streams JobPosting items
     passesKeywords() + matchScore()  →  filter
     applier.apply()          →  submit (if action !== 'save')
     store.recordRun()        →  persist stats
  3. ctx.stream({ kind: 'autopilot.*' })  →  renderer live feed
```

### Document import pipeline

```
window.api.documents.import(req)
      ↓
jobs.enqueue('document.import')
      ↓  extract text (PDF / DOCX / TXT / OCR)
jobs.enqueue('document.chunk')
      ↓  split into semantic chunks
jobs.enqueue('document.index')
      ↓  embed via Ollama + upsert into LanceDB
```

---

## IPC Contract

All renderer↔main communication is typed end-to-end and validated:

```
Renderer                    Preload                 Main
window.api.X.method(p)
    ↓
                    ipcRenderer.invoke(channel, p)
                                ↓
                                            Zod.parse(p)   ← rejects bad payloads
                                            handler(valid)
                                ↑
    ↑
```

**Rules:**

- Channel names come from `IPC_CHANNELS` (shared) — never hardcode strings
- Payloads are Zod-validated in `registerIpc()` before handlers run
- The renderer never touches `ipcRenderer` directly
- Service hooks in `renderer/services/` are the only place that calls `window.api.*`

---

## Renderer Architecture

### Ports & Adapters (enforced by ESLint)

Components and routes may not call `window.api.*` directly. All data access goes through service hooks:

```
Route / Feature / Component
    import { useDocuments } from '@/services'    ← only this
    window.api.documents.list()                  ← ESLint error

services/ (React Query wrappers)
    window.api.*                                 ← the single crossing point
```

### State layers

| Layer               | Tool                              | Examples                                                     |
| ------------------- | --------------------------------- | ------------------------------------------------------------ |
| Server state        | React Query via `services/`       | Job listings, documents, AI models                           |
| Global client state | Zustand                           | `usePreferencesStore` (persisted), `useAppStore` (ephemeral) |
| Local UI state      | `useState` / `useReducer`         | Form values, toggle state                                    |
| Complex flows       | State machines in `lib/machines/` | AI generation wizard, autopilot wizard                       |

### Routing

TanStack Router with file-based routes in `renderer/routes/`.  
`routeTree.gen.ts` is auto-generated — do not edit manually.

### i18n

- Single entry point: `import { useTranslation } from '@/lib/i18n'`
- Never import from `'react-i18next'` directly (ESLint enforces this)
- Language resolution on startup:
  1. Persisted user preference (from `usePreferencesStore` in localStorage) — highest priority
  2. System locale auto-detected by `i18next-browser-languagedetector` (reads `navigator.language`)
  3. Falls back to `'en'` if the detected locale is not in the supported list
- Users confirm/change language in the onboarding wizard (Step 2) and in Settings → General

### Onboarding wizard

A 3-step overlay that appears on first launch (`onboardingCompleted: false` in preferences store):

1. **Welcome** — collects user name
2. **Preferences** — confirms/changes language + remote work preference
3. **Spotlight tour** — highlights each sidebar nav item one at a time with a description card

Completion is persisted in `usePreferencesStore.onboardingCompleted`. The wizard can be replayed from **Settings → General → Replay wizard**.

---

## Autopilot Scheduling

```
TaskScheduler (main process)
  every 60 min  → enqueueAutopilots('hourly')
  every 12 hr   → enqueueAutopilots('twice_daily')
  every 24 hr   → enqueueAutopilots('daily')

enqueueAutopilots(schedule)
  → AutopilotStore.listBySchedule(schedule)
  → jobs.enqueue('autopilot.run', { autopilotId }) per active autopilot
```

---

## AI Tool Configuration

Rules are enforced at multiple levels so any tool respects the architecture:

| File                              | Tool           |
| --------------------------------- | -------------- |
| `CLAUDE.md`                       | Claude Code    |
| `.github/copilot-instructions.md` | GitHub Copilot |
| `.aider/system-prompt.md`         | Aider          |
| `.jba/guidelines.md`              | JetBrains AI   |

ESLint is the final enforcement layer — violations fail at commit time regardless of which tool generated the code.

---

## Extending the App

### New IPC channel

1. Add channel to `IPC_CHANNELS` in `packages/shared/src/ipc/contracts.ts`
2. Add Zod schema in `packages/shared/src/schemas/index.ts`
3. Add handler in `apps/desktop/src/main/ipc/router.ts`
4. Expose via `contextBridge` in `apps/desktop/src/preload/index.ts`
5. Create a React Query service hook in `apps/desktop/src/renderer/services/`

### New scraper

1. Create `packages/data/src/scraping/boards/myboard.ts` implementing `BaseScraper`
2. Register in `packages/data/src/scraping/registry.ts`
3. Add board ID to `BOARD_IDS` in `packages/shared/src/schemas/index.ts`

### New route / page

1. Create `apps/desktop/src/renderer/routes/mypage.tsx` with `createFileRoute`
2. Add nav item in `Sidebar.tsx` — include a `tourId` string for onboarding spotlight targeting
3. Add route constant in `constants/routes.ts`
4. Add `nav.mypage` i18n key to all locale files under `apps/desktop/src/renderer/i18n/locales/`

### New shared package

1. Create directory under `packages/`
2. Add to `pnpm-workspace.yaml`
3. Respect dependency direction — packages below `apps/desktop` must not import from it
