import { z } from 'zod';

import { AUTH_CAPABLE_BOARDS } from '../types/index.js';

export const LocaleSchema = z.enum([
  'en',
  'de',
  'fr',
  'es',
  'it',
  'tr',
  'pt',
  'ru',
  'zh',
  'ja',
  'ko',
]);

export const AiMessageSchema = z.object({
  role: z.enum(['system', 'user', 'assistant']),
  content: z.string().min(1),
});

export const AiStreamChunkSchema = z.object({
  jobId: z.string(),
  delta: z.string(),
  done: z.boolean(),
  /** Structured error frame — present instead of delta when the provider fails mid-stream. */
  error: z.object({ code: z.string(), message: z.string() }).optional(),
  /** Present only when the provider emits a reasoning/thinking block. */
  thinking: z.boolean().optional(),
});

export const JobEventSchema = z.object({
  type: z.enum([
    'job.queued',
    'job.started',
    'job.progress',
    'job.stream',
    'job.completed',
    'job.failed',
    'job.cancelled',
  ]),
  jobId: z.string(),
  data: z.unknown().optional(),
  ts: z.number().int(),
});

export const AiGenerateRequestSchema = z.object({
  model: z.string().min(1),
  messages: z.array(AiMessageSchema).min(1),
  locale: LocaleSchema,
  temperature: z.number().min(0).max(2).optional(),
  maxTokens: z.number().int().min(1).max(32768).optional(),
  /**
   * Context window in tokens (Ollama `num_ctx`). Local models only — large
   * résumé/job-ad prompts overflow Ollama's small default context and get
   * silently truncated without this. Ignored by cloud/CLI providers.
   */
  contextWindow: z.number().int().min(512).max(131072).optional(),
  /**
   * AI backend — 'ollama' (default), 'openai', 'openai-compatible', 'anthropic',
   * 'gemini', or a CLI agent ('claude-code', …). Validated server-side.
   */
  provider: z.string().optional(),
  /** Base URL override for openai-compatible providers. */
  baseUrl: z.string().optional(),
  /** Reasoning effort for CLI agents that support it (e.g. Codex: low/medium/high). */
  effort: z.string().optional(),
});

/**
 * Inspection of a local (Ollama) model via `/api/show` — its real maximum
 * context window and size, used to suggest safe generation limits. All fields
 * optional: older Ollama servers omit some of `model_info`/`details`.
 */
export const ModelInspectResultSchema = z.object({
  /** Trained context length in tokens (e.g. 8192, 131072). */
  contextLength: z.number().int().positive().optional(),
  /** Parameter size label from `details` (e.g. "7B", "70.6B"). */
  parameterSize: z.string().optional(),
  /** Quantization level (e.g. "Q4_K_M"). */
  quantization: z.string().optional(),
  /** Model family (e.g. "llama", "qwen2"). */
  family: z.string().optional(),
});

export const DocumentImportRequestSchema = z.object({
  /** Original filename — used to derive title and detect format. */
  name: z.string().min(1).max(512),
  /** Raw file bytes — works in browser (FileReader), Electron, and Tauri alike. */
  bytes: z
    .instanceof(Uint8Array)
    .refine((b) => b.byteLength > 0 && b.byteLength <= 50 * 1024 * 1024, {
      message: 'document must be between 1 byte and 50 MB',
    }),
  title: z.string().optional(),
  locale: LocaleSchema.optional(),
});

export const BOARD_IDS = [
  // Major
  'linkedin',
  // German / DACH
  'arbeitsagentur',
  'berlinstartupjobs',
  'germantechjobs',
  // ATS platforms
  'greenhouse',
  'lever',
  'ashby',
  'smartrecruiters',
  'recruitee',
  'personio',
  // Remote-first / aggregators
  'aggregator',
  'remoteok',
  'remotive',
  'arbeitnow',
  'wwr',
  'ycombinator',
] as const;
export type BoardId = (typeof BOARD_IDS)[number];

/** Stable catalog id for the Adzuna-powered aggregator board. */
export const AGGREGATOR_BOARD_ID = 'aggregator' satisfies BoardId;

export const DATE_FILTER_OPTIONS = ['30m', '1h', '2h', '4h', '8h', '24h', 'week', 'month'] as const;
export type DateFilterOption = (typeof DATE_FILTER_OPTIONS)[number];

