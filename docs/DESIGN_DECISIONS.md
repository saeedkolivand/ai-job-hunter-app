# AI Job Hunter — Design Decisions

Last updated: 2026-07-17

This document records the major architectural decisions in the project — the reasoning behind the technology choices, the patterns used, and the trade-offs considered. It is a reference for contributors and reviewers who want to understand the _why_ behind the codebase.

---

## 1. Overview

AI Job Hunter is a local-first desktop application built on [Tauri][tauri] 2, with a [Rust][rust] backend as the process host and an OS-native WebView running the [React][react] frontend. It automates the mechanical parts of job searching — scraping job boards, scoring postings against a résumé with embeddings, and generating tailored résumés and cover letters — and includes an autonomous apply flow (Autopilot). Everything runs locally; there is no cloud backend.

The two halves communicate over a typed IPC layer: the contract namespaces are defined in a shared package so there is no drift between what the frontend calls and what Rust implements. Heavier work (scraping, OCR, embeddings, document export) runs natively in the Rust core; long operations are spawned as background `tokio` tasks tracked by a SQLite-backed job tracker with retry, so they don't block command handling.

The frontend uses [TanStack Router][tanstack-router] for routing, [TanStack Query][tanstack-query] for server state, [Zustand][zustand] for client state, and a small custom state machine for multi-step flows. All UI primitives come from a private `@ajh/ui` component library in the same monorepo, which enforces the design-token system through [ESLint][eslint] rules. The repository is set up for production: [Conventional Commits][conventional-commits], [semantic-release][semantic-release] for automated versioning, [Turborepo][turborepo] for incremental builds, [Husky][husky] pre-commit hooks that block lint errors, and a [Vitest][vitest] suite spanning the packages.

---

## 2. Architecture Decisions — With Reasoning

Each entry explains the _why_, not just the _what_.

---

### Tauri over Electron

[Tauri][tauri] uses the OS-native WebView (Edge/WebView2 on Windows, WebKit on macOS/Linux) instead of bundling Chromium. The result:

- **Smaller installer** (tens of MB) versus 150+ MB for a typical Electron app
- **Rust backend** instead of Node.js — low memory overhead for background tasks
- **Tighter security model** — the backend exposes only the commands explicitly allowed via Tauri's capability manifest

The trade-off: WebKit on macOS doesn't always render identically to WebView2 on Windows. That is handled with careful CSS and a cross-platform pass before each release.

---

### Monorepo structure

The app has clear, enforced package boundaries:

```
packages/shared       ← IPC contracts, Zod schemas (no React, no Node)
packages/ui           ← component library (no IPC, no state management)
packages/prompts      ← AI prompt templates, provider-aware + locale-driven (pure TypeScript)
packages/translations ← i18next + en/de resources (no app/IPC deps)
packages/test-ids     ← central TEST_IDS map
apps/desktop          ← Tauri app: Rust core + React renderer
apps/extension        ← MV3 browser extension (Chrome + Firefox): job import + opt-in autofill
```

The heavy work — scraping, AI, documents, embeddings — lives in the Rust core under `apps/desktop/src-tauri/`. (An earlier design ran some of this in a separate Node.js sidecar; it was folded into Rust to drop a process and a language from the runtime.)

Each package has its own `tsconfig`, build step, and test suite. [Turborepo][turborepo]'s dependency graph keeps builds incremental — if `packages/shared` hasn't changed, nothing that depends on it rebuilds. The key discipline is that **ESLint hard-blocks cross-boundary imports** (e.g. `packages/shared` may not import React or Node APIs); it is enforced at lint time and blocks commits, not just a convention.

---

### The IPC contract pattern

Every renderer → Rust interaction is defined in one place: `packages/shared/src/ipc/contracts/`. The flow is:

```
UI Component → Service Hook (React Query) → AppClient → IPC Contract → Tauri bridge → Rust command
```

