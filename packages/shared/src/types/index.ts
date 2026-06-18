/**
 * Cross-process shared types.
 * These types are SAFE to ship to the renderer (no Node/Electron internals).
 */

import type { z } from 'zod';

import type { AiStreamChunkSchema, JobEventSchema } from '../schemas/index.js';

export type Locale = 'en' | 'de' | 'fr' | 'es' | 'it' | 'tr' | 'pt' | 'ru' | 'zh' | 'ja' | 'ko';

export type JobStatus =
  | 'queued'
  | 'running'
  | 'streaming'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'retrying';

export type JobKind =
  | 'ai.generate'
  | 'ai.embed'
  | 'document.import'
  | 'document.ocr'
  | 'document.chunk'
  | 'document.index'
  | 'scrape.board'
  | 'scrape.url'
  | 'persist.job'
  | 'match.resume'
  | 'autopilot.run';

export interface JobRecord<TPayload = unknown, TResult = unknown> {
  id: string;
  kind: JobKind;
  status: JobStatus;
  progress: number;
  payload: TPayload;
  result?: TResult;
  error?: string;
  retries: number;
  maxRetries: number;
  createdAt: number;
  updatedAt: number;
  startedAt?: number;
  finishedAt?: number;
}

export type JobEvent = z.infer<typeof JobEventSchema>;

export interface AiMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

/**
 * Structured `ai:stream` chunk. `error` is the terminal error frame; `thinking`
 * marks a reasoning/thinking block. Inferred from `AiStreamChunkSchema`.
 */
export type AiStreamChunk = z.infer<typeof AiStreamChunkSchema>;

export interface DocumentRecord {
  id: string;
  title: string;
  source: 'pdf' | 'docx' | 'txt' | 'image' | 'url';
  path?: string;
  language?: string;
  pages?: number;
  importedAt: number;
  isDefault?: boolean;
}

export interface JobPosting {
  id: string;
  externalId?: string;
  title: string;
  company: string;
  location?: string;
  url: string;
  source: string;
  description?: string;
  requirements?: string[];
  postedAt?: number;
  capturedAt: number;
  /** Board-specific metadata (salary, remote status, etc.) */
  [key: string]: unknown;
}

export interface JobInteraction {
  jobId: string;
  title: string;
  company: string;
  url: string;
  source: string;
  location?: string;
  interactionType: 'viewed' | 'opened' | 'applied' | 'bookmarked';
  timestamp: number;
}

export interface MatchScore {
  resumeId: string;
  jobId: string;
  ats: number;
  semantic: number;
  combined: number;
  gaps: string[];
  recommendations: string[];
  explanation?: string;
}

export interface RuntimeHealth {
  ai: { ready: boolean; model?: string; memoryMB?: number };
  /**
   * CLI-agent availability, keyed by provider id (e.g. `'claude-code'`).
   * `detected` = the agent's binary is installed; `version` is its reported
   * version when known. Populated by looping the backend CLI-agent registry.
   */
  cliAgents?: Record<string, { detected: boolean; version?: string | null }>;
  data: { ready: boolean; sqlite: boolean; vector: boolean };
  workers: { active: number; idle: number; max: number };
}

/** Timing for each major phase of bootstrap(). All durations in milliseconds. */
export interface BootMetrics {
  /** Absolute timestamp (Date.now()) when bootstrap() was called. */
  startedAt: number;
  phases: {
    /** Time to create EventBus, JobQueue, RuntimeManager, StateCoordinator. */
    coreInit: number;
    /** Time for createBoardSessions(). */
    boardSessions: number;
    /** Time for runtimes.start('data') — SQLite open. */
    dataRuntime: number;
    /** Time to register all job handlers. */
    jobHandlers: number;
    /** Time to evaluate refreshScheduler(). */
    scheduler: number;
  };
  /** Total bootstrap() duration. */
  totalMs: number;
}

/** Snapshot of process-level resource usage from app.getAppMetrics(). */
export interface ProcessMetric {
  pid: number;
  type: string;
  cpuUsage: { percentCPUUsage: number; idleWakeupsPerSecond: number };
  memory: { workingSetSize: number; peakWorkingSetSize: number };
}

/** Full metrics snapshot exposed via system.getMetrics IPC. */
export interface AppMetrics {
  boot: BootMetrics | null;
  /** Time from app.whenReady() to first BrowserWindow visible (ms). */
  startupMs: number | null;
  jobQueue: { running: number; pending: number; concurrency: number };
  processes: ProcessMetric[];
  /** Timestamp of this snapshot (Date.now()). */
  snapshotAt: number;
}

