# Architecture — AI Job Hunter

## High-Level Overview

AI Job Hunter is a **local-first desktop application** built on Tauri 2. There is no cloud backend, no telemetry endpoint, and no remote database. Every computation — AI inference, web scraping, vector search, document parsing — runs on the user's machine.

The architecture follows a **ports-and-adapters** model: the React renderer communicates exclusively through typed IPC contracts, and the Rust core handles everything else — command routing plus the heavy work (scraping, document processing, embeddings) natively, without a separate process.

---

## System Architecture

```mermaid
graph TB
    subgraph Renderer["Renderer Process (React)"]
        Router["TanStack Router\n(file-based routes)"]
        Services["Service Hooks\n(React Query + IPC)"]
        Stores["Zustand Stores\n(UI client state)"]
        UILib["@ajh/ui\n(component library)"]
    end

    subgraph Tauri["Tauri Core (Rust)"]
        Commands["IPC Commands\n(21 namespaces)"]
        Keychain["OS Keychain\n(credentials)"]
        Updater["Auto-Updater"]
        Window["Window / Tray / Menu"]
    end

    subgraph Background["Background Runtimes"]
        EventBus["EventBus\n(typed pub/sub)"]
        JobQueue["JobQueue\n(task dispatch)"]
        Scheduler["Scheduler\n(cron-like)"]
        RuntimeManager["RuntimeManager\n(lifecycle)"]
    end

    subgraph AIRuntime["AI Runtime"]
        OllamaClient["Ollama Client\n(chat + embed)"]
        CloudProviders["Cloud Providers\n(OpenAI / Anthropic\nGemini / LM Studio)"]
        EmbedWorker["Embedding Worker\n(Web Worker)"]
    end

    subgraph DataRuntime["Data Runtime"]
        SQLite["SQLite\n(Drizzle ORM)"]
        LanceDB["LanceDB\n(vector search)"]
        ChunkWorker["Chunking Worker\n(Web Worker)"]
        OCRWorker["OCR Worker\n(Tesseract.js)"]
    end

    subgraph Scrapers["Scraping Layer"]
        Playwright["Playwright\n(browser automation)"]
        Cheerio["Cheerio\n(HTML parsing)"]
        Boards["18+ Board\nImplementations"]
    end

    Router --> Services
    Services --> Commands
    Commands --> EventBus
    EventBus --> JobQueue
    JobQueue --> RuntimeManager
    RuntimeManager --> AIRuntime
    RuntimeManager --> DataRuntime
    RuntimeManager --> Scrapers
    Playwright --> Boards
    Cheerio --> Boards
    OllamaClient --> EmbedWorker
    SQLite --> ChunkWorker
    SQLite --> OCRWorker
    Commands --> Keychain
    Commands --> Updater
```

---

## Component Breakdown

### `apps/tauri` — Desktop Shell

The Tauri app is split into two processes:

**Rust core (`src-tauri/`)** — thin orchestration layer:

| Module            | Responsibility                                                       |
| ----------------- | -------------------------------------------------------------------- |
| `commands/`       | IPC endpoint handlers; routes invocations to the appropriate runtime |
| `scraping/`       | 18 board-specific Playwright scrapers                                |
| `documents/`      | Document import, OCR dispatch, SQLite storage                        |
| `jobs/`           | Job tracker state machine (queued → running → done/failed)           |
| `credentials/`    | OS keychain CRUD via Tauri keychain plugin                           |
| `conversations/`  | Chat history persistence                                             |
| `autopilot/`      | Workflow engine + step scheduler                                     |
| `apply_helpers/`  | Form-filling logic for auto-apply                                    |
| `ai_generations/` | Metadata tracking for generated documents                            |
| `export/`         | DOCX/PDF rendering using docx + jsPDF                                |
| `updater/`        | Auto-update state (check, download, install)                         |
| `browser/`        | System browser detection and launch                                  |

**React renderer (`src/renderer/`)** — feature-scoped UI:

| Directory            | Responsibility                                                   |
| -------------------- | ---------------------------------------------------------------- |
| `routes/`            | TanStack Router file-based pages (9 routes)                      |
| `features/`          | Feature-scoped component trees (never cross-import)              |
| `services/`          | React Query hooks wrapping every IPC namespace                   |
| `lib/`               | Pure utilities: motion tokens, i18n, state machine, `cn()`       |
| `store/`             | Zustand stores for persistent UI state                           |
| `providers/`         | React context providers (AppClient, Capability, PerformanceMode) |
| `hooks/`             | Shared React hooks (`useMachine`, `useMouseParallax`)            |
| `components/layout/` | Sidebar, Titlebar, StatusBar, PageShell                          |

