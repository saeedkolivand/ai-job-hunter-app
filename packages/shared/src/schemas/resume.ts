import { z } from 'zod';

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
 * IPC boundary a non-UI caller can reach directly. `semanticScoringEnabled`
 * mirrors `MatchResumeRequestSchema`'s field of the same name — an omitted
 * flag defaults to keyword-only on the Rust side, never "provider decides".
 */
export const MatchTextRequestSchema = z.object({
  resumeId: z.string().min(1),
  jobText: z.string().min(1).max(200_000),
  semanticScoringEnabled: z.boolean().optional(),
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
    /** Whether the run's `draft` stage should generate a résumé. Default true:
     *  an existing caller that never sets this gets byte-identical behavior.
     *  `false` is the cover-letter-only run — `draft` no-ops at zero cost,
     *  nothing résumé-shaped is validated, repaired or persisted, and the
     *  posting's saved résumé is left alone (`ai_generations::merge_application`'s
     *  pick-non-empty merge keeps the stored value for an empty incoming field). The grounding stages
     *  (`analyze_job`, `match_evidence`, `strategy`) still run: the letter
     *  prompt fences `<resume_strategy>` and is told to follow it. */
    includeResume: z.boolean().default(true),
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
  })
  .refine((data) => data.includeResume || data.includeCoverLetter, {
    message: 'a run must produce at least one document',
    path: ['includeResume'],
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

export type MatchResumeRequest = z.infer<typeof MatchResumeRequestSchema>;
export type MatchTextRequest = z.infer<typeof MatchTextRequestSchema>;
export type ResumeTrimSuggestionsRequest = z.infer<typeof ResumeTrimSuggestionsRequestSchema>;
