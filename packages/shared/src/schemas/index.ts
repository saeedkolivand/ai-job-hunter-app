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
  stream: z.boolean().optional(),
  /** AI backend — 'ollama' (default), 'openai', 'openai-compatible', 'anthropic', 'gemini'. */
  provider: z.string().optional(),
  /** Base URL override for openai-compatible providers. */
  baseUrl: z.string().optional(),
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
});
// Note: the `AiGenerationSaveRequest` type is declared in the aiGenerations IPC
// contract (single source for that name); this schema validates the same shape.

export const ConversationSaveMessageSchema = z.object({
  conversationId: z.string().default('default'),
  role: z.string().default('user'),
  content: z.string().default(''),
});
export type ConversationSaveMessageRequest = z.infer<typeof ConversationSaveMessageSchema>;

export const ResumeExtractTextSchema = z.object({
  name: z.string().min(1).max(512),
  bytes: z
    .instanceof(Uint8Array)
    .refine((b) => b.byteLength > 0 && b.byteLength <= 25 * 1024 * 1024, {
      message: 'file must be between 1 byte and 25 MB',
    }),
});
export type ResumeExtractTextRequest = z.infer<typeof ResumeExtractTextSchema>;

export const APPLIER_IDS = ['linkedin', 'indeed', 'greenhouse', 'workday'] as const;
export type ApplierId = (typeof APPLIER_IDS)[number];

export const ApplyStartSchema = z.object({
  board: z.enum(APPLIER_IDS),
  url: z.string().url(),
  coverLetter: z.string().max(20_000).optional(),
  resumePath: z.string().optional(),
  autoSubmit: z.boolean().optional(),
});
export type ApplyStartRequest = z.infer<typeof ApplyStartSchema>;

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
  action: z.enum(['save', 'review', 'auto_apply']),
  schedule: z.enum(['manual', 'hourly', 'daily', 'twice_daily']),
  resumeText: z.string().optional(),
  coverLetter: z.string().optional(),
  autoSubmit: z.boolean().default(false),
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
  salaryMin: z.number().int().optional(),
  salaryMax: z.number().int().optional(),
  techStack: z.array(TechStackItemSchema).optional(),
});

export type AutopilotCreate = z.infer<typeof AutopilotCreateSchema>;
export type AutopilotUpdate = z.infer<typeof AutopilotUpdateSchema>;
export type JobPreferences = z.infer<typeof JobPreferencesSchema>;

export type AiGenerateRequest = z.infer<typeof AiGenerateRequestSchema>;
export type DocumentImportRequest = z.infer<typeof DocumentImportRequestSchema>;
export type ScrapeBoardRequest = z.infer<typeof ScrapeBoardRequestSchema>;
export type ScrapeUrlRequest = z.infer<typeof ScrapeUrlRequestSchema>;
export type HybridSearchRequest = z.infer<typeof HybridSearchRequestSchema>;
export type MatchResumeRequest = z.infer<typeof MatchResumeRequestSchema>;