---

### `packages/shared` — Contract Layer

The single source of truth for renderer ↔ Rust communication:

```
packages/shared/src/
├── ipc/
│   ├── contracts/          # 21 typed namespace definitions
│   │   ├── ai.ts
│   │   ├── aiGenerations.ts
│   │   ├── apply.ts
│   │   ├── autopilot.ts
│   │   ├── boards.ts
│   │   ├── conversations.ts
│   │   ├── credentials.ts
│   │   ├── dialog.ts
│   │   ├── documents.ts
│   │   ├── geocode.ts
│   │   ├── jobPreferences.ts
│   │   ├── jobs.ts
│   │   ├── linkedin.ts
│   │   ├── match.ts
│   │   ├── privacy.ts
│   │   ├── resume.ts
│   │   ├── scrape.ts
│   │   ├── search.ts
│   │   ├── shortcuts.ts
│   │   ├── support.ts
│   │   ├── system.ts
│   │   └── updater.ts
│   └── contracts.ts        # Re-exports all namespaces
├── schemas/                # Zod validation schemas
├── types/                  # JobRecord, DocumentRecord, MatchScore, etc.
├── language-detection.ts   # franc.js language detection
├── ai-models.ts            # Model registry per provider
└── utils.ts
```

---

### `packages/core` — Runtime Infrastructure

```mermaid
graph LR
    EventBus["EventBus\n(typed pub/sub)"]
    JobQueue["JobQueue\n(dispatch + retry)"]
    Scheduler["Scheduler\n(async cron)"]
    RuntimeManager["RuntimeManager\n(lifecycle)"]
    StateCoordinator["StateCoordinator\n(cross-runtime sync)"]
    Logger["Logger\n(Pino factory)"]

    RuntimeManager --> EventBus
    RuntimeManager --> JobQueue
    RuntimeManager --> Scheduler
    RuntimeManager --> StateCoordinator
```

- **EventBus** — typed publish/subscribe within the Node process; decouples producers from consumers
- **JobQueue** — enqueues tasks by kind, dispatches to handler, supports retry with backoff
- **Scheduler** — cron-like recurring tasks (Autopilot runs, model health checks)
- **RuntimeManager** — starts/stops AI and Data runtimes, coordinates graceful shutdown
- **StateCoordinator** — synchronizes state changes across AI, Data, and Worker runtimes

---

### `packages/ai` — AI Runtime

```mermaid
graph TB
    AiRuntime["AiRuntime"]
    OllamaClient["OllamaClient\n(HTTP via undici)"]
    ModelRegistry["ModelRegistry\n(per-provider model list)"]
    ChatGen["ChatGenerator\n(streaming SSE)"]
    EmbedGen["EmbeddingGenerator\n(batch vectors)"]
    CloudAdapters["Cloud Adapters\nOpenAI / Anthropic / Gemini"]

    AiRuntime --> OllamaClient
    AiRuntime --> ModelRegistry
    AiRuntime --> ChatGen
    AiRuntime --> EmbedGen
    AiRuntime --> CloudAdapters
    OllamaClient --> ChatGen
    OllamaClient --> EmbedGen
```

Supports providers: **Ollama** (default, local), **OpenAI**, **Anthropic** (with extended thinking blocks), **Gemini**, **OpenAI-compatible** (LM Studio, remote Ollama).

---

### `packages/data` — Data Runtime

```mermaid
graph TB
    DataRuntime["DataRuntime"]
    SQLiteDB["SQLite\n(better-sqlite3 + Drizzle)"]
    VectorDB["LanceDB\n(ANN + hybrid search)"]
    Matcher["Matcher\n(resume-job scoring)"]
    FileProcessor["FileProcessor\n(PDF/DOCX/TXT/OCR)"]
    Scrapers["Board Scrapers"]

    DataRuntime --> SQLiteDB
    DataRuntime --> VectorDB
    DataRuntime --> Matcher
    DataRuntime --> FileProcessor
    DataRuntime --> Scrapers
    FileProcessor --> OCRWorker["OCR Worker\n(Tesseract.js)"]
    FileProcessor --> ChunkWorker["Chunk Worker\n(text splitting)"]
    VectorDB --> EmbedWorker["Embed Worker\n(batch vectorize)"]
```

