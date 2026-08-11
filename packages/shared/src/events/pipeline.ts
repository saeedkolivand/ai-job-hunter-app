export const PIPELINE_EVENTS = {
  stage: 'pipeline:stage',
} as const;

/**
 * Which half of a stage's lifecycle this event reports.
 * - `start` — the stage is about to run (emitted from the hook's `before`).
 * - `finish` — it completed successfully.
 * - `error` — it failed; the run aborts after this event.
 */
export type PipelineStagePhase = 'start' | 'finish' | 'error';

/**
 * Longest a `sectionKey` may be ON THE WIRE, in UTF-16 code units.
 *
 * NORMATIVE for the Phase-3 Rust emitter: it must reject (not truncate) anything
 * longer. The longest legal key is `experience:255` (14), so this is pure
 * headroom — the point is that a value derived from MODEL output can never be
 * long enough to matter, no matter what the model emits.
 */
export const SECTION_KEY_MAX_LENGTH = 24;

/** The section keys that carry no index. */
const FIXED_SECTION_KEYS = ['summary', 'skills', 'projects', 'education'] as const;

const EXPERIENCE_PREFIX = 'experience:';

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
  (typeof FIXED_SECTION_KEYS)[number] | `${typeof EXPERIENCE_PREFIX}${number}`;

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
  if ((FIXED_SECTION_KEYS as readonly string[]).includes(value)) return true;
  if (!value.startsWith(EXPERIENCE_PREFIX)) return false;
  const index = value.slice(EXPERIENCE_PREFIX.length);
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
