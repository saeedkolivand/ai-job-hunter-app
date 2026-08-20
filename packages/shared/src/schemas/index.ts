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

/**
 * The declared-intent vocabulary for `AiGenerateRequestSchema.intent` — a
 * named constant (mirroring `DATE_FILTER_OPTIONS` below) rather than an
 * inline array literal so the IPC codegen (`pnpm gen:ipc`, see
 * `packages/shared/scripts/gen-ipc-rust.ts`) can emit the SAME literal list
 * as a Rust `&[&str]` const for `resolve_intent`'s own tests
 * (`commands/ai_provider/mod.rs`) to iterate — one source of truth for the
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
   * `commands/ai_provider/mod.rs`) — never a genuinely separate "no opinion"
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

/**
 * Every board id the app knows, in catalog display order.
 *
 * This list MIRRORS the Rust registry (`scraping/boards/mod.rs::SCRAPERS`) —
 * which is the real catalog the UI renders from — so the two can drift, and did:
 * `jobicy` shipped as a registered, listed board with en/de labels and was
 * missing here entirely. `pnpm gen:ipc` now emits this list to
 * `ipc_contracts/board_ids.rs` and a Rust test compares it against `SCRAPERS`
 * in BOTH directions, so a board added on either side fails the build until it
 * is added on the other.
 */
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
  'pinpoint',
  'rippling',
  'breezy',
  'bamboohr',
  'workable',
  'comeet',
  // Remote-first / aggregators
  'aggregator',
  'remoteok',
  'remotive',
  'arbeitnow',
  'jobicy',
  'themuse',
  'wwr',
  'ycombinator',
] as const;
export type BoardId = (typeof BOARD_IDS)[number];

/** Stable catalog id for the Adzuna-powered aggregator board. */
export const AGGREGATOR_BOARD_ID = 'aggregator' satisfies BoardId;

export const DATE_FILTER_OPTIONS = [
  '15m',
  '30m',
  '1h',
  '2h',
  '4h',
  '8h',
  '24h',
  'week',
  'month',
] as const;
export type DateFilterOption = (typeof DATE_FILTER_OPTIONS)[number];