export const ScrapeBoardsRequestSchema = z.object({
  boards: z.array(z.enum(BOARD_IDS)).min(1).max(6),
  query: z.string().min(1),
  location: z.string().optional(),
  // Target number of postings to collect per board. The backend paginates each
  // board at its real page size until it has ~amount results (or hits the
  // per-board page budget), then stops.
  amount: z.number().int().min(1).max(100).default(25),
  // When true (a NEW search, not "show more"), the backend replaces the live
  // postings cache the instant the first new result streams in — so a failed or
  // empty search keeps the previous results. Omitted/false = append.
  replace: z.boolean().optional(),
  dateFilter: z.enum(DATE_FILTER_OPTIONS).optional(),
  // Structured location (from a picked geocode suggestion) — lets boards filter
  // by precise place/country/radius instead of fuzzy free text (#49/#40).
  // ISO 3166-1 alpha-2 (the geocode suggestion's countryCode is always 2 letters);
  // validated here so a malformed value can't propagate through IPC/scraping.
  countryCode: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2}$/)
    .optional(),
  latitude: z.number().optional(),
  longitude: z.number().optional(),
  radiusKm: z.number().int().min(0).max(200).optional(),
  // Structured search filters consumed by LinkedIn's `search_paginated` (and
  // ignored by boards without such filters). Free-text codes so new LinkedIn
  // filter values work without a schema change; validated server-side.
  // `jobType`: 'F' (Full-time), 'P' (Part-time), 'C' (Contract), … ;
  // `workType`: '1' (On-site), '2' (Remote), '3' (Hybrid);
  // `sortBy`: 'DD' (Date Descending), 'R' (Relevance).
  jobType: z.string().optional(),
  workType: z.string().optional(),
  experienceLevel: z.string().optional(),
  easyApply: z.boolean().optional(),
  activelyHiring: z.boolean().optional(),
  verified: z.boolean().optional(),
  sortBy: z.string().optional(),
  // Company / board identifiers for ATS boards (greenhouse, lever, ashby,
  // recruitee, personio, smartrecruiters) whose public APIs have no global
  // keyword search — they require a company slug (e.g. Greenhouse
  // `boards-api.greenhouse.io/v1/boards/{company}/jobs`). Absent/empty = no
  // company filter; only ATS boards read it, every other board ignores it.
  companies: z.array(z.string().trim().min(1)).optional(),
});

export const ScrapeUrlRequestSchema = z.object({
  url: z.string().url(),
});

export const MatchResumeRequestSchema = z.object({
  resumeId: z.string().min(1),
  jobId: z.string().min(1),
  semanticScoringEnabled: z.boolean().optional(),
});

export const MatchResumeBatchRequestSchema = z.object({
  resumeId: z.string().min(1),
  jobIds: z.array(z.string().min(1)).max(1000),
  semanticScoringEnabled: z.boolean().optional(),
});

export const JobIdSchema = z.object({ jobId: z.string().min(1) });

export const CredentialSetSchema = z.object({
  boardId: z.enum(AUTH_CAPABLE_BOARDS),
  username: z.string().min(1).max(254),
  password: z.string().min(1).max(512),
});

export const CredentialBoardSchema = z.object({
  boardId: z.enum(AUTH_CAPABLE_BOARDS),
});

export type CredentialSetRequest = z.infer<typeof CredentialSetSchema>;
export type CredentialBoardRequest = z.infer<typeof CredentialBoardSchema>;

export const EmbedRequestSchema = z.object({
  text: z.string().min(1).max(200_000),
  model: z.string().optional(),
});
export type EmbedRequest = z.infer<typeof EmbedRequestSchema>;

export const AiGenerationSaveSchema = z.object({
  candidateName: z.string().default(''),
  jobTitle: z.string().default(''),
  companyName: z.string().default(''),
  resumeLanguage: z.string().default('en'),
  jobAdLanguage: z.string().default('en'),
  targetLanguage: z.string().default('en'),
  mismatch: z.boolean().default(false),
  topRequirements: z.array(z.string()).default([]),
  mode: z.string().default('ats'),
  resumeText: z.string().default(''),
  coverLetterText: z.string().default(''),
  jobAd: z.string().default(''),
  // Application link — the job this generation targets and the board it came
  // from. `jobUrl` is what marks an autopilot found job as "applied".
  jobUrl: z.string().default(''),
  board: z.string().default(''),
  // Application extras — answered questions and the company-research brief used,
  // merged onto the per-job record so it is the full application aggregate.
  applicationAnswers: z
    .array(
      z.object({
        id: z.string().default(''),
        question: z.string().default(''),
        answer: z.string().default(''),
      })
    )
    .default([]),
  companyBrief: z.string().default(''),
  // The AI-suggested "questions to ask the interviewer" — the second assistant,
  // merged onto the per-job record alongside the application answers.
  interviewQuestions: z
    .array(
      z.object({
        id: z.string().default(''),
        question: z.string().default(''),
        why: z.string().default(''),
        audience: z.string().default('general'),
      })
    )
    .default([]),
});
// Note: the `AiGenerationSaveRequest` type is declared in the aiGenerations IPC
// contract (single source for that name); this schema validates the same shape.