`AppClient` is injected via React context. In production it is backed by `createTauriInvokeClient()`; in tests by `createMockClient()`. The UI is therefore portable — the entire React frontend can run against mocked data without Tauri running at all.

This is the Ports and Adapters (Hexagonal) pattern: `AppClient` is the port, `TauriInvokeClient` and `MockClient` are the adapters, and the UI only knows about the port interface.

---

### AI streaming

Streaming uses Tauri's event system rather than a request/response pattern:

1. The UI calls `client.ai.generate(req)` and gets back a `generationId` immediately
2. The UI subscribes to `client.ai.onStream(handler)` — a Tauri event listener
3. Rust receives SSE chunks from the LLM and emits each delta as a Tauri event to the renderer
4. The streaming component appends each delta to the output buffer
5. When `chunk.done === true`, the UI unsubscribes and transitions the state machine forward

The state machine matters here: streaming moves through explicit states (`idle → configuring → generating → extracting → done`). Without it, the flow would be a tangle of booleans; with it, each valid state is named and transitions are enforced.

---

### Custom state machine over XState

The flows in this app have at most a handful of states. XState is powerful but adds bundle weight, introduces its own config DSL, and is overkill for short linear flows.

The project uses a micro state machine (~80 lines, `lib/machine.ts`) and a `useMachine(machine)` hook covering:

- State transitions via `send(event)`
- `busyStates` — when the machine is loading
- `errorStates` — when to show an error UI

For flows like an onboarding wizard or document generation, that is sufficient. The trade-off is no parallel states, history, or guards — none of which are needed here, and XState remains an option if they ever are.

---

### Hybrid search

Posting search combines two signals, computed in the Rust core (`commands/search.rs`, `search_hybrid`):

1. **Semantic similarity** — the query is embedded via the active provider (`documents::embed`), then **cosine similarity** is computed against stored posting embeddings. This surfaces relevant results even when exact keywords don't match ("senior engineer" matching "staff software engineer").
2. **Keyword overlap** — term overlap between the query and the posting text.

The two scores are combined into a weighted score and the top-K hits are returned. (Embeddings are stored in SQLite alongside the documents, not in a separate vector engine — the corpus is small enough that an in-memory cosine pass is fast and avoids another dependency.)

---

### State management

There are two separate state concerns:

**Server state** (data from IPC/[Rust][rust]) is managed entirely by [TanStack Query][tanstack-query]. Every IPC call has a corresponding service hook (`use-jobs.ts`, `use-documents.ts`, etc.) handling caching, background refetch, optimistic updates, and loading/error states. This replaces the `useState + useEffect` data-fetching antipattern.

**Client state** (UI-only) lives in [Zustand][zustand] stores (e.g. persisted user preferences, transient generation session). [Zustand][zustand] is used over Redux because the stores are simple, there is no boilerplate, and it integrates well with [React][react] 19.

---

### Credential storage

API keys and job-board passwords are stored in the OS-native keychain:

- Windows: Credential Manager (DPAPI encryption)
- macOS: Keychain
- Linux: libsecret (GNOME Keyring / KWallet)

The Rust backend uses the `keyring-core` crate with platform-specific adapters; the Tauri process registers the backend via `init_keyring()` at startup. The renderer calls credential commands through the IPC contract and never handles raw secrets. This is security-by-design: even with a renderer XSS, secrets are not reachable because they live outside the web context.

---

### Backup and restore

Each backup-able persistent store implements a single `DataStore` trait — `export() -> Value` and `import(&Value)` with REPLACE semantics. The `data.export` / `data.import` commands assemble one versioned JSON bundle (`{ version, exportedAt, stores }`) read/written via the dialog plugin; the user backs up or restores from Settings → Privacy.

Credentials (in the OS keychain), ephemeral caches, and the transient job-execution log are deliberately excluded. Restore is a full replace, not a merge, so a backup file is an exact snapshot. The `DataStore` trait keeps the data layer uniform without a heavyweight ORM, and the bundle is the foundation any future sync feature would build on.