export const ScrapeBoardsRequestSchema = z.object({
  // Bounded by the catalog size (not a fixed number) so selecting every listed
  // board always validates and adding a board needs no schema edit.
  //
  // The `z.enum` constrains the TypeScript type and this schema's own `parse`.
  // It does NOT reach the wire: `pnpm gen:ipc` emits `boards` as `Vec<String>`,
  // because the codegen lowers enums to strings — so the id an IPC call
  // actually sends is checked by the Rust registry lookup (`boards::get`), not
  // here. The real dedup+truncate defense against a request-amplification
  // payload likewise lives server-side in the Rust engine (registry-size cap
  // over the deduped set).
  boards: z.array(z.enum(BOARD_IDS)).min(1).max(BOARD_IDS.length),
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
  // recruitee, personio, smartrecruiters, pinpoint, rippling, breezy,
  // bamboohr) whose public APIs have no global keyword search — they require
  // a company slug (e.g. Greenhouse
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

/**
 * Request for `match:text`: score a stored résumé against arbitrary job-ad
 * TEXT instead of a `PostingsCache` id. The Score tab in `JobAdView` only ever
 * has `jobDesc: string` in hand — `TailorFlow` receives an `Application` /
 * `AutopilotFoundJob`, neither of which carries a posting-cache id, and that
 * cache is RAM-only and deliberately transient (discovery is transient by
 * design), so a saved application could never have an entry anyway. Same
 * 200_000-byte cap as `ResumeTrimSuggestionsRequestSchema.jobText` below (this
 * reads the same kind of text and mirrors the server's
 * `MAX_JOB_DESCRIPTION_BYTES`); the Rust command clamps too, since this is an
 * IPC boundary a non-UI caller can reach directly.
 */
export const MatchTextRequestSchema = z.object({
  resumeId: z.string().min(1),
  jobText: z.string().min(1).max(200_000),
});

/**
 * Request for the advisory trim panel: rank a résumé's bullets by how much
 * keyword weight each one carries for THIS posting, weakest first.
 *
 * Takes the résumé and job ad as **text**, not ids — the AI-Generate flow scores
 * the currently-previewed (possibly unsaved, possibly hand-edited) document
 * against a pasted job ad, neither of which need exist in a store. Scoring is
 * embedding-free (see `documents/keywords.rs`), so this is cheap enough to call
 * on every committed edit.
 */
export const ResumeTrimSuggestionsRequestSchema = z.object({
  resumeText: z.string().min(1).max(200_000),
  jobText: z.string().min(1).max(200_000),
  /** Export market — resolves `maxPages`. Defaults to the intl profile. */
  locale: z.string().max(32).optional(),
});

/**
 * Request for `resume:validateContent` — deterministic content-quality checks
 * (factual accuracy, ATS structure, AI-voice tells) on an already-generated
 * résumé/letter against its source résumé and the job ad. See
 * `validate::content::{ContentInput, validate_content}` (Rust, L1 — no AI call,
 * safe to run on every save). Same size caps as
 * `ResumeTrimSuggestionsRequestSchema` — this reads the same kind of text.
 */
export const ResumeValidateContentSchema = z.object({
  generated: z.string().min(1).max(200_000),
  source: z.string().min(1).max(200_000),
  jobAd: z.string().max(200_000),
  topRequirements: z.array(z.string().max(300)).max(50),
  targetLanguage: z.string().max(32),
  docKind: z.enum(['resume', 'coverLetter']),
});
export type ResumeValidateContentRequest = z.infer<typeof ResumeValidateContentSchema>;

/**
 * The historic vocabulary of `pipeline_runs.depth`/`QualityReport.pipeline`
 * values — `fast` (the renderer's own deterministic pass), `quality` (the
 * staged Rust pipeline — the fixed value every new run persists), and `max`
 * (a second staged depth removed for wasting tokens on no acted-on value).
 *
 * **A closed vocabulary for READING, not for a request field.** `resumePipeline
 * .run`'s wire request has no `depth` field any more — the pipeline it runs is
 * fixed — but a run row or a persisted `QualityReport` written before that
 * change (or by the renderer's own fast pass) still carries one of these three
 * values, and both `PipelineRunSummary.depth` and `QualityReport.pipeline`
 * type against this set so a historic value round-trips instead of being
 * silently relabelled.
 */
export const GENERATION_DEPTHS = ['fast', 'quality', 'max'] as const;
export type GenerationDepth = (typeof GENERATION_DEPTHS)[number];

/**
 * Request for `resumePipeline.run` — one staged, budgeted résumé generation.
 *
 * **Identity + inputs only.** Routing (provider/model/baseUrl) is backend-owned
 * (`Completer::from_active`), and so is the BUDGET: `maxSteps`/`maxTokens`/
 * `runTimeout` are compile-time `Budget::RESUME_QUALITY` constants, never
 * renderer-supplied, because they bound spend on a paid API (see
 * `pipeline::budget`'s module doc and its lock test).
 *
 * **Two ways in, per side, ID WINS.** `resumeId`/`jobId` resolve SERVER-side —
 * `resumeId` through the `DocumentStore`, `jobId` through the live postings
 * cache — so the model never sees a renderer-supplied document body on that
 * path. `resumeText`/`jobAdText` exist for the apply flow's other two entry
 * points (a pasted job ad, an Autopilot found job), which have no cache id or
 * stored résumé id to hand over. The Rust `execute` resolution rule: a
 * nonempty id ALWAYS wins over the matching text field (never a silent
 * fallback — a missing id is a hard error, not a text retry), and at least one
 * of each pair is required (`resumeId` or `resumeText`; `jobId` or
 * `jobAdText`). `jobTitle`/`companyName`/`board` are the posting identity the
 * cache lookup would otherwise have supplied — read only on the text path.
 * Every one of these still reaches a prompt ONLY through the existing fenced
 * paths (ADR-010); this schema does not change that boundary.
 *
 * `effort` is the ordinary cross-provider reasoning-effort token, exactly as
 * `AiGenerateRequest.effort`: it scales sampling AND the run deadline
 * (`qualityRunDeadlineSecs`), bounded by the same ≤3× multiplier table every
 * other deadline uses, and an unrecognized value falls back to 1.0.
 */
export const ResumePipelineRunSchema = z
  .object({
    resumeId: z.string().default(''),
    jobId: z.string().default(''),
    /** Pasted/found-job résumé text — the id-less path. Ignored (never a
     *  fallback) when `resumeId` is set. Same cap class as
     *  `ResumeTrimSuggestionsRequestSchema.resumeText` — the same kind of text. */
    resumeText: z.string().max(200_000).default(''),
    /** Pasted/found-job posting text — the id-less path. Ignored (never a
     *  fallback) when `jobId` is set. Same cap class as `coverLetterText`
     *  below (both mirror the server's `MAX_JOB_DESCRIPTION_BYTES`). */
    jobAdText: z.string().max(200_000).default(''),
    /** Posting identity for the text path only — the cache lookup's `title`. */
    jobTitle: z.string().max(512).default(''),
    /** Posting identity for the text path only — the cache lookup's `company`. */
    companyName: z.string().max(512).default(''),
    /** Posting identity for the text path only — the cache lookup's `source`
     *  board. Short slug (`"linkedin"`, `"indeed"`, an aggregator name). */
    board: z.string().max(64).default(''),
    /** The posting URL this run belongs to — the run store's retention key and
     *  the `ai_generations` aggregate key. Empty for an unlinked generation. */
    jobUrl: z.string().max(2_048).default(''),
    targetLanguage: z.string().max(32).default('en'),
    /** Resolved job-market id (see `resolveMarket`) — drives the letter's
     *  etiquette (`crate::locale::letter::conventions`, the SAME fixture the
     *  export path reads). Defaults to the international baseline so an
     *  existing caller that never sets this gets byte-identical behavior. */
    market: z.string().max(32).default('intl'),
    /** Today's date, pre-formatted by the renderer per the target locale —
     *  handed to the letter prompt so the model places it instead of
     *  inventing one. Empty = no date (the current behavior for every
     *  existing caller). */
    today: z.string().max(64).default(''),
    effort: z.string().max(32).optional(),
    /** The posting's top requirements, as the JD-analysis step extracted them —
     *  the same list `resume:validateContent` takes. */
    topRequirements: z.array(z.string().max(300)).max(50).default([]),
    /** An already-generated cover letter to validate alongside the résumé.
     *  Empty = no letter in scope (no letter checks run). Legacy/validate-only:
     *  when {@link includeCoverLetter} is true the `cover_letter` STAGE writes
     *  its own letter and this text is the fallback for callers that skip it. */
    coverLetterText: z.string().max(200_000).default(''),
    /** Whether the run's `cover_letter` stage should generate a letter (one
     *  extra streamed pass) instead of no-opping. Default false: an existing
     *  caller that never sets this gets byte-identical behavior — the stage
     *  finishes instantly at zero cost, exactly as if it did not exist. */
    includeCoverLetter: z.boolean().default(false),
    /** Opt-in: research the posting's company before writing the letter and
     *  fence a `<company_research>` block into its prompt when a brief comes
     *  back non-empty. Ignored when {@link includeCoverLetter} is false.
     *  Admitted through the SAME shared `"ai_research"` rate/concurrency/
     *  daily-budget bucket `ai_research_company` uses — a second, billable
     *  provider web search, never an unbounded one. Default false: an
     *  existing caller gets byte-identical behavior (no extra call, no new
     *  block). */
    researchCompany: z.boolean().default(false),
  })
  .refine((data) => data.resumeId.trim() !== '' || data.resumeText.trim() !== '', {
    message: 'either resumeId or resumeText is required',
    path: ['resumeId'],
  })
  .refine((data) => data.jobId.trim() !== '' || data.jobAdText.trim() !== '', {
    message: 'either jobId or jobAdText is required',
    path: ['jobId'],
  });
/**
 * The INPUT shape, not `z.infer`'s output — deliberately. Every field here has
 * a `.default(...)`, so the OUTPUT type (every other `*Request` type in this
 * file uses it) makes them all REQUIRED, and an existing caller built before
 * this PR's five new fields (`TailoredResumePanel`, `useResumePipelineSession`)
 * would fail to type-check despite the wire being unaffected — nothing on
 * this transport ever calls `.parse()` (see `ClampedRequest`'s Rust doc:
 * "Zod does not run on this transport"), so there is no runtime difference to
 * protect, only a compile-time one to avoid.
 *
 * A bare `z.input` still leaves BOTH source pairs optional (every field has
 * a `.default(...)`), so `{}` type-checked even though the `.refine`s above
 * reject it at parse time and Rust rejects it at runtime
 * (`resume_source`/`job_source`, `resolve.rs`) — a caller could build a
 * request nothing downstream would ever accept and the compiler would say
 * nothing. The two groups are intersected as two SEPARATE two-member unions,
 * not one `RequireOneOf` helper applied twice to the same base type — nesting
 * it that way does not enforce both groups independently (the second
 * application can subsume the first). Each pair still allows BOTH fields set
 * — id wins at runtime, same as today — only "neither" is excluded.
 */
type ResumePipelineRunRequestBase = Omit<
  z.input<typeof ResumePipelineRunSchema>,
  'resumeId' | 'resumeText' | 'jobId' | 'jobAdText'
>;
export type ResumePipelineRunRequest = ResumePipelineRunRequestBase &
  ({ resumeId: string; resumeText?: string } | { resumeId?: string; resumeText: string }) &
  ({ jobId: string; jobAdText?: string } | { jobId?: string; jobAdText: string });

/**
 * Request for `resumePipeline.regenerateSection` — re-run ONE section of a
 * finished run through the repair splice primitive.
 *
 * `sectionKey` is the closed `PipelineSectionKey` grammar
 * (`summary` | `skills` | `experience:<u8>` | `projects` | `education`).
 * **`"header"` is not in it and is rejected at the boundary**: the contact
 * header is owned by the editor at export time (ADR-0021), so a model may never
 * rewrite it. `note` is an optional free-text steer and is FENCED as untrusted
 * data in the user turn (ADR-010) — never appended to a system prompt.
 */
export const ResumePipelineRegenerateSectionSchema = z.object({
  runId: z.string().min(1).max(128),
  sectionKey: z.string().min(1).max(24),
  note: z.string().max(500).optional(),
});
export type ResumePipelineRegenerateSectionRequest = z.infer<
  typeof ResumePipelineRegenerateSectionSchema
>;

/**
 * Request for `resumePipeline.resolveFabrication` — the user's per-bullet
 * verdict on ONE surviving fabrication finding in a run's quality report.
 *
 * Nothing is ever removed silently: a run stays `needs_review` until every
 * flagged bullet carries a decision, and the decision is recorded IN the
 * persisted report (inside the document's own slot, so a later re-validation of
 * the other document cannot orphan it).
 */
export const ResumePipelineResolveFabricationSchema = z.object({
  runId: z.string().min(1).max(128),
  /** Stable identity of the finding within the report: `<code>#<index>` as the
   *  report lists them. */
  issueKey: z.string().min(1).max(128),
  decision: z.enum(['remove', 'keep']),
});
export type ResumePipelineResolveFabricationRequest = z.infer<
  typeof ResumePipelineResolveFabricationSchema
>;

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
  // Scraped salary (Adzuna only, today) — grounds the salary application answer.
  // Absent/unknown when the board didn't report one.
  salaryMin: z.number().optional(),
  salaryMax: z.number().optional(),
  salaryCurrency: z.string().optional(),
});
export type ApplicationTrackRequest = z.infer<typeof ApplicationTrackSchema>;

