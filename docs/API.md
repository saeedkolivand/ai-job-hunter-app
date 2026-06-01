# IPC API Reference — AI Job Hunter

Last updated: 2026-06-01

All renderer ↔ Rust communication is defined as typed contracts in `packages/shared/src/ipc/contracts/`. The renderer accesses them exclusively through `AppClient` service hooks.

> **Never call `window.__TAURI_INVOKE__` directly.** Use the service hooks in `apps/tauri/src/renderer/services/`.

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
| [aiGenerations](#aigenerations)   | Generated document metadata                |
| [apply](#apply)                   | Auto-apply to job postings                 |
| [autopilot](#autopilot)           | Workflow automation engine                 |
| [boards](#boards)                 | Job board management                       |
| [conversations](#conversations)   | Chat history                               |
| [credentials](#credentials)       | Encrypted credential storage               |
| [dialog](#dialog)                 | Native file dialogs                        |
| [documents](#documents)           | Document import/export                     |
| [geocode](#geocode)               | Location lookup                            |
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
  temperature?: number; // 0–1, default 0.3
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

Accepts the full **job ad text** (not a company name). The backend extracts the company internally via the `CompanyResearch` enricher (`cover_letter/research/`), runs Brave search + provider synthesis, and caches the result in `KvCache`. Returns `{ company, brief }`. Degrades gracefully — returns `{ company: "", brief: "" }` when there is no Brave key or the search/synthesis fails, so generation always proceeds. The brief is folded into cover-letter and application-answer prompts as an untrusted-fenced block (see ADR-010). See `commands/ai.rs: ai_research_company`.

#### `ai.embed(text: string): Promise<number[]>`

Generates a vector embedding for the given text using the configured embedding model.

#### `ai.setProviderKey(provider: AIProvider, key: string): Promise<void>`

Stores an API key in the OS keychain for the given provider.

#### `ai.getProviders(): Promise<ProviderConfig[]>`

Returns all configured providers and their availability status.

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

## `apply`

Automated job application.

#### `apply.job(req: ApplyRequest): Promise<ApplyResult>`

Applies to a job posting automatically using Playwright.

```typescript
interface ApplyRequest {
  jobId: string;
  resumeId: string;
  coverLetterId?: string;
  customMessage?: string;
  credentials?: string; // credential key from OS keychain
}

interface ApplyResult {
  success: boolean;
  applicationId?: string;
  error?: string;
  screenshotPath?: string;
}
```

---

## `autopilot`

Workflow automation engine — defines and executes multi-step job application workflows.

#### `autopilot.create(workflow: WorkflowDefinition): Promise<string>` — returns workflow ID

#### `autopilot.update(id: string, patch: Partial<WorkflowDefinition>): Promise<void>`

#### `autopilot.delete(id: string): Promise<void>`

#### `autopilot.list(): Promise<WorkflowRecord[]>`

#### `autopilot.get(id: string): Promise<WorkflowRecord>`

#### `autopilot.run(id: string): Promise<void>` — triggers immediate run

#### `autopilot.pause(id: string): Promise<void>`

#### `autopilot.resume(id: string): Promise<void>`

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
  step: 'scrape' | 'match' | 'generate' | 'apply' | 'complete' | 'error';
  detail: string;
  timestamp: string;
  jobId?: string;
  error?: string;
}
```

---

## `boards`

Job board configuration management.

#### `boards.list(): Promise<BoardConfig[]>`

#### `boards.enable(boardId: string): Promise<void>`

#### `boards.disable(boardId: string): Promise<void>`

#### `boards.getStats(boardId: string): Promise<BoardStats>`

```typescript
interface BoardConfig {
  id: string; // e.g. "linkedin", "indeed", "greenhouse"
  name: string;
  isEnabled: boolean;
  requiresAuth: boolean;
  hasCredentials: boolean;
  lastScrapedAt?: string;
}
```

**Supported board IDs**: `linkedin`, `indeed`, `stepstone`, `xing`, `greenhouse`, `lever`, `ashby`, `smartrecruiters`, `recruitee`, `personio`, `workday`, `remoteok`, `remotive`, `arbeitsagentur`, `berlinstartupjobs`, `germantechjobs`, `arbeitnow`, `ycombinator`

---

## `conversations`

Chat history persistence.

#### `conversations.list(limit?: number): Promise<ConversationRecord[]>`

#### `conversations.insert(record: ConversationInsert): Promise<string>`

#### `conversations.clear(): Promise<void>`

```typescript
interface ConversationRecord {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  model?: string;
  createdAt: string;
}
```

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

type TemplateId =
  | 'classic'
  | 'modern'
  | 'executive'
  | 'swiss-minimal'
  | 'two-column'
  | 'editorial-serif'
  | 'mono-technical'
  | 'refined-executive'
  | 'academic';
```

---

## `geocode`

Location lookup and reverse geocoding.

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
  | 'new'
  | 'viewed'
  | 'applied'
  | 'interviewing'
  | 'offer'
  | 'rejected'
  | 'bookmarked';

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

Resume-job semantic matching and ATS scoring.

#### `match.resume(resumeId: string, jobId: string): Promise<MatchScore>`

#### `match.batch(resumeId: string, jobIds: string[]): Promise<MatchScore[]>`

```typescript
interface MatchScore {
  jobId: string;
  resumeId: string;
  overallScore: number; // 0–100
  semanticScore: number; // vector similarity
  keywordScore: number; // ATS keyword match
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

```typescript
interface PrivacySettings {
  enableCrashReporting: boolean;
  enableAnalytics: boolean;
  dataRetentionDays: number;
}
```

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
  collection: 'jobs' | 'resumes' | 'skills' | 'conversations';
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

## `updater`

In-app update management.

#### `updater.check(): Promise<UpdateInfo | null>`

#### `updater.downloadAndInstall(): Promise<void>`

```typescript
interface UpdateInfo {
  version: string;
  releaseDate: string;
  releaseNotes: string;
  downloadUrl: string;
}
```

### Events

#### `updater.onProgress(handler): Unsubscribe`

```typescript
interface UpdateProgress {
  status: 'downloading' | 'installing' | 'done' | 'error';
  percent?: number;
  error?: string;
}
```

[ollama]: https://ollama.com
