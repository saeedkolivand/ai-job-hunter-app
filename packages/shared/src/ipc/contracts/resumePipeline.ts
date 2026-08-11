import type { PipelineStageEvent } from '../../events/pipeline.js';
import type {
  GenerationDepth,
  ResumePipelineRegenerateSectionRequest,
  ResumePipelineResolveFabricationRequest,
  ResumePipelineRunRequest,
} from '../../schemas/index.js';
import type { ContentReportPayload } from './resume.js';

/**
 * The staged résumé pipeline — `quality` depth today, `max` in Phase 4.
 *
 * `run` starts the background run and returns its ids immediately; stage
 * progress streams as `pipeline:stage` events (subscribe via {@link
 * ResumePipelineContract.onStage}) and the run's own terminal signal is the
 * `pipeline:stage` event whose `phase` is `finish`/`error` for the LAST stage —
 * NOT the `jobs:event`, and not `awaitAiStream` resolving.
 *
 * **Why the draft stream is not the completion signal.** The draft stage
 * streams under the run's own umbrella `jobId` so the user watches the résumé
 * appear, which means the shared stream machinery marks that job completed the
 * moment the draft finishes — while validation and up to two repair rounds are
 * still to come. That stream is DISPLAY-ONLY. A renderer that treats
 * `awaitAiStream` resolving as "the run is done" will show an unvalidated,
 * unrepaired draft as final.
 */
export interface ResumePipelineContract {
  /**
   * Start one staged run. Resolves as soon as the run is admitted (the
   * concurrency wait happens inside the run, so a queued run still returns its
   * ids immediately).
   */
  run(req: ResumePipelineRunRequest): Promise<PipelineRunStarted>;

  /** One run with its full stage trail. `null` for an unknown id. */
  get(runId: string): Promise<PipelineRunDetail | null>;

  /**
   * The retained runs for one posting, newest first — at most
   * `RETENTION_RUNS_PER_JOB` (3), which is what the backend keeps.
   */
  listForJob(jobUrl: string): Promise<PipelineRunSummary[]>;

  /**
   * Re-generate ONE section of a finished run and splice it back in, through
   * the same primitive the repair loop uses. Rejects `"header"` (and anything
   * else outside the closed `PipelineSectionKey` grammar) at the boundary.
   */
  regenerateSection(req: ResumePipelineRegenerateSectionRequest): Promise<PipelineRunDetail>;

  /**
   * Record the user's Remove/Keep verdict on ONE surviving fabrication finding.
   * The run stays `needs_review` until every flagged bullet has one.
   */
  resolveFabrication(req: ResumePipelineResolveFabricationRequest): Promise<PipelineRunDetail>;

  /** Subscribe to the `pipeline:stage` progress stream. Returns an unsubscribe fn. */
  onStage(handler: (event: PipelineStageEvent) => void): () => void;
}

/** What `run` hands back: the two ids every later call and every event key on. */
export interface PipelineRunStarted {
  /** `pipeline_runs.id` — the key for `get`, `regenerateSection`,
   *  `resolveFabrication`, and every `pipeline:stage` event's `runId`. */
  runId: string;
  /** The umbrella job id — the key for `jobs.cancel`, for `jobs:event`, and
   *  for the draft stage's display-only `ai:stream` deltas. */
  jobId: string;
}

/**
 * How a run ended. `needsReview` is NOT a failure: the document exists and is
 * usable, but the report carries findings the user has to resolve per bullet
 * (see {@link ResumePipelineContract.resolveFabrication}) — a run in this state
 * must never be presented as clean.
 */
export type PipelineRunStatus = 'running' | 'completed' | 'needsReview' | 'failed' | 'cancelled';

/** One run, without its stage trail — the shape the runs list renders. */
export interface PipelineRunSummary {
  runId: string;
  jobUrl: string;
  /** Which flow produced this run. `"resume"` for this contract; the same
   *  tables host other staged runs under other kinds. */
  kind: string;
  depth: GenerationDepth;
  status: PipelineRunStatus;
  startedAt: number;
  finishedAt?: number;
  /** The backend `StoppedReason` wire token (`done`, `run_timeout`,
   *  `max_repairs`, `cancelled`, …) — absent while the run is still going. */
  stoppedReason?: string;
  metrics: PipelineRunMetrics;
}

/** Counts and durations only — never generated text (ADR-027). */
export interface PipelineRunMetrics {
  /** Provider round-trips this run actually made. */
  calls?: number;
  /** How many of {@link PipelineRunMetrics.calls} were served from the stage
   *  cache instead of a provider. */
  cached?: number;
  /** Repair rounds run (0–2). */
  repairRounds?: number;
  /** Whether the last repair round was REVERTED because it produced strictly
   *  more criticals than the draft it was trying to fix. */
  reverted?: boolean;
  issueCount?: number;
  criticalCount?: number;
  ms?: number;
}

/** One run plus everything needed to render its timeline and its report. */
export interface PipelineRunDetail extends PipelineRunSummary {
  events: PipelineRunEvent[];
  /**
   * The run's deterministic content report for the résumé, and for the cover
   * letter when one was in scope. `null` until the validate stage has run.
   */
  report: PipelineQualityReport | null;
  /** The finished résumé body. Empty until the draft stage completes. */
  resumeText: string;
}

/** One stage event as persisted (the durable twin of `pipeline:stage`). */
export interface PipelineRunEvent {
  seq: number;
  ts: number;
  stage: string;
  phase: 'start' | 'finish' | 'error';
  /** The stage's own small summary payload — counts, section keys, a stopped
   *  reason. Clamped at the store; never the generated document. */
  artifact: unknown;
}

/**
 * The persisted quality-report wrapper as the pipeline writes it into
 * `AiGenerationRecord.qualityReport`. Structurally the renderer's existing
 * `QualityReport` (`schemaVersion: 2`, per-document slots), with two
 * pipeline-only additions the renderer must tolerate:
 *
 * - `pipeline` is `'quality'`/`'max'` here, not only `'fast'`;
 * - a slot may carry `fabrications`, the per-bullet review list.
 */
export interface PipelineQualityReport {
  schemaVersion: 2;
  pipeline: GenerationDepth;
  generatedAt: number;
  resume?: PipelineQualityReportSlot;
  coverLetter?: PipelineQualityReportSlot;
}

/** One document's slot: its report, its staleness anchor, and any surviving
 *  fabrication findings awaiting a per-bullet verdict. */
export interface PipelineQualityReportSlot {
  report: ContentReportPayload;
  /** djb2 hash of the EXACT text `report` validated — the renderer's existing
   *  staleness anchor, computed Rust-side with the identical algorithm. */
  sourceTextHash: number;
  /** Surviving fabrication findings, in report order. Absent when the run
   *  found none. */
  fabrications?: PipelineFabrication[];
}

/** One flagged bullet awaiting (or carrying) the user's verdict. */
export interface PipelineFabrication {
  /** `<code>#<index>` — echo this back as `issueKey`. */
  issueKey: string;
  code: string;
  /** The offending span, verbatim from the generated document. */
  evidence: string;
  /** `undefined` until the user decides — which is what keeps the run
   *  `needsReview`. */
  decision?: 'remove' | 'keep';
}

export const RESUME_PIPELINE_CHANNELS = {
  run: 'resumePipeline:run',
  get: 'resumePipeline:get',
  listForJob: 'resumePipeline:listForJob',
  regenerateSection: 'resumePipeline:regenerateSection',
  resolveFabrication: 'resumePipeline:resolveFabrication',
} as const;