---

### Component library

`packages/ui` is a standalone React component library consumed as `@ajh/ui` within the monorepo. It provides:

- **Design tokens** as CSS custom properties (`--color-brand`, `--color-surface-elevated`, etc.)
- **TailwindCSS v4** consuming those tokens via `@theme`
- **Motion presets** — named tokens (`transition.fast`, `transition.spring`, …) instead of inline `{ duration, ease }` objects
- **ESLint enforcement** — custom rules block raw `<button>`/`<select>`/`<textarea>`, hardcoded hex colors, and inline transition objects in feature code

The library has no IPC, no Zustand, and no routing — it is a pure UI package.

---

### Document processing pipeline

When a user imports a PDF, DOCX, TXT, or image:

1. **Format detection** — Rust inspects the extension and magic bytes
2. **Text extraction** — PDF and DOCX via dedicated parsers; images via OCR
3. **Storage** — raw text and metadata are saved to SQLite
4. **Chunking** — text is split into chunks for embedding
5. **Embedding** — each chunk is vectorized via the active embedding provider
6. **Vector storage** — embeddings are stored in `documents.db` for later similarity search

Progress events are emitted at each stage so the UI can show a live indicator.

---

## 3. Frontend Patterns

---

### React Query service-hook pattern

Every piece of server state follows the same shape:

```typescript
// A query
export function useJobs(filters?: JobFilters) {
  const client = useAppClient();
  return useQuery({
    queryKey: queryKeys.jobs.list(filters),
    queryFn: () => client.jobs.list(filters),
    staleTime: 5 * 60 * 1000,
  });
}

// A mutation with cache invalidation
export function useDeleteJob() {
  const client = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => client.jobs.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.jobs.lists() }),
  });
}
```

All query keys live in a centralized `queryKeys` object so invalidation is reliable. Service hooks are the only place IPC is called — components never call `client.*` directly.

---

### Feature isolation

The `features/` directory enforces a strict ownership model:

```
features/
  ai-generate/      ← owns its own components, hooks, types
  jobs/             ← never imports from ai-generate/
  autopilot/
  resumes/
```

Each feature exposes a narrow public surface; everything else is private, and cross-feature imports are forbidden by ESLint. A feature can be refactored without breaking another because they have zero coupling.

---

### State machine pattern

```typescript
// Step 1: define the machine
export const generationMachine = defineMachine({
  initial: 'idle',
  transitions: {
    idle:       { start: 'configuring' },
    configuring:{ generate: 'generating', reset: 'idle' },
    generating: { extract: 'extracting', fail: 'error', reset: 'idle' },
    extracting: { complete: 'done', fail: 'error' },
    done:       { reset: 'idle' },
    error:      { reset: 'idle' },
  },
  busyStates: ['generating', 'extracting'],
  errorStates: ['error'],
});

// Step 2: use it in a component
const [state, send] = useMachine(generationMachine);

// Step 3: render by state, not by flag
{state === 'generating' && <StreamingText text={delta} />}
{state === 'error'      && <ErrorState retry={() => send('reset')} />}
{state === 'done'       && <OutputPanel output={result} />}
```

A state machine avoids impossible states (`isLoading && isDone`): every valid state is named and exactly one is active at a time.

---

### i18n wrapper pattern

Never import `react-i18next` directly:

```typescript
// ✅ correct
import { useTranslation } from '@ajh/translations';

// ❌ wrong — ESLint error
import { useTranslation } from 'react-i18next';
```

The wrapper in `lib/i18n.ts` provides a consistent namespace and isolates the underlying i18n library behind one module, so it can be swapped without touching component files.

---

### Zod validation

IPC request/response payloads and user-facing form data are validated with Zod:

```typescript
// packages/shared/src/schemas/job.ts
export const JobRecordSchema = z.object({
  id: z.string().uuid(),
  title: z.string().min(1),
  company: z.string().min(1),
  location: z.string().optional(),
  salary: z.string().optional(),
  remote: z.enum(['remote', 'hybrid', 'onsite']).optional(),
  status: z.enum(['new', 'saved', 'applied', 'rejected', 'interviewing']),
  scrapedAt: z.number().int().positive(),
});

export type JobRecord = z.infer<typeof JobRecordSchema>;
```

Validation happens at system boundaries (IPC receive, form submit). Inside the app the TypeScript types are trusted — no defensive `if (!data?.id)` checks scattered through component logic.

---

## 4. TypeScript Patterns

### Strict configuration

The project uses TypeScript 6 with `strict: true`, `noUncheckedIndexedAccess: true`, and `exactOptionalPropertyTypes: true`, which catches undefined array access, optional-vs-absent property distinctions, and missing null checks.

### Discriminated unions for state

```typescript
// ❌ ambiguous
interface GenerationState {
  status: string;
  output?: string;
  error?: string;
}

// ✅ discriminated union — every state is complete and exclusive
type GenerationState =
  | { status: 'idle' }
  | { status: 'generating'; generationId: string }
  | { status: 'done'; output: string; metadata: GenerationMeta }
  | { status: 'error'; message: string };
```

A `switch` on `state.status` narrows the type in each branch — `output` only exists when `status === 'done'`.

### `as const` for query keys

```typescript
export const queryKeys = {
  jobs: {
    all: () => ['jobs'] as const,
    lists: () => ['jobs', 'list'] as const,
    list: (filters?: JobFilters) => ['jobs', 'list', filters] as const,
    detail: (id: string) => ['jobs', 'detail', id] as const,
  },
} as const;
```

`as const` makes the tuple types literal rather than `string[]`, which React Query relies on for precise cache invalidation.

---

## 5. Testing Approach

The suite uses Vitest, split across packages:

| What's tested      | Approach                                             |
| ------------------ | ---------------------------------------------------- |
| IPC contract types | Type-level tests with `expectTypeOf`                 |
| Service hooks      | `renderHook` + mock AppClient                        |
| Utility functions  | Pure unit tests                                      |
| State machines     | `send()` + assert state                              |
| Rust commands      | `#[cfg(test)]` modules with `tempfile` in-memory DBs |
| Board scrapers     | HTTP fixtures (no real network calls)                |

The mock AppClient (`createMockClient()`) is the key enabler for testing UI in isolation: any feature component can be rendered with a mock that returns deterministic data, without Tauri running.

---

## 6. Performance Considerations

### Off-thread CPU-intensive work

OCR runs off the UI thread; text chunking, embedding, and document extraction run in the Rust core as background `tokio` tasks, keeping both the renderer and command handling responsive.

### Performance mode

The app offers three backend tiers (low-memory, balanced, performance) + a custom mode where users tune per-element visuals and backend knobs independently. The frontend resolves a `PerformanceProfile` to a concrete `PerformanceBackendConfig` IPC payload. The backend holds a process-global `OnceLock<ArcSwap<PerformanceConfig>>` (L0 module) that subsystems read without requiring an AppHandle — this pattern enables the Ollama embed builder (a free function) to query the live keep-alive setting. Three backend knobs are now real: (1) scraper concurrency threaded to `ScraperEngine::set_concurrency(n)`, (2) Ollama keep-alive (seconds) flowing into model/chat/embeddings request bodies, (3) cache TTL + row-cap bounding on match-scores and posting-vectors tables. Tier values and custom overrides are resolved in one place (`resolveBackendConfig` in preferences-schema.ts); the backend never branches on mode string. See ADR-019 (performance profile) for full architecture and tier definitions.

### Streaming rendering

AI output appends delta characters to a buffer rather than re-rendering the full string on each chunk, which avoids layout thrashing during generation.

### Incremental monorepo builds

Turborepo tracks file hashes per package, so unchanged packages skip their build — this meaningfully shortens CI build time after the first run.

---

