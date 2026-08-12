export const PIPELINE_EVENTS = {
  stage: 'pipeline:stage',
} as const;

/**
 * Which half of a stage's lifecycle this event reports.
 * - `start` — the stage is about to run (emitted from the hook's `before`).
 * - `finish` — it completed successfully.
 * - `error` — it failed; the run aborts after this event.
 *
 * The array is the source of truth (NORMATIVE for the Phase-3 emitter);
 * {@link PipelineStagePhase} is derived from it so a value can never be added
 * to the type without also widening the array the lock test asserts against.
 */
export const PIPELINE_STAGE_PHASES = ['start', 'finish', 'error'] as const;

/** The closed phase vocabulary — see {@link PIPELINE_STAGE_PHASES}. */
export type PipelineStagePhase = (typeof PIPELINE_STAGE_PHASES)[number];

/**
 * Every stage name the staged résumé pipeline can run, in pipeline order.
 *
 * The ORDER-PRESERVED UNION of the two Rust depth lists (`QUALITY_STAGES` and
 * `MAX_STAGES` in `pipeline/resume/mod.rs`), not a third hand-written list: a
 * Rust test pins both directions — every depth's stages must appear here, and
 * every name here must belong to at least one depth — so this cannot drift from
 * the pipelines it names.
 *
 * Emitted into Rust by `pnpm gen:ipc` (`ipc_contracts::events::PIPELINE_STAGES`)
 * for the same reason {@link PIPELINE_STAGE_PHASES} is: it is a CLOSED
 * vocabulary that both sides key on. Two consumers today —
 *
 * - a `pipeline:stage` event's `stage` field (the renderer's timeline, which
 *   ignores a name it does not know, so a rename is otherwise silent);
 * - the per-stage model overrides (`ai_stage_overrides`), whose primary key is
 *   one of these names — a row for anything else is rejected at write AND at
 *   import, so a tampered backup cannot introduce a stage that never runs.
 */
export const PIPELINE_STAGES = [
  'analyze_job',
  'match_evidence',
  'strategy',
  'draft',
  'sections',
  'assemble',
  'validate',
  'repair',
  'llm_judge',
] as const;

/** The closed stage vocabulary — see {@link PIPELINE_STAGES}. */
export type PipelineStage = (typeof PIPELINE_STAGES)[number];

/**
 * The stages that make NO provider call in any depth that runs them: `assemble`
 * renders already-paid-for sections into the document body, `validate` is a
 * deterministic comparison against the source résumé.
 *
 * Derived from the Rust `Pipeline::free_stage_names()` of BOTH depths and pinned
 * against them (`pipeline::resume::test`). "In any depth that runs them" is the
 * careful part: `free_stage_names` is per-depth, and a stage that is free in one
 * pipeline but pays in another would not belong here.
 *
 * NORMATIVE: a model override on one of these is refused at write AND at import.
 * There is no model to choose — the stage never asks one anything — so the
 * control would be inert, and (before the refusal existed) a malformed row on a
 * free stage could still fail a whole run at resolve time. Filter these out of
 * any per-stage model UI.
 */
export const PIPELINE_STAGES_FREE = ['assemble', 'validate'] as const;

/** Whether a stage can actually spend a provider call — the set a per-stage
 *  model picker should offer. */
export function isPayingPipelineStage(value: unknown): value is PipelineStage {
  return isPipelineStage(value) && !(PIPELINE_STAGES_FREE as readonly string[]).includes(value);
}

/** Runtime guard for {@link PipelineStage} — the checkable form of the closed
 *  set above, so a renderer that reads a stage name off the wire (or off a
 *  stored override row) can reject an unknown one instead of rendering it. */
export function isPipelineStage(value: unknown): value is PipelineStage {
  return typeof value === 'string' && (PIPELINE_STAGES as readonly string[]).includes(value);
}

/**
 * Longest a `sectionKey` may be ON THE WIRE, in UTF-16 code units.
 *
 * NORMATIVE for the Phase-3 Rust emitter: it must reject (not truncate) anything
 * longer. The longest legal key is `experience:255` (14), so this is pure
 * headroom — the point is that a value derived from MODEL output can never be
 * long enough to matter, no matter what the model emits.
 *
 * Emitted into Rust by `pnpm gen:ipc` (`ipc_contracts::events`) alongside
 * {@link PIPELINE_SECTION_KEYS_FIXED} and
 * {@link PIPELINE_SECTION_EXPERIENCE_PREFIX}, so "normative for the Rust
 * emitter" is a generated constant rather than a hand-copied literal.
 */
export const SECTION_KEY_MAX_LENGTH = 24;

