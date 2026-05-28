# AI Job Hunter — Design Decisions

This document covers every major architectural decision in the project — the reasoning behind technology choices, patterns used, and the trade-offs considered. It is a reference for contributors, reviewers, and anyone who wants to understand the _why_ behind the codebase.

---

## 1. Elevator Pitch

### 30-Second Version (recruiter / HR screen)

> "AI Job Hunter is a full-stack desktop application I built with Tauri, React, and Rust. It automates the mechanical parts of job searching — scraping 18+ job boards, scoring postings against your resume using vector embeddings, generating tailored cover letters, and auto-applying. Everything runs locally on your machine with no cloud backend. I designed it as a production-grade monorepo with a strict ports-and-adapters architecture, a typed IPC contract layer, a custom component library, and automated releases."

### 2-Minute Version (technical interview)

> "The app is a local-first desktop application built on Tauri 2. Tauri uses a Rust backend as the process host and a Chromium WebView for the React frontend. Those two halves communicate over a typed IPC layer — I defined 21 typed contract namespaces in a shared package so there's no drift between what the frontend calls and what Rust implements.
>
> The heavier work — scraping, OCR, embeddings — runs in a Node.js sidecar process so it doesn't block Tauri's async runtime. The sidecar has its own runtime manager, a typed pub/sub event bus, and a job queue with retry.
>
> On the frontend I used TanStack Router for file-based routing, TanStack Query for all server state, Zustand for client state, and a custom micro state machine for multi-step flows. All UI primitives come from a private `@ajh/ui` component library I built in the same monorepo, which enforces the design token system via ESLint rules.
>
> The project is fully set up for production: Conventional Commits, semantic-release for automated versioning, Turborepo for incremental builds, Husky pre-commit hooks that block any lint error from reaching the repo, and a Vitest suite covering 437 test cases."

---

## 2. Architecture Decisions — With Reasoning

These are the questions interviewers actually ask. Each answer explains the _why_, not just the _what_.

---

### "Why Tauri instead of Electron?"

Tauri uses the OS-native WebView (Edge on Windows, WebKit on macOS/Linux) instead of bundling Chromium. The result:

- **50–80 MB installer** vs 150+ MB for Electron apps
- **Rust backend** instead of Node.js — near-zero memory overhead for background tasks
- **Better security model** — the backend exposes only the commands you explicitly allow via Tauri's capability manifest

The trade-off: WebKit on macOS doesn't always render identically to Edge on Windows. I handled that with careful CSS and a cross-platform test pass before each release.

---

### "Why did you use a monorepo?"

The app has clear, enforced package boundaries:

```
packages/shared    ← IPC contracts, Zod schemas (no React, no Node)
packages/ui        ← component library (no IPC, no state management)
packages/prompts   ← AI prompt templates (pure TypeScript)
packages/core      ← event bus, job queue, logger
packages/ai        ← Ollama client, AI runtime
packages/workers   ← Web Workers (OCR, embeddings)
```

Each package has its own `tsconfig`, build step, and test suite. Turborepo's dependency graph means builds are incremental — if `packages/shared` hasn't changed, nothing that depends on it rebuilds. This keeps `pnpm build` fast even as the project grows.

The key discipline: **ESLint hard-blocks cross-boundary imports**. The renderer can't import from `@ajh/core` or `@ajh/data`. This isn't just a convention — it's enforced at lint time and blocks commits.

---

### "What is the IPC contract pattern?"

Every renderer → Rust interaction is defined in one place: `packages/shared/src/ipc/contracts/`. The pattern is:

```
UI Component → Service Hook (React Query) → AppClient → IPC Contract → Tauri bridge → Rust command
```

`AppClient` is injected via React context. In production it's backed by `createTauriInvokeClient()`. In tests it's backed by `createMockClient()`. The UI is completely portable — you can run `pnpm dev:frontend` to develop the entire React frontend against mocked data without Tauri running at all.

**Interview follow-up:** _This is the Ports and Adapters (Hexagonal Architecture) pattern_. `AppClient` is the port. `TauriInvokeClient` and `MockClient` are the adapters. The UI only knows about the port interface.

---

### "How does AI streaming work?"

Streaming uses Tauri's event system rather than a request/response pattern:

1. UI calls `client.ai.generate(req)` → gets back a `generationId` immediately
2. UI subscribes to `client.ai.onStream(handler)` — this is a Tauri event listener
3. Rust receives SSE chunks from the LLM, emits each delta as a Tauri event to the renderer
4. The `StreamingText` component appends each delta to the output buffer
5. When `chunk.done === true`, the UI unsubscribes and transitions the state machine to `extracting`

