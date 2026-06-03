/**
 * Cross-process shared types.
 * These types are SAFE to ship to the renderer (no Node/Electron internals).
 */

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

export interface JobEvent {
  type:
    | 'job.queued'
    | 'job.started'
    | 'job.progress'
    | 'job.stream'
    | 'job.completed'
    | 'job.failed'
    | 'job.cancelled';
  jobId: string;
  data?: unknown;
  ts: number;
}

export interface AiMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export interface AiStreamChunk {
  jobId: string;
  delta: string;
  done: boolean;
  /** Structured error frame — present instead of delta when the provider fails mid-stream. */
  error?: { code: string; message: string };
  /** Present only when the provider emits a reasoning/thinking block (e.g. Anthropic extended thinking). */
  thinking?: boolean;
}

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

export interface SearchHit<T = unknown> {
  id: string;
  score: number;
  payload: T;
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

/** Renderer-safe credential metadata. Never includes the password. */
export interface CredentialMetadata {
  boardId: string;
  username: string;
  savedAt: number;
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