/**
 * The section keys that carry no index — the fixed half of the
 * {@link PipelineSectionKey} grammar.
 *
 * Exported (and prefixed) because `pnpm gen:ipc` reads it as the source of
 * truth for the Rust `PIPELINE_SECTION_KEYS_FIXED`; the package barrel is flat,
 * so the name has to say which grammar it belongs to.
 */
export const PIPELINE_SECTION_KEYS_FIXED = ['summary', 'skills', 'projects', 'education'] as const;

/** The indexed half: `experience:<u8>`. Also codegen'd into Rust. */
export const PIPELINE_SECTION_EXPERIENCE_PREFIX = 'experience:';

/** `0`–`255`, no leading zeros — the u8 index of one experience entry. */
const EXPERIENCE_INDEX = /^(0|[1-9][0-9]{0,2})$/;

/**
 * Which résumé section a section-wise stage worked on.
 *
 * A CLOSED grammar, exactly like {@link PipelineStagePhase}, because this value
 * is derived from MODEL output and rides a channel that claims to be
 * content-free: `summary` | `skills` | `experience:<index>` | `projects` |
 * `education`, where `<index>` is a decimal u8 (0–255, no leading zeros)
 * identifying one experience entry. Nothing else is a section key — a free-form
 * label the model invented is a bug in the emitter, not a new section.
 *
 * The type states the grammar; {@link isPipelineSectionKey} enforces it at
 * runtime (a template-literal type cannot express "u8" or a length bound).
 */
export type PipelineSectionKey =
  | (typeof PIPELINE_SECTION_KEYS_FIXED)[number]
  | `${typeof PIPELINE_SECTION_EXPERIENCE_PREFIX}${number}`;

/**
 * Runtime guard for {@link PipelineSectionKey} — the checkable form of the
 * grammar above, so "closed set" is a test rather than a comment.
 *
 * NORMATIVE for the Phase-3 emitter: a `sectionKey` that fails this must never
 * reach the wire. Length is checked FIRST, so a hostile value is rejected before
 * any further work is done on it.
 */
export function isPipelineSectionKey(value: unknown): value is PipelineSectionKey {
  if (typeof value !== 'string' || value.length > SECTION_KEY_MAX_LENGTH) return false;
  if ((PIPELINE_SECTION_KEYS_FIXED as readonly string[]).includes(value)) return true;
  if (!value.startsWith(PIPELINE_SECTION_EXPERIENCE_PREFIX)) return false;
  const index = value.slice(PIPELINE_SECTION_EXPERIENCE_PREFIX.length);
  return EXPERIENCE_INDEX.test(index) && Number(index) <= 255;
}

/**
 * Payload of the `pipeline:stage` event — the per-stage progress trail of one
 * multi-step run (camelCase; the Phase-3 Rust emitter's payload struct is the
 * hand-synced counterpart of this interface).
 *
 * Emitted by the Phase-3 shell hook that implements the Rust `StageHooks`
 * trait; the channel and this shape are frozen now so the contract is stable
 * before either side is written. Content-free by construction (ADR-027): stage
 * names, counts, and durations only — never generated text. `sectionKey` is the
 * one model-derived field, which is why its vocabulary is closed and bounded
 * rather than a free-form `string`.
 */
export interface PipelineStageEvent {
  /** The pipeline run this stage belongs to (`pipeline_runs.id`). */
  runId: string;
  /** The job/run id the renderer already filters `jobs:event` on, so a panel
   *  can correlate stage progress with the job it started. */
  jobId: string;
  /** The stage's stable name (Rust `Stage::name`), e.g. `"draft"`. */
  stage: string;
  phase: PipelineStagePhase;
  /** 0-based position of this stage in the pipeline. */
  index: number;
  /** How many stages the pipeline has in total — `index`/`total` drives the
   *  progress bar without the renderer knowing the stage list. */
  total: number;
  /** 1-based attempt number for this stage: `1` on the first run, `2`+ for a
   *  budgeted repair re-ask (see the Rust `Budget::max_repair_attempts`). */
  attempt: number;
  /** The résumé section this stage worked on, for section-wise stages only.
   *  Omitted from the wire otherwise. Closed vocabulary — see
   *  {@link PipelineSectionKey} / {@link isPipelineSectionKey}. */
  sectionKey?: PipelineSectionKey;
  /** Wall-clock duration of the stage body. Present on `finish`/`error` only. */
  ms?: number;
  /** Total content-validation issues found by this stage. `finish`/`error` only. */
  issueCount?: number;
  /** How many of `issueCount` were CRITICAL — the subset that blocks. */
  criticalCount?: number;
}