---

### `packages/ui` — Component Library (`@ajh/ui`)

A standalone React component library with no routing, IPC, or state management dependencies. Consumed only from the renderer.

---

## Data Flow

### AI Generation Request

```mermaid
sequenceDiagram
    participant UI as React UI
    participant SH as Service Hook
    participant IPC as Tauri IPC
    participant Cmd as Rust Command
    participant AI as AI Runtime
    participant LLM as LLM (Ollama/Cloud)

    UI->>SH: useAiGenerate(config)
    SH->>IPC: appClient.ai.generate(req)
    IPC->>Cmd: invoke("ai_generate", payload)
    Cmd->>AI: route to provider
    AI->>LLM: POST /api/chat (stream)
    LLM-->>AI: SSE chunks
    AI-->>Cmd: delta tokens
    Cmd-->>IPC: emit("ai:stream", {delta, done})
    IPC-->>SH: onStream callback
    SH-->>UI: append delta to output
    UI->>UI: re-render StreamingText
```

### Document Import Pipeline

```mermaid
sequenceDiagram
    participant UI as React UI
    participant IPC as Tauri IPC
    participant Doc as Documents Module
    participant OCR as OCR Worker
    participant Chunk as Chunk Worker
    participant Embed as Embed Worker
    participant DB as SQLite + LanceDB

    UI->>IPC: documents.import(filePath)
    IPC->>Doc: detect format (PDF/DOCX/TXT/image)
    Doc->>OCR: if image → Tesseract.js
    OCR-->>Doc: extracted text
    Doc->>DB: insert document record (SQLite)
    Doc->>Chunk: split text into chunks
    Chunk-->>Doc: chunk array
    Doc->>Embed: vectorize chunks (Ollama)
    Embed-->>Doc: float32[] vectors
    Doc->>DB: upsert vectors (LanceDB)
    Doc-->>IPC: DocumentRecord
    IPC-->>UI: success + document metadata
```

### Hybrid Search

```mermaid
sequenceDiagram
    participant UI as Search Page
    participant IPC as Tauri IPC
    participant Search as Search Handler
    participant Lance as LanceDB
    participant SQL as SQLite

    UI->>IPC: search.hybrid({query, collection, topK, semanticWeight})
    IPC->>Search: route request
    Search->>Lance: ANN vector search (semantic)
    Lance-->>Search: scored document IDs
    Search->>SQL: keyword filter on matched IDs
    SQL-->>Search: refined result set
    Search-->>IPC: HybridSearchResult[]
    IPC-->>UI: ranked results
```

### Autopilot Execution

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Queued: enable workflow
    Queued --> Running: scheduler fires
    Running --> Paused: user pauses
    Paused --> Running: user resumes
    Running --> Completed: all steps done
    Running --> Failed: unrecoverable error
    Completed --> Queued: next schedule interval
    Failed --> Queued: manual retry
