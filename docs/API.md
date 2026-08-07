# IPC API Reference — AI Job Hunter

Last updated: 2026-08-07

All renderer ↔ Rust communication is defined as typed contracts in `packages/shared/src/ipc/contracts/`. The renderer accesses them exclusively through `AppClient` service hooks.

> **Never call `window.__TAURI_INVOKE__` directly.** Use the service hooks in `apps/desktop/src/renderer/services/`.

---

## Transport

| Direction       | Mechanism                      | Description                |
| --------------- | ------------------------------ | -------------------------- |
| Renderer → Rust | `tauri.invoke(cmd, payload)`   | Request/response (promise) |
| Rust → Renderer | `tauri.listen(event, handler)` | Push events (subscription) |

---

## Namespace Index

| Namespace                         | Description                                |
| --------------------------------- | ------------------------------------------ |
| [ai](#ai)                         | AI generation, model management, streaming |
| [agent](#agent)                   | Agentic application prep flow              |
| [aiGenerations](#aigenerations)   | Generated document metadata                |
| [autopilot](#autopilot)           | Scheduled job-discovery agent              |
| [boards](#boards)                 | Job board management                       |
| [cliAgents](#cliagents)           | CLI agent install management               |
| [contactProfile](#contactprofile) | Candidate contact information              |
| [credentials](#credentials)       | Encrypted credential storage               |
| [dialog](#dialog)                 | Native file dialogs                        |
| [documents](#documents)           | Document import/export                     |
| [geocode](#geocode)               | Location lookup                            |
| [github](#github)                 | GitHub repository import for projects      |
| [jobPreferences](#jobpreferences) | Saved search preferences                   |
| [jobs](#jobs)                     | Job tracker CRUD + events                  |
| [linkedin](#linkedin)             | LinkedIn session management                |
| [match](#match)                   | Resume-job matching                        |
| [privacy](#privacy)               | Privacy settings                           |
| [resume](#resume)                 | Resume management                          |
| [scrape](#scrape)                 | Board scraping                             |
| [search](#search)                 | Hybrid semantic + keyword search           |
| [shortcuts](#shortcuts)           | Keyboard shortcut config                   |
| [support](#support)               | Help, diagnostics, feedback                |
| [system](#system)                 | System info, health, performance           |
| [updater](#updater)               | Auto-update management                     |

---

## `ai`

AI generation, model management, streaming output, and provider configuration.

### Methods

#### `ai.generate(req: GenerateRequest): Promise<GenerateResponse>`

Initiates AI generation. Returns immediately with a `generationId`; content streams via `ai.onStream`.

```typescript
interface GenerateRequest {
  type: 'cover-letter' | 'resume' | 'email' | 'summary';
  resumeId: string;
  jobId?: string;
  jobText?: string;
  language: string; // ISO 639-1 code: "en", "de", etc.
  model: string; // e.g. "mistral", "gpt-4o"
  provider: AIProvider;
  template?: string; // template ID for document export
  intent?: 'deterministic' | 'prose' | 'prose_grounded' | 'default'; // declared sampling intent — each provider adapter picks its own numbers (see AiProvider::sampling_profile); real values wherever the provider accepts them, omitted only where sending is documented-harmful
  temperature?: number; // explicit override — wins over `intent` on every adapter; in practice only ever set by the Ollama per-model "Custom temperature" slider, NOT a universal 0–1/0.3-default knob
  maxTokens?: number; // default 4096
  systemPromptOverride?: string;
}

interface GenerateResponse {
  generationId: string;
}
```

#### `ai.listModels(): Promise<ModelInfo[]>`

Returns available models for the currently configured provider.

```typescript
interface ModelInfo {
  id: string;
  name: string;
  provider: AIProvider;
  contextLength: number;
  isAvailable: boolean;
}
```

#### `ai.pullModel(name: string): Promise<void>`

Downloads an [Ollama][ollama] model. Progress is emitted as `ai:pull-progress` events.

#### `ai.inspectModel(req: { model: string }): Promise<ModelInspectResult | null>`

Returns per-model context-window and max-token limits from [Ollama][ollama] (`/api/show`). Returns `null` for non-local providers or when the Ollama server is unreachable. Used to populate `modelLimits` in the preferences store and show hardware-lag warnings. See `commands/ai.rs: ai_inspect_model`.

#### `ai.researchCompany(req: { jobAd: string; provider?: string; model?: string; baseUrl?: string }): Promise<{ company: string; brief: string }>`

Accepts the full **job ad text** (not a company name). The backend extracts the company internally via the `CompanyResearch` enricher (`cover_letter/research/`), runs the **active provider's own web search + synthesis** (each provider's native `research()` — a native web-search tool, or the Ollama Web Search API for Ollama), and caches the result in `KvCache`. Returns `{ company, brief }`. Degrades gracefully — returns `{ company: "", brief: "" }` when the provider can't search or the search/synthesis fails (or times out), so generation always proceeds. The brief is folded into cover-letter and application-answer prompts as an untrusted-fenced block (see ADR-010). See `commands/ai.rs: ai_research_company`.

#### `ai.embed(req: { text: string; model?: string }): Promise<{ vector: number[]; dim: number; provider: string; model: string } | { error: string }>`

Generates a vector embedding for `req.text` using the active embedding provider/model persisted in the document store (`req.model` is part of the IPC contract but not currently read by the handler — the active config always wins). Input longer than the provider's per-chunk limit is split into chunks, each embedded separately, mean-pooled, and L2-normalized. A context-length overflow triggers an ADAPTIVE-TRUNCATION RETRY internally, inside this same call (the chunk is halved and re-sent, down to a hard floor) — never a separate follow-up request the caller has to issue. Returns the vector + metadata (provider, model, embedding dimensions) on success, or `{ error: string }` on failure (e.g. once truncation hits its floor with no success).

#### `ai.setProviderKey(provider: AIProvider, key: string): Promise<void>`

Stores an API key in the OS keychain for the given provider.

#### `ai.getProviders(): Promise<ProviderConfig[]>`

Returns all configured providers and their availability status.

#### `ai.modelCapabilities(provider: string, model?: string, baseUrl?: string): Promise<{ supportsWebSearch: boolean; supportsReasoning: boolean; effortLevels: string[] }>`

Network-free capability probe for a provider/model combination. Returns:

- `supportsWebSearch` — whether this model can attempt web-grounded research (per `ModelCapabilities` matrix)
- `supportsReasoning` — whether this model reasons at all, NOT whether the app can steer that reasoning via a parameter (that's `effortLevels`, below — narrower). A CLI coding agent is `true` here uniformly across every backend even though the app has no lever to influence its effort on most of them.
- `effortLevels` — the exact set of reasoning-effort values the app can actually SEND for this model (e.g., `["low", "medium", "high"]`). Can be empty even when `supportsReasoning` is true — either the provider/model genuinely doesn't accept the parameter (most CLI agents besides Codex), reasoning is unsupported, or the provider/model is unknown. **Per-model**: several providers' accepted level SET genuinely varies by model tier (Gemini's `thinkingLevel`, Anthropic's `output_config.effort`) — callers should not assume all models of a provider share the same levels.

Degrades gracefully for unknown providers to `{ supportsWebSearch: false, supportsReasoning: false, effortLevels: [] }`. Used by the Settings → AI effort picker to render exactly which effort levels are valid for the chosen model.

#### `ai.listProviderModels(req: { provider: string; baseUrl?: string }): Promise<ProviderModelInfo[]>`

Fetches the live model catalogue from a cloud provider using its stored API key. Raises an error if:

- The provider key is not stored or has been revoked (401/403)
- The provider is unreachable (network error, 5xx)
- The response is malformed or missing the models array

`baseUrl` is forwarded for OpenAI-compatible servers (LM Studio, vLLM, etc.). Never falls back to a cached list when verifying a newly-entered key in onboarding; display surfaces cached by model-pickers if the network fails.

```typescript
interface ProviderModelInfo {
  /** Canonical model ID; always present. */
  name: string;
  /** Human-readable label, when the provider distinguishes it from `name`. */
  displayName?: string;
  /** Unix epoch milliseconds. Normalized across providers' native formats. */
  createdAt?: number;
  /** Max input tokens (context window). */
  contextLength?: number;
}
```

**Important:** No field is guaranteed from every provider. Gemini provides no `createdAt`; OpenAI-compatible servers may omit `contextLength`. Treat missing fields as absent, never as a default value.

### Events

#### `ai.onStream(handler): Unsubscribe`

```typescript
interface StreamChunk {
  id: string; // generationId from generate()
  delta: string; // text fragment
  done: boolean; // true on last chunk
  thinking?: string; // reasoning content — normalized across all providers (OpenAI reasoning_content, Gemini thought parts, Ollama message.thinking, inline <think> blocks)
}
```

#### `ai.onPullProgress(handler): Unsubscribe`

```typescript
interface PullProgress {
  model: string;
  status: 'downloading' | 'verifying' | 'done' | 'error';
  percent: number;
  error?: string;
}
```

### Types

```typescript
type AIProvider = 'ollama' | 'openai' | 'anthropic' | 'gemini' | 'openai-compatible';
```

---

## `agent`

Agentic application-prep flow: a user-facing orchestration that researches a company, matches a resume, drafts a cover letter, and suggests interview questions for a single job. Phase 2 implements the "prep application" flow (read-only tools, display-only proposal). Phase 3 adds the human-in-the-loop confirm gate: when the agent proposes a Write action, it suspends and emits a `confirm_request` step; the renderer collects the user's approval/edits/denial via `agent.confirm()`. See `apps/desktop/src-tauri/src/agent/` (Rust controller, flows, tools, gate) and `apps/desktop/src/renderer/features/jobs/` (UI panel).

### Methods

#### `agent.run(req: AgentRunRequest): Promise<{ jobId: string }>`

Starts the "prep this application" agentic loop for one job. Returns immediately with a `jobId`; progress streams via `agent.onStep()` as `agent:step` events, and the run concludes with a `jobs:event` of type `completed`/`failed`/`cancelled`. When a Write tool is invoked, the run suspends and emits a `confirm_request` step; the agent does not proceed until the renderer calls `agent.confirm()`.

```typescript
interface AgentRunRequest {
  /** Resume id to pass to the tools. */
  resumeId: string;
  /** Job id to pass to the tools. */
  jobId: string;
  /** AI provider ('ollama' | 'openai' | 'anthropic' | 'gemini' | 'openai-compatible'). */
  provider: string;
  /** Model name — must support tool calling (validated server-side). */
  model: string;
  /** Custom base URL for OpenAI-compatible providers (optional). */
  baseUrl?: string;
}
```

#### `agent.confirm(req: AgentConfirmRequest): Promise<{ ok: boolean }>`

Resolves a suspended Write confirmation for a running agent. `ok` is `false` when there is no such pending call (already resolved, timed out, cancelled, or unknown id) — never throws for that case. Edited args may change CONTENT only; the Rust backend re-validates them and rejects any routing/egress fields (all identity stays in the trusted `ToolContext`).

```typescript
interface AgentConfirmRequest {
  /** The agent_run job id this step belongs to. */
  jobId: string;
  /** The pending call id (echoed from the confirm_request step). */
  callId: string;
  /** User's decision: 'approve', 'approveEdited', or 'deny'. */
  decision: 'approve' | 'approveEdited' | 'deny';
  /** Edited tool arguments (content only, required only for 'approveEdited'). */
  editedArgs?: unknown;
}
```

### Events

#### `agent.onStep(handler): Unsubscribe`

```typescript
interface AgentStepEvent {
  /** The agent_run job id this step belongs to. */
  jobId: string;
  /** 1-based turn index (terminal proposal is steps + 1). */
  step: number;
  /** The model's plan/narration text for this step. */
  text: string;
  /** Tool names the model asked to run this turn. */
  tools: string[];
  /** Tool names that were denied (empty in prep flow; Write tools suspend instead). */
  denied: string[];
  /** Step kind: 'turn' (in-loop narration), 'confirm_request' (suspended Write), or 'proposal' (terminal). */
  kind: 'turn' | 'confirm_request' | 'proposal';
  /** Present only on a 'confirm_request' step — the pending Write call to approve. */
  confirm?: AgentConfirmPayload;
}

interface AgentConfirmPayload {
  /** Stable id of this pending call ('{step}-{tool}'); echo in agent.confirm(). */
  callId: string;
  /** The Write tool the agent wants to run (trusted registry name). */
  tool: string;
  /** Args that WILL execute on approval (untrusted model output — render as data). */
  args: unknown;
}
```

---

## `aiGenerations`

Tracks metadata for all AI-generated documents.

#### `aiGenerations.save(record: AIGenerationRecord): Promise<void>`

#### `aiGenerations.list(filters?: AIGenerationFilters): Promise<AIGenerationRecord[]>`

#### `aiGenerations.get(id: string): Promise<AIGenerationRecord>`

#### `aiGenerations.delete(id: string): Promise<void>`

```typescript
interface ApplicationAnswer {
  id: string;
  question: string;
  answer: string;
}

interface AIGenerationRecord {
  id: string;
  type: 'cover-letter' | 'resume' | 'email' | 'summary';
  documentId: string;
  jobId?: string;
  jobUrl?: string; // links record to a FoundJob for applied-status derivation
  board?: string; // board the job came from
  model: string;
  provider: AIProvider;
  content: string;
  language: string;
  template?: string;
  applicationAnswers?: ApplicationAnswer[]; // answered application questions
  companyBrief?: string; // cached company research (untrusted)
  createdAt: string;
}
```

`aiGenerations.save` performs a **per-job merge-upsert by `jobUrl`** (`merge_application` in `ai_generations/mod.rs`): résumé, cover letter, answers, and brief from separate generation actions all land on one row when they share a `jobUrl`. Manual generations without a `jobUrl` insert as separate rows. See `commands/ai_generations.rs`.

---

## `autopilot`

Job-discovery agent — defines and runs scheduled searches that find, rank, and surface
matching jobs (the user tailors & applies with the assistant; there is no auto-apply).

#### `autopilot.create(workflow: WorkflowDefinition): Promise<string>` — returns workflow ID

#### `autopilot.update(id: string, patch: Partial<WorkflowDefinition>): Promise<void>`

#### `autopilot.delete(id: string): Promise<void>`

#### `autopilot.list(): Promise<WorkflowRecord[]>`

#### `autopilot.get(id: string): Promise<WorkflowRecord>`

#### `autopilot.run(id: string): Promise<void>` — triggers immediate run

#### `autopilot.pause(id: string): Promise<void>`

#### `autopilot.resume(id: string): Promise<void>`

#### `autopilot.takePendingFocus(): Promise<string | null>`

Atomically take and clear the autopilot-focus intent buffered by the shell while the app was starting (cold-start deep link `ajh://autopilot/<id>`). A deep-link deep-link emitted during Rust setup fires before the renderer's `onFocus` listener attaches, so the event is lost; the renderer pulls the buffered `autopilotId` once its JS loop is live (on mount and when focus is regained). Returns `null` when nothing is buffered (the common case—only set by a cold-start deep link). Mirrors `menu.takePending()`.

```typescript
interface WorkflowDefinition {
  name: string;
  boards: string[]; // board IDs to scrape
  query: string; // job search query
  location?: string;
  salaryMin?: number;
  salaryMax?: number;
  remote?: 'yes' | 'no' | 'hybrid';
  schedule: WorkflowSchedule;
  resumeId: string;
  coverLetterTemplate?: string;
  customMessage?: string;
  maxApplicationsPerRun?: number;
  assistant?: boolean; // opt-in: generate short AI notes for top matches
  assistantProvider?: string; // provider snapshot at opt-in time
  assistantModel?: string; // model snapshot at opt-in time
  assistantBaseUrl?: string; // base URL snapshot for OpenAI-compatible providers
}

interface WorkflowSchedule {
  type: 'hourly' | 'daily' | 'weekly' | 'manual';
  hour?: number; // for daily/weekly
  dayOfWeek?: number; // for weekly (0=Sunday)
  timezone?: string;
}
```

### Events

#### `autopilot.onStep(handler): Unsubscribe`

```typescript
interface AutopilotStepEvent {
  workflowId: string;
  step: 'scrape_start' | 'scrape_done' | 'rank_done' | 'complete' | 'cancelled' | 'error';
  detail: string;
  timestamp: string;
  jobId?: string;
  error?: string;
}
```

---

## `boards`

Job board configuration management and session import.

#### `boards.list(): Promise<BoardConfig[]>`

#### `boards.enable(boardId: string): Promise<void>`

#### `boards.disable(boardId: string): Promise<void>`

#### `boards.getStats(boardId: string): Promise<BoardStats>`

#### `boards.getConfig(boardId: string): Promise<BoardLoginConfig | null>`

Returns the login configuration for a board, including login URL and available auth predicates.

```typescript
interface BoardLoginConfig {
  id: string;
  displayName: string;
  loginUrl: string;
  hasAuthUrlPredicate: boolean; // true if the board supports URL-based auth detection
  hasAuthCookiePredicate: boolean; // true if the board supports cookie-based auth detection
}
```

#### `boards.catalog(): Promise<BoardCatalogEntry[]>`

Returns the catalog of all available job boards, including authentication requirements and visibility status. Each entry describes the board's scraping mode, login requirements, and whether it is listed in the board picker. The catalog derives from the `Scraper` trait in `apps/desktop/src-tauri/src/scraping/types/mod.rs`; see individual scraper implementations for auth-tier and listing overrides.

```typescript
type BoardAuthRequirement = 'guest' | 'optional' | 'required';

interface BoardCatalogEntry {
  id: string; // e.g. "linkedin", "greenhouse", "aggregator"
  displayName: string; // human-readable board name
  mode: 'http' | 'browser' | string; // scraping transport mode
  auth: BoardAuthRequirement; // login requirement: 'guest' (default), 'optional' (login enriches), 'required' (no results without auth)
  listed: boolean; // whether visible in the board picker
}
```

See `boards.catalog` channel (`BOARDS_CHANNELS.catalog = 'boards:catalog'`); frontend hook `useBoardsCatalog()` in `apps/desktop/src/renderer/services/use-boards/use-boards.ts`.

#### `boards.importCookies(boardId: string): Promise<CookieImportResult>`

Attempts to import an existing job-board session from the user's installed Chromium browsers (Chrome, Edge, Brave). Reads the browser's encrypted cookie store, decrypts v10/v11 AES-256-GCM cookies (DPAPI on Windows, Keychain/libsecret on Unix), filters for the board's domain, and writes the same artifacts (`cookies.json` + `auth-status.json`) that the in-app login flow produces. Best-effort: never a regression — missing browser, locked profile, or decrypt failure all map to non-error outcomes. v20 App-Bound Encryption (Chrome 127+) is out of scope. See `apps/desktop/src-tauri/src/scraping/board_login/import.rs` (implementation + design doc) and `apps/desktop/src-tauri/src/platform/chrome/mod.rs` (Chromium detection).

```typescript
type CookieImportOutcome = 'Imported' | 'NoSession' | 'Undecryptable' | 'BrowserNotFound';

interface CookieImportResult {
  outcome: CookieImportOutcome; // 'Imported' = success; others are non-error fallbacks
  imported: number; // cookie count (>0 only when outcome='Imported')
  error?: string; // only set if outcome='Error' (i.e. unexpected IO)
}
```

```typescript
interface BoardConfig {
  id: string; // e.g. "linkedin", "greenhouse", "aggregator"
  name: string;
  isEnabled: boolean;
  requiresAuth: boolean;
  hasCredentials: boolean;
  lastScrapedAt?: string;
}
```

**Supported board IDs** (23 active scrapers, canonical list: `BOARD_IDS` in `packages/shared/src/schemas/index.ts`): `aggregator`, `ashby`, `bamboohr`, `breezy`, `berlinstartupjobs`, `comeet`, `germantechjobs`, `greenhouse`, `lever`, `linkedin`, `personio`, `pinpoint`, `recruitee`, `remoteok`, `remotive`, `rippling`, `smartrecruiters`, `arbeitsagentur`, `arbeitnow`, `themuse`, `workable`, `ycombinator`, `wwr`

Note: `indeed`, `stepstone`, `xing`, `workday`, and `glassdoor` were retired as direct scrapers (ADR-026). Their job results are now routed through the `aggregator` board (Adzuna/JSearch/Jooble).

---

## `cliAgents`

CLI agent install management — detect installed coding agents (Claude Code, etc.) on the host and one-click install via npm.

#### `cliAgents.status(): Promise<CliAgentsStatus>`

Returns cached install status for every CLI agent (Claude Code, Gemini CLI, etc.) plus npm availability. Probes for binaries on PATH and version.

#### `cliAgents.redetect(): Promise<CliAgentsStatus>`

Clears the detection cache and re-probes all agents (call after an install).

#### `cliAgents.install(opts: { commandName: string; args: string[]; onOutput?: (line: string) => void; signal?: AbortSignal }): Promise<CliAgentInstallResult>`

One-click install: spawns a shell capability-allowlisted command (fixed npm args). Implemented over the `@tauri-apps/plugin-shell` adapter; the caller cannot tell it isn't a plain IPC command. The capability allowlist is static and matches exactly 3 fixed-arg commands (see `apps/desktop/src-tauri/capabilities/default.json`).

```typescript
interface CliAgentStatus {
  id: string; // provider id ('claude-code' | 'codex' | 'gemini-cli')
  binary: string; // e.g. 'claude'
  installed: boolean;
  version: string | null;
  package: string; // npm package name
  docsUrl: string; // official setup docs
  installCommandName: string; // shell capability command
  installArgs: string[]; // exact args; must match capability allowlist
}

interface CliAgentsStatus {
  agents: CliAgentStatus[];
  npmAvailable: boolean; // gates one-click install
}

interface CliAgentInstallResult {
  code: number | null; // process exit code (null if killed); 0 = success
  success: boolean;
}
```

See: `packages/shared/src/ipc/contracts/cliAgents.ts` (contract), `apps/desktop/src-tauri/src/commands/cli_agents.rs` (read-only Rust commands).

---

## `contactProfile`

Candidate contact information — the single source of truth for the document header contact line. Built from named fields (name, email, phone, location, LinkedIn, GitHub, website, and custom links), localized per language.

### Methods

#### `contactProfile.get(): Promise<ContactProfile>`

Fetches the stored contact profile.

#### `contactProfile.set(profile: ContactProfile): Promise<{ success: true }>`

Persists the contact profile. Rejects (throws) on failure — an unmanaged store or an invalid payload — instead of resolving with an in-band `{ error }` field, so a failed save can never be silently mistaken for success. The profile is the seed for document headers; user edits to generated text are preserved.

#### `contactProfile.headerLine(lang: string): Promise<string>`

Returns the profile's header contact line as markdown, localized for the given ISO-639-1 language code (e.g., `"de"`, `"en-US"`). This is the same line built by the Rust backend, shared across all render paths (résumé, cover letter, DOCX), ensuring no format/language-specific divergence. Returns an empty string when the profile contributes nothing.

```typescript
interface ContactProfile {
  fullName?: string;
  email?: string;
  phone?: string;
  location?: LocalizedText; // { default: string, byLang?: Record<string, string> }
  linkedin?: string; // URL
  github?: string; // URL
  website?: string; // URL
  extraLinks?: ContactLink[]; // [{ label: string, url: string }, …]
  photo?: string; // data: URI only (JPEG/PNG/WebP/GIF, square-cropped)
}
```

See: `packages/shared/src/ipc/contracts/contactProfile.ts` (contract), `apps/desktop/src-tauri/src/commands/contact_profile.rs` + `apps/desktop/src-tauri/src/contact_profile/mod.rs` (implementation).

---

## `credentials`

Encrypted OS keychain storage for board credentials and API keys.

#### `credentials.set(entry: CredentialEntry): Promise<void>`

#### `credentials.remove(key: string): Promise<void>`

#### `credentials.hasCredential(key: string): Promise<boolean>`

#### `credentials.listKeys(): Promise<string[]>`

```typescript
interface CredentialEntry {
  key: string; // e.g. "linkedin", "openai"
  username?: string;
  password?: string;
  apiKey?: string;
}
```

---

## `dialog`

Native OS file dialogs.

#### `dialog.openFile(options: OpenFileOptions): Promise<string | null>`

#### `dialog.saveFile(options: SaveFileOptions): Promise<string | null>`

#### `dialog.openDirectory(): Promise<string | null>`

```typescript
interface OpenFileOptions {
  title?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
  multiple?: boolean;
}
```

---

## `documents`

Document import, storage, and export.

#### `documents.import(filePath: string): Promise<DocumentRecord>`

#### `documents.list(): Promise<DocumentRecord[]>`

#### `documents.getText(id: string): Promise<string>`

Fetches the stored extracted text for a document by id. Returns an empty string if the document is missing or has no text (never rejects).

#### `documents.get(id: string): Promise<DocumentRecord>`

#### `documents.delete(id: string): Promise<void>`

#### `documents.setDefault(id: string): Promise<void>`

#### `documents.exportDocument(req: ExportRequest): Promise<Uint8Array>`

#### `documents.exportAndSave(req: ExportRequest): Promise<string>` — saves to disk, returns path

```typescript
interface DocumentRecord {
  id: string;
  name: string;
  format: 'pdf' | 'docx' | 'txt' | 'md' | 'image';
  language: string;
  pageCount?: number;
  isDefault: boolean;
  createdAt: string;
}

interface ExportRequest {
  content: string; // markdown or plain text
  format: 'docx' | 'pdf';
  template: TemplateId;
  language: string;
  metadata?: ExportMetadata;
}

// Canonical union: packages/shared/src/ipc/contracts/documents.ts (mirrors the
// Rust TemplateId enum). Removed ids (e.g. 'modern') deserialize to 'classic'.
type TemplateId =
  | 'classic'
  | 'swiss-minimal'
  | 'academic'
  | 'atelier'
  | 'meridian'
  | 'throughline'
  | 'portrait'
  | 'lebenslauf'
  | 'cadence'
  | 'regent'
  | 'aria'
  | 'saffron';
```

---

## `geocode`

Location lookup and reverse geocoding.

#### `geocode.suggest(query: string): Promise<GeocodeSuggestion[]>`

Autocomplete suggestions filtered to city-level and country-level results only (via `to_city_country` filter in Rust backend). Returned display strings are formatted as `"City, Country"` for cities or bare country name for country-level matches.

```typescript
interface GeocodeSuggestion {
  display: string;
  lat?: number | null;
  lon?: number | null;
  countryCode?: string | null;
}
```

#### `geocode.lookup(query: string): Promise<GeocodeResult[]>`

```typescript
interface GeocodeResult {
  displayName: string;
  city?: string;
  country?: string;
  lat: number;
  lon: number;
}
```

---

## `github`

GitHub repository import for the resume builder's Projects step.

### `github.importRepos(input: string): Promise<GitHubRepo[]>`

Fetches the user's public GitHub repositories by username or URL. Input is validated for SSRF (username `^[A-Za-z0-9-]{1,39}$` or a github.com URL with valid host); the backend constructs the API URL server-side. Returns a deduplicated, sorted list (forks excluded, sorted by stars descending). On error, throws `AppError` with one of: `Validation("GitHub user not found")` (404), `RateLimited("GitHub rate limit reached, try again later")` (403/429), or `Network("Failed to reach GitHub")` (other non-2xx).

```typescript
interface GitHubRepo {
  name: string; // repository name (deduplicated slug)
  description?: string; // repository description (up to 400 chars in the builder)
  htmlUrl: string; // public GitHub repository URL
  stars: number; // star count (used for sorting)
  language?: string; // primary language (e.g. "TypeScript")
  pushedAt?: string; // last push timestamp (ISO 8601)
}
```

**Contract & IPC:** `packages/shared/src/ipc/contracts/github.ts` (`GitHubContract.importRepos`). **Client:** `apps/desktop/src/tauri-client/namespaces/github/github.ts` (unwraps `{ repos }` response, throws on `{ error }`). **Service hook:** `useGitHubImport()` mutation in `apps/desktop/src/renderer/services/use-github-import/use-github-import.ts`.

**SSRF hardening:** Input is validated as either a bare username (regex) or a github.com URL (hostname extracted via `github_url_first_segment`, non-GitHub hosts rejected). API URL is constructed server-side in Rust (`api_url()` helper), never forwarded from the client.

---

## `jobPreferences`

Saved job search preferences and filters.

#### `jobPreferences.get(): Promise<JobPreferences>`

#### `jobPreferences.save(prefs: JobPreferences): Promise<void>`

```typescript
interface JobPreferences {
  query?: string;
  location?: string;
  remote?: 'yes' | 'no' | 'hybrid' | 'any';
  salaryMin?: number;
  salaryMax?: number;
  boards?: string[];
  dateFilter?: 'day' | 'week' | 'month' | 'any';
  language?: string;
}
```

---

## `jobs`

Job tracker — CRUD plus lifecycle events.

#### `jobs.list(filters?: JobFilters): Promise<JobRecord[]>`

#### `jobs.get(id: string): Promise<JobRecord>`

#### `jobs.update(id: string, patch: Partial<JobRecord>): Promise<void>`

#### `jobs.delete(id: string): Promise<void>`

#### `jobs.cancel(id: string): Promise<void>`

#### `jobs.retry(id: string): Promise<void>`

#### `jobs.markViewed(id: string): Promise<void>`

#### `jobs.markApplied(id: string): Promise<void>`

#### `jobs.bookmark(id: string): Promise<void>`

```typescript
interface JobRecord {
  id: string;
  boardId: string;
  title: string;
  company: string;
  location?: string;
  url: string;
  description?: string;
  salary?: string;
  remote?: 'yes' | 'no' | 'hybrid';
  status: JobStatus;
  appliedAt?: string;
  scrapedAt: string;
}

type JobStatus =
  'new' | 'viewed' | 'applied' | 'interviewing' | 'offer' | 'rejected' | 'bookmarked';

interface JobFilters {
  status?: JobStatus[];
  boardId?: string;
  query?: string;
  remote?: string;
  dateFrom?: string;
}
```

### Events

#### `jobs.onEvent(handler): Unsubscribe`

```typescript
interface JobEvent {
  type: 'queued' | 'started' | 'progress' | 'completed' | 'failed' | 'cancelled';
  jobId: string;
  progress?: number; // 0–100
  error?: string;
}
```

---

## `linkedin`

LinkedIn session management for scraping.

#### `linkedin.setSession(cookie: string): Promise<void>`

#### `linkedin.clearSession(): Promise<void>`

#### `linkedin.checkSession(): Promise<{ valid: boolean; expiresAt?: string }>`

---

## `match`

Resume-job semantic matching and ATS scoring. Default path is **keyword-only** (no embedding); semantic scoring is opt-in.

#### `match.resume(resumeId: string, jobId: string): Promise<MatchScore>`

On-demand per-job scoring — the single scoring path. `MatchScoresProvider` (`apps/desktop/src/renderer/features/jobs/providers/MatchScoresProvider.tsx`) requests a score per job as rows render and distributes results via `useJobMatchScore(jobId)`. The former one-pass `match.batch` / `match_resume_batch` command was **removed** (zero consumers).

```typescript
interface MatchScore {
  jobId: string;
  resumeId: string;
  overallScore: number; // 0–100
  semanticScore: number; // vector similarity
  keywordScore: number; // ATS keyword match (cached resume keywords + live stemming per JD language)
  matchedSkills: string[];
  missingSkills: string[];
  languageMismatch: boolean;
  experienceLevelMatch: boolean;
}
```

---

## `privacy`

Privacy and data control settings.

#### `privacy.getSettings(): Promise<PrivacySettings>`

#### `privacy.updateSettings(settings: Partial<PrivacySettings>): Promise<void>`

#### `privacy.exportData(): Promise<string>` — returns path to exported ZIP

#### `privacy.deleteAllData(): Promise<void>`

#### `privacy.getCrashReporting(): Promise<CrashReportingSettings>`

#### `privacy.setCrashReporting(settings: CrashReportingSettings): Promise<CrashReportingSettings>`

```typescript
interface CrashReportingSettings {
  /** User's choice. Defaults to true. */
  enabled: boolean;
  /** Whether the setup wizard has shown that choice yet. */
  consentShown: boolean;
}
```

Rust-owned rather than a renderer preference: the Sentry client is constructed
before `tauri::Builder` runs, so there is no WebView (and no `localStorage`) at
the moment the decision is needed. Transmission requires **both** flags — see
[ADR 0020](adr/0020-crash-reporting.md). There is no `enableAnalytics` and no
`dataRetentionDays`: no behavioural analytics is collected, and retention is the
processor's 30-day window, not an app setting.

---

## `resume`

Resume management (higher-level than `documents`).

#### `resume.getDefault(): Promise<DocumentRecord | null>`

#### `resume.setDefault(id: string): Promise<void>`

#### `resume.analyze(id: string): Promise<ResumeAnalysis>`

```typescript
interface ResumeAnalysis {
  language: string;
  detectedRole?: string;
  yearsOfExperience?: number;
  skills: string[];
  educationLevel?: string;
  atsScore: number; // 0–100
  strengths: string[];
  weaknesses: string[];
  recommendations: string[];
  languageMismatchRisk: boolean;
}
```

---

## `scrape`

Job board scraping.

#### `scrape.board(req: BoardScrapeRequest): Promise<ScrapeResult>`

#### `scrape.url(url: string): Promise<JobPosting>`

#### `scrape.listPostings(boardId: string): Promise<JobPosting[]>`

#### `scrape.persistJob(posting: JobPosting): Promise<string>` — returns job ID

```typescript
interface BoardScrapeRequest {
  boardId: string;
  query: string;
  location?: string;
  pages?: number; // default 1
  dateFilter?: 'day' | 'week' | 'month' | 'any';
  locale?: string;
}

interface JobPosting {
  boardId: string;
  externalId: string;
  title: string;
  company: string;
  location?: string;
  url: string;
  description?: string;
  salary?: string;
  remote?: 'yes' | 'no' | 'hybrid';
  postedAt?: string;
  scrapedAt: string;
  // Ghost-job trust signal, always populated by the backend (non-blocking —
  // flag-only, never drops a posting). See docs/knowledge/scraping-domain.md
  // § Trust assessment.
  trust?: {
    score: number; // 0–100
    level: 'high' | 'medium' | 'low';
    flags: Array<'missingApplyUrl' | 'invalidUrl' | 'suspiciousDomain' | 'companyDomainMismatch'>;
  };
}

interface ScrapeResult {
  boardId: string;
  postings: JobPosting[];
  totalFound: number;
  errors: string[];
}
```

---

## `search`

Hybrid semantic + keyword search across all collections.

#### `search.hybrid(req: HybridSearchRequest): Promise<HybridSearchResult[]>`

```typescript
interface HybridSearchRequest {
  query: string;
  collection: 'jobs' | 'resumes' | 'skills';
  topK: number; // 1–200
  semanticWeight?: number; // 0–1, default 0.7
  filters?: Record<string, unknown>;
}

interface HybridSearchResult {
  id: string;
  score: number;
  semanticScore: number;
  keywordScore: number;
  payload: Record<string, unknown>;
}
```

---

## `shortcuts`

Keyboard shortcut configuration.

#### `shortcuts.get(): Promise<ShortcutConfig>`

#### `shortcuts.set(config: ShortcutConfig): Promise<void>`

#### `shortcuts.reset(): Promise<void>`

```typescript
interface ShortcutConfig {
  openSearch: string;
  newGeneration: string;
  runScrape: string;
  toggleSidebar: string;
  // ... additional shortcuts
}
```

---

## `support`

Help resources and diagnostics.

#### `support.exportLogs(): Promise<string>` — path to ZIP

#### `support.clearCache(): Promise<void>`

#### `support.getSystemInfo(): Promise<SystemInfo>`

#### `support.submitFeedback(message: string): Promise<void>`

---

## `system`

System health, version, and performance.

#### `system.health(): Promise<HealthReport>`

#### `system.getVersion(): Promise<string>`

#### `system.getMetrics(): Promise<SystemMetrics>`

#### `system.setPerformanceMode(mode: PerformanceMode): Promise<void>`

```typescript
interface HealthReport {
  ollamaConnected: boolean;
  ollamaVersion?: string;
  availableModels: string[];
  dbStatus: 'ok' | 'error';
  vectorDbStatus: 'ok' | 'error';
  diskSpaceGb: number;
  memoryUsageMb: number;
}

interface SystemMetrics {
  cpuPercent: number;
  memoryUsedMb: number;
  memoryTotalMb: number;
  diskUsedGb: number;
  activeWorkers: number;
  queuedJobs: number;
}

type PerformanceMode = 'low' | 'balanced' | 'performance';
```

---

## `menu`

Native app menu and tray menu intent handling.

#### `menu.takePending(): Promise<PendingMenuIntent | null>`

Pulls any buffered menu intents that arrived while the renderer was hidden or backgrounded. Called by the renderer on window focus / visibility restore to catch missed tray-menu or macOS menu-bar events. Returns the queued intent (navigate or action), or `null` if none pending. **Root cause:** Tauri's `app.emit` is fire-and-forget with no per-listener queue; a WebView2 that's hidden, backgrounded, or not-yet-mounted misses the event entirely. Solution: buffer on the Rust side, pull from the renderer once the JS event loop is live.

```typescript
type PendingMenuIntent =
  | { event: 'menu.navigate'; payload: MenuNavigateEvent }
  | { event: 'menu.action'; payload: MenuActionEvent };

interface MenuNavigateEvent {
  route: string;
  section: string | null; // optional anchor within the route
}

interface MenuActionEvent {
  action: 'check-updates' | 'shortcuts';
}
```

#### `menu.onNavigate(handler): Unsubscribe`

Listens for menu navigation events (route clicks from app menu or tray).

#### `menu.onAction(handler): Unsubscribe`

Listens for menu actions (global commands like check-updates).

---

## `updater`

In-app update management.

#### `updater.check(): Promise<UpdateCheckResult>`

Checks for a new release. Returns a typed result: `{ available: true, version }` if an update exists, `{ available: false }` if up-to-date, or `{ error: string }` if the check failed.

```typescript
type UpdateCheckResult =
  { available: true; version: string } | { available: false } | { error: string };
```

#### `updater.download(): Promise<void>`

#### `updater.install(): Promise<void>`

#### `updater.changelog(): Promise<ChangelogResult>`

Fetches recent release history (newest first) for the in-app changelog. Returns releases or an error string (never rejects).

```typescript
interface ChangelogRelease {
  version: string; // without leading 'v'
  name: string | null; // release title from GitHub
  body: string | null; // release notes (Markdown)
  publishedAt: string | null; // ISO 8601 timestamp
  url: string; // GitHub release page
  prerelease: boolean;
}

interface ChangelogResult {
  releases?: ChangelogRelease[];
  error?: string; // set if fetch/parse failed
}
```

### Events

#### `updater.onStatus(handler): Unsubscribe`

Listens for update progress and check results.

```typescript
interface UpdateProgress {
  status: 'downloading' | 'installing' | 'done' | 'error';
  percent?: number;
  error?: string;
}
```

[ollama]: https://ollama.com