## 7. Build & Release Pipeline

### Automated versioning

The repo uses [semantic-release][semantic-release] driven by [Conventional Commits][conventional-commits]:

| Commit prefix                  | Release type |
| ------------------------------ | ------------ |
| `feat:`                        | minor        |
| `fix:`, `perf:`                | patch        |
| `BREAKING CHANGE` footer       | minor (0.x)  |
| `chore:`, `docs:`, `refactor:` | no release   |

Releases are **manually triggered** (Actions → "🚀 Release" → `action: release`); semantic-release then bumps the version, publishes release notes to the GitHub release, and (via a separate dispatch) triggers the Tauri build pipeline. Nothing auto-runs on push/merge to `main`. While the project is on `0.x`, a `BREAKING CHANGE` bumps the minor, not the major (`release.config.mjs` pre-1.0 guard).

### Pre-commit hooks ([Husky][husky] + [lint-staged][lint-staged])

Every commit runs `eslint --fix` on staged [TypeScript][typescript], [Prettier][prettier] on the rest, and [commitlint][commitlint] on the message. Pre-push runs the full gate (typecheck, lint, `cargo check`/`test`/`clippy`, formatting). The effect is that the main branch never carries a lint or type error.

---

## 8. Rust Backend

### Why Rust for the backend

- **Memory safety** — no GC pauses during scraping or file processing; [Tokio][tokio] handles concurrent board scraping without an OS thread per request.
- **SQLite ownership** — [rusqlite][rusqlite] with the bundled feature ships [SQLite][sqlite] inside the binary, so there is no external database process or version mismatch.
- **OS integration** — keychain, file dialogs, tray icon, single-instance, window-state, notifications, and the auto-updater are all thin [Rust][rust] wrappers over [Tauri][tauri] plugins.

### SQLite design

Several independent [SQLite][sqlite] databases, each with its own [Rust][rust] struct and `Mutex<Connection>` (using [parking_lot][parking-lot] for poison-free locking):

| Database             | Purpose                                        |
| -------------------- | ---------------------------------------------- |
| `documents.db`       | Résumé/document metadata + embeddings          |
| `conversations.db`   | Chat history                                   |
| `ai_generations.db`  | Generation + application records               |
| `job_preferences.db` | User search preferences                        |
| `contact_profile.db` | Contact profile used for document headers      |
| `jobs.db`            | Background job-execution tracker (retry state) |
| `pipeline_cache.db`  | Company-research and OCR cache (TTL)           |

Keeping them separate means a corrupt `conversations.db` doesn't affect `documents.db`. The job-execution tracker and the cache are excluded from backups (transient/ephemeral).

### Concurrent scraping

Board scrapers run in parallel via [Tokio][tokio]'s `spawn`. Each scraper holds a `CancellationToken` so a scrape can be stopped mid-run, and results are streamed to the renderer via `app.emit()` as they arrive.

---

## 9. Known Limitations & Future Work

1. **Type-safe IPC end to end** — the TypeScript contracts are typed, but the Rust side still returns `Value` (untyped JSON) for most commands. Generating TypeScript bindings directly from Rust types (e.g. `tauri-specta`) would remove the manual contract maintenance.
2. **Database consolidation** — the per-domain SQLite files keep faults isolated but add overhead for cross-store queries; consolidating into one database with a migration framework is a candidate once tooling versions line up.
3. **End-to-end tests** — unit/integration coverage is solid, but there is no E2E suite driving the full Tauri process; a Playwright-based suite would close that gap.
4. **Observability** — log files exist (via `tauri-plugin-log`), but there is no structured tracing; spans around the generation pipeline would make latency sources visible.

---

## 10. Technology Reference

