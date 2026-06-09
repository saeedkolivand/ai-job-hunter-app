import { z } from 'zod';

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
  stream: z.boolean().optional(),
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
  'indeed',
  'stepstone',
  // German / DACH
  'arbeitsagentur',
  'berlinstartupjobs',
  'germantechjobs',
  'xing',
  // ATS platforms
  'greenhouse',
  'lever',
  'ashby',
  'smartrecruiters',
  'recruitee',
  'personio',
  'workday',
  // Remote-first / aggregators
  'remoteok',
  'remotive',
  'arbeitnow',
  'wwr',
  'ycombinator',
] as const;
export type BoardId = (typeof BOARD_IDS)[number];

export const DATE_FILTER_OPTIONS = ['30m', '1h', '2h', '4h', '8h', '24h', 'week', 'month'] as const;
export type DateFilterOption = (typeof DATE_FILTER_OPTIONS)[number];

export const ScrapeBoardRequestSchema = z.object({
  board: z.enum(BOARD_IDS),
  query: z.string().min(1),
  location: z.string().optional(),
  pages: z.number().int().min(1).max(20).default(1),
  dateFilter: z.enum(DATE_FILTER_OPTIONS).optional(),
  locale: LocaleSchema.optional(),
  // Structured location (from a picked geocode suggestion) — lets boards filter
  // by precise place/country/radius instead of fuzzy free text (#49/#40).
  countryCode: z.string().optional(),
  latitude: z.number().optional(),
  longitude: z.number().optional(),
  radiusKm: z.number().int().min(0).max(200).optional(),
});

export const ScrapeUrlRequestSchema = z.object({
  url: z.string().url(),
});

export const HybridSearchRequestSchema = z.object({
  query: z.string().min(1),
  collection: z.enum(['jobs', 'resumes', 'skills', 'conversations']),
  topK: z.number().int().min(1).max(200).default(20),
  filters: z.record(z.string(), z.unknown()).optional(),
  semanticWeight: z.number().min(0).max(1).default(0.7),
});

export const MatchResumeRequestSchema = z.object({
  resumeId: z.string().min(1),
  jobId: z.string().min(1),
});

export const JobIdSchema = z.object({ jobId: z.string().min(1) });

export const CredentialSetSchema = z.object({
  boardId: z.enum(['linkedin', 'indeed', 'xing', 'glassdoor']),
  username: z.string().min(1).max(254),
  password: z.string().min(1).max(512),
});

export const CredentialBoardSchema = z.object({
  boardId: z.enum(['linkedin', 'indeed', 'xing', 'glassdoor']),
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
  board: z.string().min(1),
  query: z.string().min(1),
  location: z.string().optional(),
  workType: z.enum(['remote', 'hybrid', 'on-site']).optional(),
  pages: z.number().int().min(1).max(10).default(2),
  dateFilter: z.string().optional(),
});

export const AutopilotFilterSchema = z.object({
  minMatchScore: z.number().min(0).max(100).default(50),
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
  remote: z.string().optional(),
  seniority: z.string().optional(),
  techStack: z.array(TechStackItemSchema).optional(),
});

export type AutopilotCreate = z.infer<typeof AutopilotCreateSchema>;
export type AutopilotUpdate = z.infer<typeof AutopilotUpdateSchema>;
export type JobPreferences = z.infer<typeof JobPreferencesSchema>;

export type AiGenerateRequest = z.infer<typeof AiGenerateRequestSchema>;
export type ModelInspectResult = z.infer<typeof ModelInspectResultSchema>;
export type DocumentImportRequest = z.infer<typeof DocumentImportRequestSchema>;
export type ScrapeBoardRequest = z.infer<typeof ScrapeBoardRequestSchema>;
export type ScrapeUrlRequest = z.infer<typeof ScrapeUrlRequestSchema>;
export type HybridSearchRequest = z.infer<typeof HybridSearchRequestSchema>;
export type MatchResumeRequest = z.infer<typeof MatchResumeRequestSchema>;