// Patch the user-editable tracking fields of an existing Application. Each field
// is optional; an absent field is left unchanged. `nextActionAt` is nullable to
// allow explicitly clearing the reminder.
export const ApplicationUpdateSchema = z.object({
  id: z.string().min(1),
  notes: z.string().optional(),
  // Non-negative: the server-side guard (`parse_next_action_at`) rejects a
  // negative epoch-ms, so the wire contract mirrors it rather than silently
  // clearing the reminder on a bad value.
  // Clearing the reminder (explicit null) or moving it to a new date also
  // resets the backend's follow-up-notification marker, so the new due date
  // notifies once. Setting the SAME value again is not a reschedule.
  nextActionAt: z.number().int().min(0).nullable().optional(),
  comp: z.string().optional(),
  // The canonical primary contact for the application (recruiter / hiring
  // manager / apply-by-email recipient — one person, one pair). Server-side
  // BOTH inbound names go through the same trim + byte-cap + address-format
  // guards, so the caps here match the deprecated aliases below exactly.
  //
  // Byte-length (not `.max()`'s char-count), matching the Rust guards and the
  // `jobDescription` precedent below: a 200-CHARACTER CJK name is 600 bytes, so
  // a char cap passed here and then failed server-side with an error the user
  // could do nothing about.
  contactName: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 200, {
      message: 'contactName must be at most 200 bytes',
    })
    .optional(),
  contactEmail: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 254, {
      message: 'contactEmail must be at most 254 bytes',
    })
    .optional(),
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
  // DEPRECATED aliases of contactName/contactEmail, still accepted so existing
  // callers (the apply-by-email tab, the extension) keep working: a write under
  // either name lands in the SAME storage, and both names come back populated
  // with that one value. Sending both in one patch → the canonical one wins.
  // Byte-capped identically to the canonical pair above — they hit one column,
  // so a laxer alias would just be a way around the canonical bound.
  recipientName: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 200, {
      message: 'recipientName must be at most 200 bytes',
    })
    .optional(),
  recipientEmail: z
    .string()
    .trim()
    .refine((v) => new TextEncoder().encode(v).length <= 254, {
      message: 'recipientEmail must be at most 254 bytes',
    })
    .optional(),
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
  // Free-text (not `BoardId`-typed) deliberately, so a saved target with a since-
  // retired board id still deserializes. The 64 ceiling is just a generous sanity
  // bound against a corrupt/hostile autopilots.json — the real bound is the Rust
  // engine's server-side registry dedup+truncate (`max_boards_per_batch()`), so
  // this never needs to change as the board catalog grows.
  boards: z.array(z.string().min(1)).min(1).max(64),
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
  // Watched-companies-only mode (ADR-030 §e): when true, a run resolves the
  // user's currently-starred discovered companies at run time and scrapes only
  // those per-ATS company slugs (instead of the curated seed). Additive +
  // optional so old autopilots deserialize unchanged.
  watchedCompaniesOnly: z.boolean().optional(),
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
  // Phase 4 (opt-in): attach a short AI-reasoned note to the top matches of each
  // scheduled run. Read-only enrichment — never applies or submits anything.
  // Optional (like the other autopilot fields); the Rust store owns the `false`
  // default, so absent → notes off and existing autopilots stay note-free.
  assistant: z.boolean().optional(),
  // Provider snapshot for the headless AI-notes call. The scheduler runs with no
  // renderer, so the active provider/model/base URL resolved at opt-in time is
  // persisted here; the run then resolves the SAME centralized provider layer
  // (`Completer`) that `ai_generate` uses. Absent/empty → notes skip gracefully.
  assistantProvider: z.string().optional(),
  assistantModel: z.string().optional(),
  assistantBaseUrl: z.string().optional(),
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
  // ISO 3166-1 alpha-2, captured alongside `location` from a picked geocode
  // suggestion (mirrors AutopilotTargetSchema.countryCode) — lets a seeded
  // location carry its real country instead of a scraper having to guess one.
  countryCode: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2}$/)
    .optional(),
  techStack: z.array(TechStackItemSchema).optional(),
  // Backend-readable copy of the renderer's own `applicant.salaryExpectation`
  // (Task #30) — free text, no client-side length cap; the Rust store clamps
  // it (~200 bytes) at the write boundary, matching every other
  // renderer-supplied string in this contract.
  salaryExpectation: z.string().optional(),
  // Extra recruiting/staffing agency company names, merged with the built-in
  // const list when cross-board dedup flags a posting's `isAgency` (ADR-029 §i).
  // Free text; the Rust store clamps each entry + the list length at the write
  // boundary (per the dedicated single-column setter, PR #695 pattern). `.max`
  // mirrors the Rust `MAX_EXTRA_AGENCY_COMPANIES` cap (same guard as otherKeys).
  extraAgencyCompanies: z.array(z.string().trim().min(1)).max(500).optional(),
});