| Technology                               | Why                                                                          |
| ---------------------------------------- | ---------------------------------------------------------------------------- |
| **[Tauri][tauri] 2**                     | Smaller binary than Electron; [Rust][rust] backend; OS-native WebView        |
| **[React][react] 19**                    | Concurrent features; first-class TanStack support                            |
| **[TypeScript][typescript] 6**           | `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes` for stricter safety |
| **[TanStack Router][tanstack-router]**   | File-based routing with typed route + search params                          |
| **[TanStack Query][tanstack-query]**     | Removes `useEffect` data fetching; caching + background refetch              |
| **[Zustand][zustand]**                   | Minimal client-state store; [React][react] 19 concurrent-safe                |
| **[Tailwind CSS][tailwindcss] v4**       | CSS-first config via `@theme`; zero runtime                                  |
| **[motion/react][motion-react]**         | High-performance animation library                                           |
| **[Zod][zod]**                           | Schema-first validation with type inference                                  |
| **[Turborepo][turborepo]**               | Incremental monorepo builds                                                  |
| **[Vitest][vitest]**                     | Fast, [Vite][vite]-native test runner                                        |
| **[semantic-release][semantic-release]** | Automated versioning from commit messages                                    |
| **[parking_lot][parking-lot]**           | `Mutex` replacement with no lock poisoning                                   |
| **[rusqlite][rusqlite]**                 | Embedded [SQLite][sqlite] bundled into the binary                            |

---

## 11. AI-Assistant Agent System

The repository ships a Claude Code agent system under `.claude/` (tracked alongside source). The system consists of 12 specialized agents covering distinct ownership domains — `resume-export-expert`, `job-match-expert`, `scraping-applier-expert`, `ai-provider-expert`, `rust-backend-architect`, `tauri-security-reviewer`, `frontend-reviewer`, `performance-profiler`, `testing-reviewer`, `test-author`, `pdf-docx-generator`, and `project-steward` — plus a set of slash commands: `/review-{rust,security,performance,ats,resume,template,export,frontend,scraping,ai}`, `/implement-feature`, `/fix-bug`, `/refactor-module`, `/add-tests`, `/update-docs`, and `/prepare-release`.

The system also includes domain skills and checklists (`.claude/skills/`), a Stop review-gate hook that runs on every diff and blocks only HIGH/CRITICAL findings, and a distilled lessons log (`.claude/memory/lessons.jsonl`).

The per-change flow is: Primary Owner agent makes the change → review pass against the owner's checklist (HIGH/CRITICAL findings block, others are advisory; ≤ 3 reviewers total) → if the change touches testable logic, `test-author` writes tests and `testing-reviewer` audits coverage → `project-steward` closes by syncing documentation, knowledge files, ADRs, and the lessons log.

`CLAUDE.md` is the source of truth for the full operating contract, agent routing table, and enforced rules. The same domain-routing and review conventions apply when other AI tools work in this repository — see `AGENTS.md`, `.clinerules`, `.windsurfrules`, and `.github/copilot-instructions.md` for the per-tool rules files that document the conventions those assistants follow.

[tauri]: https://tauri.app
[react]: https://react.dev
[rust]: https://www.rust-lang.org
[typescript]: https://www.typescriptlang.org
[tanstack-router]: https://tanstack.com/router
[tanstack-query]: https://tanstack.com/query
[zustand]: https://github.com/pmndrs/zustand
[tailwindcss]: https://tailwindcss.com
[motion-react]: https://motion.dev
[zod]: https://zod.dev
[turborepo]: https://turborepo.com
[vitest]: https://vitest.dev
[vite]: https://vite.dev
[semantic-release]: https://github.com/semantic-release/semantic-release
[conventional-commits]: https://www.conventionalcommits.org
[husky]: https://typicode.github.io/husky
[lint-staged]: https://github.com/lint-staged/lint-staged
[commitlint]: https://commitlint.js.org
[eslint]: https://eslint.org
[prettier]: https://prettier.io
[tokio]: https://tokio.rs
[sqlite]: https://www.sqlite.org
[rusqlite]: https://github.com/rusqlite/rusqlite
[parking-lot]: https://github.com/Amanieu/parking_lot