// Edit the résumé/cover-letter text of an existing saved generation, selected by
// `id`. Unlike the save merge-upsert this is a direct overwrite, so the user can
// blank out or fully replace text the merge would otherwise have kept. Each text
// field is optional — absent means "leave that field unchanged".
export const AiGenerationUpdateSchema = z.object({
  id: z.string(),
  resumeText: z.string().optional(),
  coverLetterText: z.string().optional(),
});
// Note: the `AiGenerationUpdateRequest` type is declared in the aiGenerations IPC
// contract (single source for that name); this schema validates the same shape.

// Manual referral helper — a locally-stored "referral contact" the user wants to
// ask for a referral at a target company. Create OR update in one call: an absent
// `id` inserts a fresh row, a present `id` overwrites that row. Every person
// detail is entered MANUALLY by the user — there is no LinkedIn scraping or
// profile fetch; `linkedinUrl` is just an optional free-text field.
export const ReferralUpsertSchema = z.object({
  // Absent → insert a new contact; present → overwrite the row with this id.
  id: z.string().optional(),
  // The job this referral targets (links to the autopilot found job; indexed).
  jobUrl: z.string().default(''),
  companyName: z.string().default(''),
  personName: z.string().default(''),
  personRole: z.string().optional(),
  // Manual free text — NOT fetched/scraped.
  linkedinUrl: z.string().optional(),
  emailDraft: z.string().optional(),
  messageDraft: z.string().optional(),
  inviteNoteDraft: z.string().optional(),
  channel: z.enum(['email', 'linkedin_message', 'connection_note']).default('email'),
  status: z.enum(['draft', 'sent', 'replied']).default('draft'),
  notes: z.string().optional(),
});
// Note: the `ReferralUpsertRequest` type is declared in the referrals IPC
// contract (single source for that name); this schema validates the same shape.

// ─── Application tracking schemas (ADR 0001) ───────────────────────────────────

// Manual create / Jobs-page Save. `applications_track` marks it `applied`;
// `applications_save_from_posting` keeps it `saved`. All fields optional — a
// hand-tracked application may have no link yet.
export const ApplicationTrackSchema = z.object({
  // Optional job link. Empty → a link-less pursuit (its own Application).
  jobUrl: z.string().optional(),
  board: z.string().optional(),
  company: z.string().optional(),
  title: z.string().optional(),
  candidate: z.string().optional(),
  // Job description captured at save time (e.g. an aggregator posting whose URL is
  // a redirect that can't be re-resolved). Carried so tailoring has the ad text
  // without a second fetch. Same byte-bound refine as ApplicationUpdateSchema.
  jobDescription: z
    .string()
    .refine((v) => new TextEncoder().encode(v).length <= 200_000, {
      message: 'jobDescription must be at most 200000 bytes',
    })
    .optional(),
});
export type ApplicationTrackRequest = z.infer<typeof ApplicationTrackSchema>;

// Patch the user-editable tracking fields of an existing Application. Each field
// is optional; an absent field is left unchanged. `nextActionAt` is nullable to
// allow explicitly clearing the reminder.
export const ApplicationUpdateSchema = z.object({
  id: z.string().min(1),
  notes: z.string().optional(),
  nextActionAt: z.number().int().nullable().optional(),
  comp: z.string().optional(),
  contactName: z.string().optional(),
  contactEmail: z.string().optional(),
  // The imported/pasted job description, persisted onto the Application so a JD
  // captured from the browser DOM survives to tailoring. Capped to a sane bound
  // so a pathological paste can't bloat the row. Byte-length (not char-count) so
  // it matches the Rust store's 200_000-BYTE limit — multi-byte UTF-8 otherwise
  // passes validation then gets silently truncated.
  // ponytail: 200 KB ceiling matches the 8 MB-frame era; raise if real JDs exceed it.
  jobDescription: z
    .string()
    .refine((v) => new TextEncoder().encode(v).length <= 200_000, {
      message: 'jobDescription must be at most 200000 bytes',
    })
    .optional(),
  jobSummary: z.string().max(50_000).optional(),
});
export type ApplicationUpdateRequest = z.infer<typeof ApplicationUpdateSchema>;

