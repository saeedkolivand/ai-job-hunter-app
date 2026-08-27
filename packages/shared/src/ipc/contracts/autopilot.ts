import type { AutopilotCreate, AutopilotUpdate } from '../../schemas/index.js';
import type { Autopilot, AutopilotRunStatus, JobTrustAssessment } from '../../types/index.js';

/**
 * Job-discovery agent: saved searches that run on a schedule, then rank and
 * surface the matching jobs. It never submits anything — auto-apply was
 * removed, so a stored cover letter is a reusable starting point for the apply
 * assistant, and the opt-in `assistant` notes are read-only enrichment. The
 * user tailors and applies by hand.
 */
export interface AutopilotContract {
  list(): Promise<Autopilot[]>;

  get(req: { autopilotId: string }): Promise<Autopilot | null>;

  create(req: AutopilotCreate): Promise<Autopilot>;

  update(req: { autopilotId: string } & AutopilotUpdate): Promise<Autopilot>;

  remove(req: { autopilotId: string }): Promise<void>;

  /**
   * Run an autopilot now. The backend command *resolves* (does not reject) with
   * an `{ error }` payload on a scrape failure or unknown id, so callers MUST
   * inspect `error` — a resolved value is not proof of success. `jobId` is
   * present on every non-error outcome (success / cancel).
   *
   * `status` mirrors the outcome persisted on the record (`completed` /
   * `completedWithErrors` / `failed`) on a run that reached the record site, so
   * a caller can tell a run that found real jobs from one where every board
   * failed WITHOUT re-fetching the record. Absent on the early `{ error }` and
   * `{ cancelled }` outcomes.
   *
   * `skipped: 'already-running'` is the concurrent-run guard's early return: a
   * double-invoke of the SAME autopilot (a scheduler retry racing a fresh
   * occurrence, or two manual triggers) is de-duplicated rather than run twice.
   * No `jobId`/`error`/`status` accompanies it — no run happened for this call.
   */
  run(req: { autopilotId: string }): Promise<{
    jobId?: string;
    error?: string;
    status?: AutopilotRunStatus;
    skipped?: 'already-running';
  }>;

  pause(req: { autopilotId: string }): Promise<void>;

  resume(req: { autopilotId: string }): Promise<void>;

  onStep(handler: (event: AutopilotStepEvent) => void): () => void;

  /** Fired by the shell (tray "New jobs" click or a validated deep link) to
   *  focus an autopilot's found-jobs panel. An empty `autopilotId` is a pure
   *  "refresh the list" signal (e.g. after a tray Pause-All) with no navigation. */
  onFocus(handler: (event: AutopilotFocusEvent) => void): () => void;

  /** Atomically take + clear the autopilot-focus intent buffered by the shell.
   *  A cold-start `ajh://autopilot/<id>` deep link fires the `autopilot:focus`
   *  emit during Rust setup, before the renderer's `useAutopilotFocusNavigation`
   *  listener attaches, so the event is lost; the shell buffers the id and the
   *  renderer pulls it once its JS loop is live (on mount + on the emitted
   *  event). The IPC response is reliable where the event was not. Resolves to
   *  the buffered `autopilotId`, or `null` when nothing is buffered (the common
   *  case — only set by a cold-start deep link). Mirrors `menu.takePending`. */
  takePendingFocus(): Promise<string | null>;

  /**
   * The current top-scoring matches across every non-archived autopilot,
   * recomputed at query time (nothing about this list is persisted). The
   * population is every found job belonging to an `active` or `paused`
   * autopilot — an archived one contributes nothing, but pausing an autopilot
   * only stops it scraping, so its past finds still compete here. Jobs the
   * same posting was found by are merged into one row (`sources`), and a row
   * only appears when its best score clears its own score-kernel's "High"
   * tier cut (`MATCH_TIER_CUTS`) — an unscored job never qualifies. `matches`
   * is capped at a fixed size as a payload guard only; `total` is the
   * qualifying count before that cap, so `total > matches.length` signals
   * truncation the caller may want to communicate (e.g. "and N more").
   */
  bestMatches(): Promise<AutopilotBestMatchesResult>;
}

export interface AutopilotStepEvent {
  jobId: string;
  autopilotId: string;
  step: string;
  detail: string;
}

export interface AutopilotFocusEvent {
  autopilotId: string;
}

/** One autopilot that surfaced a {@link AutopilotBestMatch} row. */
export interface AutopilotBestMatchSource {
  autopilotId: string;
  autopilotName: string;
  /** The originating autopilot is currently paused (still contributes rows). */
  paused: boolean;
  /** When THIS autopilot first surfaced the job (its own found-jobs entry). */
  foundAt: number;
}

/**
 * One cross-autopilot best-match row. When the same posting was found by more
 * than one autopilot (or scraped from more than one board), those finds
 * collapse into a single row — `sources.length > 1` is how a caller tells a
 * merged duplicate from a single-source match, and `clusterMembers` lists the
 * other board copies the way a `Posting`/`AutopilotFoundJob` row already does.
 */
export interface AutopilotBestMatch {
  /** Cluster id — the row's stable identity across refetches. */
  key: string;
  title: string;
  company: string;
  /** The canonical member's url — the "view job" target. */
  url: string;
  location?: string;
  board?: string;
  salaryMin?: number;
  salaryMax?: number;
  salaryCurrency?: string;
  /** The best-scored member's score. Always present — an unscored cluster
   *  never qualifies for this list. */
  score: number;
  scoreSource: 'keyword' | 'combined';
  scoreProvisional?: boolean;
  postedAt?: number;
  /** EARLIEST discovery across every source that surfaced this job. */
  foundAt: number;
  applied?: boolean;
  isAgency?: boolean;
  trust?: JobTrustAssessment;
  /** A cluster member's AI-reasoned note, when any member has one. */
  assistantNotes?: string;
  clusterMembers?: Array<{ key: string; board?: string; url: string }>;
  /** Every autopilot that surfaced this job. Length > 1 means a merged duplicate. */
  sources: AutopilotBestMatchSource[];
}

/** Response shape for {@link AutopilotContract.bestMatches}. */
export interface AutopilotBestMatchesResult {
  /** Qualifying rows, pre-sorted (tier desc, score desc, key asc). Capped at a
   *  fixed size — see {@link AutopilotContract.bestMatches}. */
  matches: AutopilotBestMatch[];
  /** Qualifying count BEFORE the cap. `total > matches.length` means truncated. */
  total: number;
  /** Distinct autopilots contributing at least one qualifying row. */
  autopilotCount: number;
}

export const AUTOPILOT_CHANNELS = {
  list: 'autopilot:list',
  get: 'autopilot:get',
  create: 'autopilot:create',
  update: 'autopilot:update',
  remove: 'autopilot:remove',
  run: 'autopilot:run',
  pause: 'autopilot:pause',
  resume: 'autopilot:resume',
  bestMatches: 'autopilot:bestMatches',
} as const;