// Cross-board dedup "split" request (ADR-029 §h): mark `memberKey` as NOT a
// duplicate of each of `otherKeys` (opaque canonical job keys the renderer
// echoes back from a cluster's members). `autopilotId` scopes an autopilot
// found-jobs split so that record's annotations are recomputed too.
export const DedupMarkNotDuplicateRequestSchema = z.object({
  memberKey: z.string().trim().min(1),
  otherKeys: z.array(z.string().trim().min(1)).min(1).max(32),
  autopilotId: z.string().optional(),
});

// ─── Discovery (passively-harvested ATS company slugs) — ADR-030 §f ───────────

// Typeahead search over discovered/seeded company slugs + display names. The
// query may be empty (returns the top rows); the ~100-char ceiling is a generous
// sanity bound — the Rust command re-clamps server-side (renderer Zod is not a
// trust boundary). `atsKind` optionally scopes to a single ATS (unused today).
export const DiscoverySearchRequestSchema = z.object({
  query: z.string().trim().min(0).max(100),
  atsKind: z.string().optional(),
});

// Star / unstar a discovered (or curated-seed) company — the user's "watched
// companies" set that a `watchedCompaniesOnly` autopilot resolves at run time.
export const DiscoveryStarRequestSchema = z.object({
  atsKind: z.string().trim().min(1),
  slug: z.string().trim().min(1),
  starred: z.boolean(),
});