The state machine is important here: streaming goes through states `idle → configuring → generating → extracting → done`. Without the machine, you'd manage this with a tangle of booleans. With the machine, each valid state is explicit and transitions are enforced.

---

### "Why did you write your own state machine instead of using XState?"

The flows in this app have 5–8 states at most. XState is a powerful library but it adds ~20 KB to the bundle, requires learning its own config DSL, and is overkill for simple linear flows.

I wrote a micro state machine in ~80 lines of TypeScript (`lib/machine.ts`) and a `useMachine(machine)` hook. It covers:

- State transitions via `send(event)`
- `busyStates` — know when the machine is loading
- `errorStates` — know when to show an error UI

For a flow like "onboarding wizard" or "document generation", this is all you need. The trade-off is that it doesn't support parallel states, history, or guards — but none of those are needed here, and you can always swap to XState later if they become needed.

---

### "How does the search work?"

Hybrid search combines two signals:

1. **Semantic search** — the user's query is embedded via Ollama (or whichever provider is active), then an ANN (approximate nearest neighbor) search runs against LanceDB vectors. This finds semantically relevant results even when the exact keywords don't match ("senior engineer" matching "staff software engineer").

2. **Keyword/filter search** — SQL WHERE conditions narrow the ANN candidates by metadata (location, salary, remote flag, board, etc.)

The results are re-ranked using a weighted score:

```
finalScore = semanticWeight × semanticScore + (1 − semanticWeight) × keywordScore
```

A `semanticWeight` of 0.7 gives 70% weight to vector similarity and 30% to keyword relevance. The user can tune this in the search UI.

---

### "How is state management structured?"

There are two separate state concerns:

**Server state** (data from IPC/Rust): all managed by TanStack Query. Every IPC call has a corresponding service hook (`use-jobs.ts`, `use-documents.ts`, etc.). React Query handles caching, background refetch, optimistic updates, and loading/error states. This replaces `useState + useEffect` for remote data — an antipattern that's common in less mature codebases.

**Client state** (UI-only): Zustand stores. Currently two stores: `preferences-store` (persisted user settings) and `session-store` (transient session data like current generation ID). Zustand was chosen over Redux because the stores are simple, there's no boilerplate, and it integrates well with React 19's concurrent features.

---

### "How does credential storage work?"

API keys and job board passwords are stored in the OS native keychain:

- Windows: Credential Manager (DPAPI encryption)
- macOS: Keychain Access
- Linux: libsecret (GNOME Keyring / KWallet)

The Rust backend uses the `keyring-core` crate with platform-specific store adapters. The Tauri process calls `init_keyring()` at startup to register the platform backend. The renderer calls `client.credentials.set()` / `.get()` through the IPC contract — it never handles raw secrets.

**Interview follow-up:** This is intentional security-by-design. Even if the renderer had an XSS vulnerability, secrets would not be accessible — they're stored outside the web context entirely.

---

### "How is the component library structured?"

`packages/ui` is a standalone React component library published as `@ajh/ui` within the monorepo. It has:

- **Design tokens** as CSS custom properties (`--color-brand`, `--color-surface-elevated`, etc.)
- **TailwindCSS v4** consuming those tokens via `@theme`
- **Motion presets** — instead of inline `{ duration: 0.2, ease: "easeOut" }` objects, every animation uses a named token (`transition.fast`, `transition.spring`, etc.)
- **ESLint enforcement** — custom rules block raw `<button>`, `<select>`, `<textarea>` elements, hardcoded hex colors, and inline transition objects in feature code

The library has no IPC, no Zustand, no routing. It's a pure UI package. This means you could use `@ajh/ui` in a web app tomorrow with no changes.

---

### "How does the document processing pipeline work?"

When a user imports a PDF, DOCX, TXT, or image:

1. **Format detection** — Rust inspects the file extension and magic bytes
2. **Text extraction** — PDF via `pdf-extract`, DOCX via `docx-rs` parsing, images via `Tesseract.js` OCR (dispatched to a Web Worker so it doesn't block)
3. **Storage** — the raw text and metadata are saved to SQLite
4. **Chunking** — text is split into ~512-token chunks in a Web Worker
5. **Embedding** — each chunk is vectorized via Ollama's `/api/embeddings` endpoint
6. **Vector storage** — vectors are upserted to LanceDB for later ANN search

Progress events are emitted at each stage so the UI can show a live progress indicator.

---

## 3. Frontend Patterns — Deep Dive

These are the patterns an interviewer who cares about frontend architecture will probe.

---

### React Query Service Hook Pattern

Every piece of server state follows this shape:

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

### Feature Isolation

The `features/` directory enforces a strict ownership model:

```
features/
  ai-generate/      ← owns its own components, hooks, types
  jobs/             ← never imports from ai-generate/
  autopilot/
  resumes/
```

Each feature exports exactly one thing from its `index.tsx`. Everything else is private. Cross-feature imports are forbidden by ESLint. This means you can refactor `ai-generate/` without worrying about breaking `jobs/` — they have zero coupling.

---

### State Machine Pattern

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

**Why this is better than booleans:** With `isLoading`, `isError`, `isDone`, `isConfiguring` you can enter impossible states — like `isLoading && isDone`. With a state machine, every valid state is named and only one state is active at a time.

---

### i18n Wrapper Pattern

Never import `react-i18next` directly:

```typescript
// ✅ correct
import { useTranslation } from '@/lib/i18n';

// ❌ wrong — ESLint error
import { useTranslation } from 'react-i18next';
```

The wrapper in `lib/i18n.ts` provides a consistent namespace and lets you swap the underlying i18n library without touching 50+ component files. It's a simple abstraction that pays off if you ever need to migrate.

---

### Zod Validation

All IPC request/response payloads and user-facing form data are validated with Zod 4:

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

Validation happens at system boundaries (IPC receive, form submit). Inside the app, the TypeScript types are trusted. No defensive `if (!data?.id)` checks scattered through component logic.

---

## 4. TypeScript Patterns

### Strict Configuration

The project uses TypeScript 6 with `strict: true`, `noUncheckedIndexedAccess: true`, and `exactOptionalPropertyTypes: true`. This catches:

- Undefined array access (`arr[0]` returns `T | undefined`)
- Optional vs absent property distinction
- Missing null checks

### Discriminated Unions for State

Instead of nullable fields:

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

With a discriminated union, a `switch` on `state.status` narrows the type in each branch — TypeScript knows `output` only exists when `status === 'done'`.

### `as const` for Query Keys

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

`as const` makes the tuple types literal, not `string[]`. React Query uses this for precise cache invalidation.

---

## 5. Testing Approach

The test suite has 437 test cases using Vitest, split across packages:

| What's tested      | Approach                                             |
| ------------------ | ---------------------------------------------------- |
| IPC contract types | Type-level tests with `expectTypeOf`                 |
| Service hooks      | `renderHook` + mock AppClient                        |
| Utility functions  | Pure unit tests                                      |
| State machines     | `send()` + assert state                              |
| Rust commands      | `#[cfg(test)]` modules with `tempfile` in-memory DBs |
| Board scrapers     | Wiremock HTTP fixtures (no real network calls)       |

The mock AppClient (`createMockClient()`) is the key enabler for testing UI in isolation. You can render any feature component and inject a mock that returns deterministic data, without Tauri running.

---

## 6. Performance Considerations

### Web Workers for CPU-Intensive Work

OCR (Tesseract.js), text chunking, and batch embedding run in dedicated Web Workers via `packages/workers`. This keeps the main thread free for React rendering. A cancellation token pattern allows long-running workers to be interrupted mid-operation.

### Performance Mode

The app detects available system memory and CPU cores via `sysinfo` (Rust) and offers three tiers:

| Mode        | Worker threads | Batch size | Use case                      |
| ----------- | -------------- | ---------- | ----------------------------- |
| Low         | 1              | 4          | Older laptops, background use |
| Balanced    | 2              | 16         | Most users                    |
| Performance | 4              | 64         | Desktop workstations          |

### Streaming Rendering

AI output uses TanStack Query's subscription pattern + `StreamingText` component that appends delta characters rather than re-rendering the full string. This prevents layout thrashing during generation.

### Incremental Monorepo Builds

Turborepo tracks file hashes per package. If `packages/ui` hasn't changed, its build is skipped. In CI this typically halves build time after the first run.

---

## 7. Build & Release Pipeline

### Automated Versioning

The repo uses `semantic-release` triggered by Conventional Commits:

| Commit prefix                  | Release type  | Example     |
| ------------------------------ | ------------- | ----------- |
| `feat:`                        | minor (1.x.0) | New feature |
| `fix:`, `perf:`                | patch (1.0.x) | Bug fix     |
| `BREAKING CHANGE` footer       | major (x.0.0) | API break   |
| `chore:`, `docs:`, `refactor:` | no release    | Maintenance |

On merge to `main`, semantic-release automatically: bumps the version, generates a CHANGELOG, creates a GitHub release, and triggers the Tauri build pipeline.

### Pre-commit Hooks (Husky + lint-staged)

Every commit runs:

1. `eslint --fix` on staged TypeScript files
2. `prettier --write` on everything else
3. `commitlint` validates the commit message format

Pre-push runs `tsc --noEmit` across the full monorepo.

**Interview talking point:** _These hooks mean the main branch never has a lint warning. Any PR that breaks ESLint or TypeScript is caught before it reaches review._

---

## 8. Full-Stack Awareness (Rust)

If an interviewer asks about the Rust side (good for senior/lead roles):

### Why Rust for the backend?

- **Memory safety** — no garbage collector pauses during scraping or file processing. Tauri's async runtime (Tokio) handles concurrent board scraping without spawning OS threads per request.
- **SQLite ownership** — `rusqlite` with the bundled feature ships SQLite as part of the binary. No external database process, no version mismatch.
- **OS integration** — keychain access, file system dialogs, tray icon, auto-updater — all via Tauri's plugin system with thin Rust wrappers.

### SQLite design

Five independent SQLite databases, each with its own Rust struct and `Mutex<Connection>` (using `parking_lot` for poison-free locking):

| Database             | Purpose                                     |
| -------------------- | ------------------------------------------- |
| `documents.db`       | Resume/document metadata + embedded vectors |
| `conversations.db`   | Chat history                                |
| `ai_generations.db`  | Generation records + metadata               |
| `job_preferences.db` | User search preferences                     |
| `company_briefs.db`  | 7-day TTL company research cache            |

Keeping them separate means a corrupt `conversations.db` doesn't affect `documents.db`.

### Concurrent scraping

Board scrapers run in parallel using `tokio::spawn`. Each scraper gets a `CancellationToken` so the user can stop a scrape mid-run. Results are collected with `futures::join_all` and streamed to the renderer via `app.emit()` as they arrive.

---

## 9. "What Would You Do Differently?"

This question shows maturity. Good answers for this project:

1. **Type-safe IPC from the start** — I defined TypeScript contracts, but the Rust side still uses `Value` (untyped JSON) for most commands. I'd use `tauri-specta` to generate TypeScript bindings directly from Rust types, eliminating the manual contract maintenance.

2. **Single SQLite database with proper migrations** — Having five separate DB files simplified the early development but adds overhead when querying across them. I'd consolidate into one database with `rusqlite_migration` once a compatible version is available for `rusqlite 0.40`.

3. **E2E tests** — The unit/integration coverage is good, but there are no end-to-end tests exercising the full Tauri process. I'd add a Playwright-based E2E suite that drives the actual desktop app.

4. **Observability** — Production log files exist (via `tauri-plugin-log`), but there's no structured event tracing. I'd add OpenTelemetry spans around the generation pipeline to see where latency actually comes from.

---

## 10. Quick-Reference: Technologies and Why

| Technology           | Why                                                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Tauri 2**          | Smaller binary than Electron; Rust backend; OS-native WebView                                                 |
| **React 19**         | Concurrent features; first-class support in TanStack ecosystem                                                |
| **TypeScript 6**     | `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes` for maximum safety                                   |
| **TanStack Router**  | File-based routing with full type safety on route params and search params                                    |
| **TanStack Query**   | Eliminates `useEffect` data fetching; built-in caching, background refetch                                    |
| **Zustand**          | Minimal boilerplate for simple client state; React 19 concurrent-safe                                         |
| **TailwindCSS v4**   | CSS-first config with `@theme`; zero runtime                                                                  |
| **motion/react**     | Framer Motion v11 rebranded; best-in-class animation performance                                              |
| **Zod 4**            | Schema-first validation with zero-cost type inference                                                         |
| **Pino**             | Fastest Node.js logger; structured JSON output for the scraper-runtime sidecar (Rust uses `tauri-plugin-log`) |
| **Turborepo**        | Incremental monorepo builds with shared remote cache                                                          |
| **Vitest**           | Fast, Vite-native test runner; no Jest config migration needed                                                |
| **semantic-release** | Fully automated versioning driven by commit messages                                                          |
| **parking_lot**      | Drop-in `std::sync::Mutex` replacement with no poisoning risk                                                 |
| **LanceDB**          | Embedded vector DB with no server process; Rust-native                                                        |
| **Tesseract.js**     | Client-side OCR in a Web Worker; no cloud API needed                                                          |