/**
 * Resolved performance configuration pushed to the Rust shell via
 * `system.setPerformanceMode`. This is the IPC boundary shape: the renderer
 * resolves the active mode (preset or custom profile) into concrete backend
 * numbers and sends them so the shell never needs to know about presets.
 *
 * Tier→number mapping (owned by the renderer's `resolveBackendConfig`):
 *   concurrency:   low→1,  balanced→2,    high→4
 *   keepAliveSecs: low→0,  balanced→300,  high→1800
 *   cacheTtlSecs:  low→86400, balanced→604800, high→null (no expiry)
 *   cacheMaxRows:  low→250,   balanced→2000,   high→null (unbounded)
 */
export interface PerformanceBackendConfig {
  /** The mode string (`low-memory` | `balanced` | `performance` | `custom`) — for logging / e2e. */
  mode: string;
  /** JobQueue concurrency (parallel workers). */
  concurrency: number;
  /** AiRuntime idle model keep-alive, in seconds (0 = unload immediately). */
  keepAliveSecs: number;
  /** Cache entry TTL in seconds; `null` = no expiry (generous). */
  cacheTtlSecs: number | null;
  /** Max cached rows; `null` = unbounded (generous). */
  cacheMaxRows: number | null;
}

/** Identifies which boards support credential-based authentication. */
export const AUTH_CAPABLE_BOARDS = ['linkedin', 'indeed', 'xing', 'glassdoor'] as const;
export type AuthCapableBoard = (typeof AUTH_CAPABLE_BOARDS)[number];

export type AutopilotStatus = 'active' | 'paused' | 'archived';
export type AutopilotSchedule = 'manual' | 'hourly' | 'daily' | 'twice_daily';

/** Outcome of an autopilot's most recent run. `interrupted` is reconciled at
 *  startup from a run left running when the app closed/crashed mid-run. */
export type AutopilotRunStatus = 'inProgress' | 'completed' | 'failed' | 'interrupted';

/** Autopilot job-discovery agent — persisted entity. Finds & ranks matching
 *  jobs on a schedule and notifies you; you apply with the tailoring assistant. */
export interface Autopilot {
  _id: string;
  name: string;
  status: AutopilotStatus;
  target: {
    board: string;
    query: string;
    location?: string;
    workType?: 'remote' | 'hybrid' | 'on-site';
    pages: number;
    dateFilter?: string;
  };
  filter: {
    minMatchScore: number;
    keywords?: string[];
    excludeKeywords?: string[];
  };
  schedule: 'manual' | 'hourly' | 'daily' | 'twice_daily';
  /** Local clock hour (0–23) a recurring schedule fires at. Used by
   *  daily/twice_daily; ignored by hourly. Defaults to 9 (09:00) when absent. */
  scheduleHour?: number;
  /** Local clock minute (0–59) a recurring schedule fires at. Used by
   *  daily/twice_daily and as the "minute past the hour" for hourly. Defaults
   *  to 0 when absent. */
  scheduleMinute?: number;
  resumeText?: string;
  /** Optional base cover letter — the reusable starting point the apply
   *  assistant tailors per found job. */
  coverLetter?: string;
  totalFound: number;
  /** Found jobs the user has applied to (derived from saved generations). */
  totalApplied: number;
  /** Jobs surfaced by the most recent run. */
  foundJobs?: AutopilotFoundJob[];
  /** Outcome of the most recent run — drives the live/failed/interrupted
   *  badge. Absent until the first run. */
  runStatus?: AutopilotRunStatus;
  lastRunAt?: number;
  createdAt: number;
  updatedAt: number;
}

/** A job posting surfaced by an autopilot run. */
export interface AutopilotFoundJob {
  title: string;
  company: string;
  url: string;
  location?: string;
  /** Full job description — used to pre-fill a tailored generation. */
  description?: string;
  /** Match score (0–100) when the posting passed ranking. */
  score?: number;
  foundAt: number;
  /** First surfaced in the most recent run — drives the "New" badge. */
  isNew?: boolean;
  /** The user has generated an application for this job (derived from a saved
   *  generation whose `jobUrl` matches `url`). Drives the "Applied" badge. */
  applied?: boolean;
}

/** Result record for a single autopilot run. */
export interface AutopilotRun {
  autopilotId: string;
  jobId: string;
  startedAt: number;
  finishedAt?: number;
  found: number;
  matched: number;
  applied: number;
  skipped: number;
  errors: string[];
}