```

---

## IPC Contract Model

Every renderer ↔ Rust interaction is defined in `packages/shared/src/ipc/contracts/`. The pattern:

```typescript
// packages/shared/src/ipc/contracts/ai.ts
export interface AiContract {
  generate(req: GenerateRequest): Promise<GenerateResponse>;
  listModels(): Promise<ModelInfo[]>;
  pullModel(name: string): Promise<void>;
  embed(text: string): Promise<number[]>;
  setProviderKey(provider: string, key: string): Promise<void>;
  onStream(handler: (chunk: StreamChunk) => void): Unsubscribe;
}
```

The renderer accesses contracts exclusively through `AppClient`:

```typescript
// apps/tauri/src/renderer/lib/app-client.ts
const client = useAppClient();
const result = await client.ai.generate(req);
```

`AppClient` is backed by `createTauriInvokeClient()` in production, and `createMockClient()` in tests — making the UI completely portable.

---

## Database Schema

### SQLite (Drizzle ORM)

```mermaid
erDiagram
    documents {
        text id PK
        text name
        text path
        text format
        text language
        text content
        integer pageCount
        integer isDefault
        text createdAt
    }

    jobs {
        text id PK
        text boardId
        text title
        text company
        text location
        text url
        text description
        text salary
        text remote
        text status
        text appliedAt
        text scrapedAt
    }

    interactions {
        text id PK
        text jobId FK
        text type
        text createdAt
    }

    ai_generations {
        text id PK
        text type
        text documentId FK
        text jobId FK
        text model
        text provider
        text content
        text createdAt
    }

    conversations {
        text id PK
        text role
        text content
        text model
        text createdAt
    }

    job_preferences {
        text id PK
        text query
        text location
        text boards
        text salary
        integer remote
        text createdAt
    }

    credentials {
        text id PK
        text board
        text username
        text createdAt
    }

    jobs ||--o{ interactions : "has"
    jobs ||--o{ ai_generations : "generates"
    documents ||--o{ ai_generations : "uses"
```

### LanceDB Collections

| Collection      | Schema                                              | Purpose                |
| --------------- | --------------------------------------------------- | ---------------------- |
| `jobs`          | `{id, vector[1024], text, boardId, title, company}` | Semantic job search    |
| `resumes`       | `{id, vector[1024], text, documentId, chunkIndex}`  | Resume similarity      |
| `skills`        | `{id, vector[1024], text, category}`                | Skill taxonomy lookup  |
| `conversations` | `{id, vector[1024], text, role, timestamp}`         | Conversation retrieval |

---

## Key Design Decisions

### 1. Local-First Architecture

All data lives on the user's machine — SQLite, LanceDB, credential keychain. No account signup, no cloud sync. This is a deliberate product decision: the target user is privacy-conscious and may be searching confidentially.

### 2. IPC Contract as Single Source of Truth

`packages/shared` is the only place where renderer ↔ Rust interaction is defined. This prevents drift between frontend expectations and backend implementation, and enables mock-based testing without Tauri.

### 3. Ports & Adapters for AppClient

The renderer never calls `window.__TAURI_INVOKE__` directly. It uses `AppClient` which can be swapped to a mock, enabling UI-only development (`pnpm dev:frontend`) and Vitest tests without the full Tauri runtime.

### 4. Native Rust Runtimes

Heavy work (scraping, OCR, embeddings) runs natively in the Rust core on Tauri's async runtime and `tokio` tasks — there is no separate Node.js process. Long operations are spawned as background tasks so they don't block command handling, and OCR runs in the renderer via Tesseract.js (its own Web Worker).

### 5. Streaming as First-Class Concern

AI generation, scraping progress, and autopilot step events all use Tauri's `emit` mechanism to push server-sent events to the renderer. This drives a reactive UI without polling.

### 6. Feature-Scoped Components

The renderer uses a `features/` directory where each feature owns its components and can only import from `packages/ui`, `services/`, and `lib/`. Cross-feature imports are ESLint-forbidden, keeping boundaries explicit.

### 7. Minimal State Machine Library

Rather than XState, the app uses a micro state machine implementation (`lib/machine.ts`, ~80 lines) with a `useMachine` hook. This keeps bundle size minimal and the mental model simple for flows with ≤ 10 states.

---

## External Integrations

| Integration      | Protocol                 | Auth                         | Purpose                      |
| ---------------- | ------------------------ | ---------------------------- | ---------------------------- |
| Ollama           | HTTP (undici)            | None (local)                 | Chat generation + embeddings |
| OpenAI           | HTTPS (REST)             | API key (keychain)           | Cloud generation fallback    |
| Anthropic        | HTTPS (REST)             | API key (keychain)           | Extended thinking generation |
| Google Gemini    | HTTPS (REST)             | API key (keychain)           | Multilingual generation      |
| LM Studio        | HTTP (OpenAI-compatible) | Optional                     | Local cloud-replacement      |
| Job boards (18+) | Playwright browser       | Board credentials (keychain) | Scraping                     |
| OS Keychain      | Tauri plugin             | OS auth                      | Credential encryption        |

---

## Package Dependency Rules

```mermaid
graph TD
    Renderer["apps/tauri renderer"]
    Shared["packages/shared"]
    UI["packages/ui"]
    Core["packages/core"]
    AI["packages/ai"]
    Data["packages/data"]
    Prompts["packages/prompts"]
    Workers["packages/workers"]

    Renderer --> Shared
    Renderer --> UI
    AI --> Shared
    Data --> Shared
    Workers --> Shared
    Prompts --> Shared

    style Renderer fill:#4f46e5,color:#fff
    style UI fill:#7c3aed,color:#fff
    style Shared fill:#0f766e,color:#fff
```

**Hard rules:**

- `packages/shared` — no React, no Node APIs, no UI
- `packages/ui` — no Zustand, no IPC, no routing
- `packages/prompts` — no UI, no `window`
- Renderer **never** imports from `@ajh/core`, `@ajh/ai`, `@ajh/data`, `@ajh/workers`