export const ResumeExtractTextSchema = z.object({
  name: z.string().min(1).max(512),
  bytes: z
    .instanceof(Uint8Array)
    .refine((b) => b.byteLength > 0 && b.byteLength <= 25 * 1024 * 1024, {
      message: 'file must be between 1 byte and 25 MB',
    }),
});
export type ResumeExtractTextRequest = z.infer<typeof ResumeExtractTextSchema>;

// ─── Autopilot schemas ────────────────────────────────────────────────────────

export const AutopilotTargetSchema = z.object({
  boards: z.array(z.string().min(1)).min(1).max(6),
  query: z.string().min(1),
  location: z.string().optional(),
  // ISO 3166-1 alpha-2 (sourced from the same geocode suggestion as the manual
  // search); validated here so a malformed value can't propagate to scraping.
  countryCode: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2}$/)
    .optional(),
  workType: z.enum(['remote', 'hybrid', 'on-site']).optional(),
  pages: z.number().int().min(1).max(10).default(2),
  dateFilter: z.string().optional(),
});

export const AutopilotFilterSchema = z.object({
  // Default 0 = keep everything. A non-zero default silently dropped jobs a
  // manual search would have returned (the autopilot zero-jobs bug); the user
  // raises this deliberately. Drives both create + update generated Rust
  // defaults (update reuses this schema via `.partial()`).
  minMatchScore: z.number().min(0).max(100).default(0),
  keywords: z.array(z.string()).optional(),
  excludeKeywords: z.array(z.string()).optional(),
});

export const AutopilotCreateSchema = z.object({
  name: z.string().min(1).max(100),
  target: AutopilotTargetSchema,
  filter: AutopilotFilterSchema,
  schedule: z.enum(['manual', 'hourly', 'daily', 'twice_daily']),
  // Local clock time a recurring schedule fires at. `scheduleHour` drives
  // daily/twice_daily (ignored by hourly); `scheduleMinute` drives both those
  // and the "minute past the hour" for hourly. Defaults applied in Rust when
  // absent (09:00 for daily/twice_daily, minute 0 for hourly).
  scheduleHour: z.number().int().min(0).max(23).optional(),
  scheduleMinute: z.number().int().min(0).max(59).optional(),
  resumeText: z.string().optional(),
  // Optional base cover letter — reused as the starting point when tailoring a
  // found job in the apply assistant. (Auto-apply was removed; this field is a
  // reusable template, not an instruction to submit anything.)
  coverLetter: z.string().optional(),
});

export const AutopilotUpdateSchema = AutopilotCreateSchema.partial().extend({
  status: z.enum(['active', 'paused', 'archived']).optional(),
});

export const AutopilotIdSchema = z.object({ autopilotId: z.string().min(1) });

export const TechStackItemSchema = z.object({
  name: z.string().min(1),
  category: z.string().min(1),
});

export const JobPreferencesSchema = z.object({
  location: z.string().optional(),
  techStack: z.array(TechStackItemSchema).optional(),
});

export type AutopilotCreate = z.infer<typeof AutopilotCreateSchema>;
export type AutopilotUpdate = z.infer<typeof AutopilotUpdateSchema>;
export type JobPreferences = z.infer<typeof JobPreferencesSchema>;

export type AiGenerateRequest = z.infer<typeof AiGenerateRequestSchema>;
export type ModelInspectResult = z.infer<typeof ModelInspectResultSchema>;
export type DocumentImportRequest = z.infer<typeof DocumentImportRequestSchema>;
export type ScrapeBoardsRequest = z.infer<typeof ScrapeBoardsRequestSchema>;
export type ScrapeUrlRequest = z.infer<typeof ScrapeUrlRequestSchema>;
export type MatchResumeRequest = z.infer<typeof MatchResumeRequestSchema>;
export type MatchResumeBatchRequest = z.infer<typeof MatchResumeBatchRequestSchema>;
