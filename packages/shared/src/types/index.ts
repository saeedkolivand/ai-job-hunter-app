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
  | 'apply.job'
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
}

export interface DocumentRecord {
  id: string;
  title: string;
  source: 'pdf' | 'docx' | 'txt' | 'image' | 'url';
  path?: string;
  language?: string;
  pages?: number;
  importedAt: number;
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
  data: { ready: boolean; sqlite: boolean; vector: boolean };
  workers: { active: number; idle: number; max: number };
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
export type AutopilotAction = 'save' | 'review' | 'auto_apply';
export type AutopilotSchedule = 'manual' | 'hourly' | 'daily' | 'twice_daily';

/** Autopilot job application agent — persisted entity. */
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
  action: 'save' | 'review' | 'auto_apply';
  schedule: 'manual' | 'hourly' | 'daily' | 'twice_daily';
  resumeText?: string;
  coverLetter?: string;
  autoSubmit: boolean;
  totalFound: number;
  totalApplied: number;
  lastRunAt?: number;
  createdAt: number;
  updatedAt: number;
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
