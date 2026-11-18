/**
 * Outcome insights and the volume nudge: what happened to the applications the
 * user sent, and a gentle signal when they are sending a lot of them untailored.
 *
 * Read-only and derived on every call from the application and generation
 * records (nothing is stored). Every shape here is machine-readable: enums and
 * numbers, no formatted prose, so an agent can reason over it directly.
 */

/** The axis a {@link InsightsOutcomeGroup} slices the applications by. */
export type InsightsDimension =
  'board' | 'tailored' | 'coverLetter' | 'targetLanguage' | 'month' | 'template';

/** How many applications in a group landed in each outcome. */
export interface InsightsOutcomeCounts {
  /** Ever reached screening, interviewing, offer or accepted, whatever the status is now. */
  positiveReply: number;
  /** Closed as rejected without ever reaching one of those stages. */
  rejected: number;
  /** Ghosted, or still `applied` past the no-reply window (`NO_REPLY_AFTER_DAYS` in the Rust `insights` module). */
  noReply: number;
  /** Still `applied` inside that window: too early to call. Never part of a rate. */
  pending: number;
}

/** Fractions in 0..1 over the decided (non-pending) applications of a group. */
export interface InsightsOutcomeRates {
  positiveReply: number;
  rejected: number;
  noReply: number;
}

/**
 * One slice of one dimension. `key` is a stable machine value: the board id,
 * `tailored`/`untailored`, `included`/`none`, a language code, `YYYY-MM` (UTC),
 * or a template id; `unknown` when the value is not recorded (no generation, or
 * no export yet for `template`).
 */
export interface InsightsOutcomeGroup {
  dimension: InsightsDimension;
  key: string;
  counts: InsightsOutcomeCounts;
  /**
   * `null` while the group has fewer decided applications than `MIN_GROUP_SAMPLE`
   * (Rust `insights` module): counts are still reported, but a rate over so few
   * would be noise, never a fact.
   */
  rates: InsightsOutcomeRates | null;
}

/** Which condition of the volume nudge held. */
export type InsightsNudgeTrigger = 'untailored' | 'nearCopyLetters';

/** The volume nudge: counts over the trailing window and the conditions that held. */
export interface InsightsVolumeNudge {
  /** The trailing window the counts cover, in hours. */
  windowHours: number;
  /** Applications that became `applied` inside the window. */
  applications: number;
  /** Of those, how many have no AI generation for their job. */
  untailored: number;
  /** Of those, how many carry a non-empty cover letter. */
  letters: number;
  /** Of those letters, how many are near-copies of another letter in the window. */
  nearCopyLetters: number;
  /** The conditions that held; never empty (the nudge is `null` otherwise). */
  triggers: InsightsNudgeTrigger[];
}

/** The whole outcome-insights report. */
export interface InsightsOutcomesReport {
  /** Every group of every dimension that has at least one counted application. */
  groups: InsightsOutcomeGroup[];
  /**
   * Set only when enough applications (`NUDGE_MIN_APPLICATIONS`) became `applied`
   * in the trailing window AND more than half are untailored or more than half of
   * their letters are near-copies of another letter; otherwise `null`. The
   * thresholds are the named constants in the Rust `insights` module.
   */
  nudge: InsightsVolumeNudge | null;
  /** When the report was computed (ms since epoch). */
  generatedAt: number;
}

export interface InsightsContract {
  /**
   * Outcome of every application the user sent, grouped by board, tailored vs
   * not, cover letter vs none, target language, month and export template, plus
   * the volume nudge when it fires. Withdrawn and saved applications are never
   * counted; pending ones are counted but kept out of every rate.
   */
  outcomes(): Promise<InsightsOutcomesReport>;
}

export const INSIGHTS_CHANNELS = {
  outcomes: 'insights:outcomes',
} as const;