// ─── Application tracking (ADR docs/adr/0001-application-aggregate-split.md) ────
//
// An **Application** is the status-bearing aggregate root for a job pursuit (the
// single source of truth for "am I pursuing this, and how far along"). An
// `AiGenerationRecord` is its child Document. NOTE: this is distinct from the
// async-exec `JobStatus` above — do NOT conflate the two.

/**
 * The ordered stage registry — the SINGLE source of truth the Rust
 * `ApplicationStatus` enum mirrors (a Rust parity test pins the id list/order).
 * Each entry: `id`, whether it is `terminal` (closed, would not normally reopen
 * — `ghosted` is intentionally NOT terminal: it is soft/reopenable), and whether
 * it is `preApply` (the user has not applied yet — only `saved`).
 */
export const APPLICATION_STAGES = [
  { id: 'saved', terminal: false, preApply: true },
  { id: 'applied', terminal: false, preApply: false },
  { id: 'screening', terminal: false, preApply: false },
  { id: 'interviewing', terminal: false, preApply: false },
  { id: 'offer', terminal: false, preApply: false },
  { id: 'accepted', terminal: true, preApply: false },
  { id: 'rejected', terminal: true, preApply: false },
  { id: 'ghosted', terminal: false, preApply: false },
  { id: 'withdrawn', terminal: true, preApply: false },
] as const;

/** The application lifecycle status union, derived from {@link APPLICATION_STAGES}. */
export type ApplicationStatus = (typeof APPLICATION_STAGES)[number]['id'];

/** One answered application question carried on the Application aggregate. */
export interface ApplicationAnswer {
  id: string;
  question: string;
  answer: string;
}

/**
 * One AI-suggested question the candidate can ASK the interviewer — distinct from
 * {@link ApplicationAnswer} (which the candidate answers). Persisted on the per-job
 * aiGenerations aggregate.
 */
export interface InterviewQuestion {
  id: string;
  question: string;
  /** Why this question lands well / what it signals to the interviewer. */
  why: string;
  /** Target interviewer — `recruiter` | `hiringManager` | `team` | `leadership` |
   *  `general` (open-typed; an unknown value is treated as `general`). */
  audience: string;
}

/** The Application aggregate root. */
export interface Application {
  id: string;
  status: ApplicationStatus;
  /** First time the status left `saved` (ms). Absent while still `saved`. */
  appliedAt?: number;
  createdAt: number;
  updatedAt: number;
  /** Normalized job URL — the dedup key. Empty for a link-less manual pursuit. */
  jobUrl: string;
  board: string;
  company: string;
  title: string;
  candidate: string;
  answers: ApplicationAnswer[];
  brief: string;
  notes: string;
  /** User-set reminder timestamp (ms) for the next action. Absent = unset. */
  nextActionAt?: number;
  comp: string;
  contactName: string;
  contactEmail: string;
  /** The imported/pasted job description (from the captured DOM at import, or a
   *  later manual paste / retry-resolve). Empty when unknown. */
  jobDescription: string;
}

/** One append-only status-history row. */
export interface StatusEvent {
  applicationId: string;
  /** Empty for the seed event of a freshly-created Application. */
  fromStatus: string;
  toStatus: string;
  at: number;
  note: string;
}

/**
 * A route the renderer navigates to when a notification is actioned. Mirrors the
 * Rust `NotificationRoute` (camelCase serde). `to` is an app route path; `search`
 * is an optional query-param map. Open-typed for zero-change extensibility.
 */
export interface NotificationRoute {
  to: string;
  search?: Record<string, unknown>;
}

/**
 * A persisted notification record. Hand-written to EXACTLY match the Rust
 * `AppNotification` camelCase serialization (`createdAt`, optional `route`).
 * `kind` is an OPEN string (e.g. `autopilot.new_jobs`, `import.result`) so new
 * notification kinds need no codebase change.
 */
export interface AppNotification {
  id: string;
  kind: string;
  title: string;
  body: string;
  /** Epoch millis. */
  createdAt: number;
  read: boolean;
  route?: NotificationRoute;
}

/**
 * The payload of a `notifications:toast` event — an in-app toast for a
 * just-pushed notification while the window was focused. Mirrors the Rust
 * `push_and_notify` emit (camelCase): the new record's `title`, `body`, and
 * optional `route` (so the toast's "View" can navigate). NOT persisted — the
 * full record is already in the inbox via `onChanged`.
 */
export interface NotificationToast {
  title: string;
  body: string;
  route?: NotificationRoute;
}
