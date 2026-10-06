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

/**
 * The declared-intent vocabulary for `AiGenerateRequestSchema.intent` — a
 * named constant (mirroring `DATE_FILTER_OPTIONS` below) rather than an
 * inline array literal so the IPC codegen (`pnpm gen:ipc`, see
 * `packages/shared/scripts/gen-ipc-rust.ts`) can emit the SAME literal list
 * as a Rust `&[&str]` const for `resolve_intent`'s own tests
 * (`commands/ai_provider/tests/chat_and_intent.rs`) to iterate — one source of truth for the
 * wire vocabulary instead of a hand-typed copy on each side that could
 * silently drift (a renamed/typo'd literal here would otherwise degrade
 * every affected request to `Intent::Default` with no test catching it).
 */
export const AI_GENERATE_INTENTS = ['deterministic', 'prose', 'prose_grounded', 'default'] as const;
export type AiGenerateIntent = (typeof AI_GENERATE_INTENTS)[number];

export const AiGenerateRequestSchema = z.object({
  model: z.string().min(1),
  messages: z.array(AiMessageSchema).min(1),
  locale: LocaleSchema,
  temperature: z.number().min(0).max(2).optional(),
  /**
   * Nucleus-sampling threshold. Detector-resistance knob (RAID, ACL 2024):
   * random sampling + repetition penalties measurably drop AI-detector
   * accuracy — applied only to prose generation surfaces (cover letter,
   * application answers, email, referral, interview), never resume/analysis.
   */
  topP: z.number().min(0).max(1).optional(),
  /** OpenAI/OpenAI-compatible + Gemini frequency penalty. */
  frequencyPenalty: z.number().min(-2).max(2).optional(),
  /** OpenAI/OpenAI-compatible + Gemini presence penalty. */
  presencePenalty: z.number().min(-2).max(2).optional(),
  /** Ollama `repeat_penalty` (distinct semantics from frequency/presence penalty — never remap). */
  repeatPenalty: z.number().min(1).max(2).optional(),
  maxTokens: z.number().int().min(1).max(32768).optional(),
  /**
   * Context window in tokens (Ollama `num_ctx`). Local models only — large
   * résumé/job-ad prompts overflow Ollama's small default context and get
   * silently truncated without this. Ignored by cloud/CLI providers.
   */
  contextWindow: z.number().int().min(512).max(131072).optional(),
  // NOTE: `provider` + `baseUrl` were REMOVED from this request (task #16). The
  // active generation provider/model/base_url is now backend-owned
  // (`AiConfigStore`, read via `ai_active_config`); the renderer can no longer
  // point generation at an arbitrary endpoint (key-exfiltration SSRF). The Rust
  // side resolves routing from the store and overwrites `model` before streaming
  // — `model` stays on the wire only because every `chat_stream` impl reads it.
  /**
   * Reasoning effort for any provider/model that supports it (backend-gated
   * per `ModelCapabilities.supports_reasoning` — see `ai_model_capabilities`'s
   * `effortLevels`). Only reaches `chat_stream` (streaming generation); the
   * agent tool-calling loop and `research*` calls keep the provider default.
   */
  effort: z.string().optional(),
  /**
   * The renderer's declared INTENT for this generation step — never a raw
   * sampling number. `'deterministic'` (analysis/résumé/job-ad-summary/
   * GitHub-projects): exact, non-creative output. `'prose'` (interview
   * questions, likely questions, STAR feedback): creative, detector-resistant
   * writing with no traceability requirement. `'prose_grounded'` (cover
   * letter, application answers, referral messages, application email): same
   * detector-resistant register as `'prose'`, but the output makes factual
   * claims about the candidate that MUST stay traceable to the résumé/job ad
   * — concretely, `'prose'` minus the presence-penalty knob (it pushes a
   * model toward new topics, i.e. invented candidate facts). Absent/
   * `'default'`: resolves to the SAME numbers as `'deterministic'` on an
   * accepting provider (see `Intent`'s own doc comment,
   * `commands/ai_provider/sampling.rs`) — never a genuinely separate "no opinion"
   * state, since omitting on an accepting provider is not a safe default
   * either. Each provider adapter maps `(model, intent)` to its OWN sampling
   * numbers via `AiProvider::sampling_profile`
   * (`commands/ai_provider/mod.rs`) — real values wherever the provider
   * accepts them (this fix's whole point is never sending them where they
   * 400 or are documented-harmful, NOT changing register everywhere else);
   * the explicit numeric fields above (`temperature` etc.) still win over
   * whatever the adapter would otherwise pick. Only reaches `chat_stream`,
   * exactly like `effort`.
   */
  intent: z.enum(AI_GENERATE_INTENTS).optional(),
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
  // The apply-by-email draft — merged onto the per-job record like the cover
  // letter, so switching tabs (or restarting) no longer loses it. Two plain
  // strings: the UI edits and copies subject and body independently.
  emailSubject: z.string().default(''),
  emailBody: z.string().default(''),
  // Deterministic content-quality report (serialized `ContentReport` JSON) for
  // THIS save's resume/cover text — see ADR-007 addendum. Optional: only a save
  // that regenerates resume_text carries a fresh one; every other save (answers,
  // brief, email draft) omits it and the merge keeps whatever report is already
  // on the aggregate.
  qualityReport: z.string().optional(),
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

export type AiGenerateRequest = z.infer<typeof AiGenerateRequestSchema>;
export type ModelInspectResult = z.infer<typeof ModelInspectResultSchema>;