export type DiscoverySearchRequest = z.infer<typeof DiscoverySearchRequestSchema>;
export type DiscoveryStarRequest = z.infer<typeof DiscoveryStarRequestSchema>;

export type AutopilotCreate = z.infer<typeof AutopilotCreateSchema>;
export type AutopilotUpdate = z.infer<typeof AutopilotUpdateSchema>;
export type JobPreferences = z.infer<typeof JobPreferencesSchema>;
export type DedupMarkNotDuplicateRequest = z.infer<typeof DedupMarkNotDuplicateRequestSchema>;

export type AiGenerateRequest = z.infer<typeof AiGenerateRequestSchema>;
export type ModelInspectResult = z.infer<typeof ModelInspectResultSchema>;
export type DocumentImportRequest = z.infer<typeof DocumentImportRequestSchema>;
export type ScrapeBoardsRequest = z.infer<typeof ScrapeBoardsRequestSchema>;
export type ScrapeUrlRequest = z.infer<typeof ScrapeUrlRequestSchema>;
export type MatchResumeRequest = z.infer<typeof MatchResumeRequestSchema>;
export type MatchTextRequest = z.infer<typeof MatchTextRequestSchema>;
export type ResumeTrimSuggestionsRequest = z.infer<typeof ResumeTrimSuggestionsRequestSchema>;
