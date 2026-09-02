/**
 * Which single setup step the Dashboard should nudge next.
 *
 * Pure derivation on purpose: the tile reads its three signals from service
 * hooks and passes plain values in here, so every branch below is testable
 * without React Query, IPC or a router.
 */

/**
 * One input to the derivation.
 *
 * Two non-boolean states, because "no answer" arrives in two shapes that have
 * opposite right responses:
 *
 * - `'pending'` — the query has no data YET. It is NOT `false`: telling a
 *   returning user to add a résumé they already have, for the half-second
 *   before the cache answers, is worse than showing nothing.
 * - `'unavailable'` — the query REJECTED. Its `data` stays `undefined` for
 *   good, so folding this into `'pending'` hid the row permanently — the one
 *   outcome the tile is not allowed to have, being the app's only always-there
 *   route to help.
 */
export type StepSignal = boolean | 'pending' | 'unavailable';

/** The three things that have to be true before the app can do its job. */
export type NextStepId = 'resume' | 'ai' | 'job';

/**
 * Whether each step is met.
 *
 * - `resume` — a document exists to tailor from.
 * - `ai` — an AI provider is configured and reachable.
 * - `job` — at least one posting carries a tracked interaction (see
 *   `TRACKED_INTERACTION_TYPES`; a dismissal is not one).
 */
export type NextStepSignals = Record<NextStepId, StepSignal>;

/** What the tile should render. */
export type NextStep =
  | { kind: 'pending' }
  | { kind: 'unavailable' }
  | { kind: 'step'; step: NextStepId; done: number; total: number }
  | { kind: 'done' };

/**
 * The order the first unmet step is picked in.
 *
 * Not a sequence the user must follow — any step can be done first, which is
 * why the badge counts met steps ("2 of 3 done") instead of claiming "Step 1
 * of 3". The order is only a preference for what to suggest when several are
 * unmet: a résumé is the input everything else consumes, AI is what turns it
 * into output, and a job is what both are pointed at.
 */
const STEP_ORDER: readonly NextStepId[] = ['resume', 'ai', 'job'];

/**
 * The first unmet step, or `done` / `unavailable` / `pending`.
 *
 * `done` counts met steps regardless of position, so a user who deletes their
 * résumé after finishing sees the résumé step with "2 of 3 done" — true, where
 * a positional counter would lie.
 */
export function deriveNextStep(signals: NextStepSignals): NextStep {
  if (STEP_ORDER.some((step) => signals[step] === 'pending')) return { kind: 'pending' };

  // Checked AFTER `'pending'`, which only delays this row and never replaces
  // it (a rejected query does not recover on its own): during a cold boot
  // where one signal has already failed and the rest are still in flight, the
  // tile stays empty instead of flashing a status row a moment before the
  // steady state. Never merged into `'pending'` — that is the bug this state
  // exists for, a failed read hiding the row for the whole session.
  if (STEP_ORDER.some((step) => signals[step] === 'unavailable')) return { kind: 'unavailable' };

  const done = STEP_ORDER.filter((step) => signals[step] === true).length;
  const next = STEP_ORDER.find((step) => signals[step] !== true);

  return next === undefined
    ? { kind: 'done' }
    : { kind: 'step', step: next, done, total: STEP_ORDER.length };
}

/**
 * Every `dashboard.nextStep.*` key the tile can render, so the locale-parity
 * test asserts the same set the component asks `t()` for.
 *
 * Not every string the tile can show: the AI step's description is borrowed
 * from `AiSetupHint`'s `aiSetup.*` reason copy (`AI_REASON_KEY` in the tile),
 * which lives in a different namespace and is walked by its own list in
 * `next-step.test.ts`.
 */
export const NEXT_STEP_TEXT_KEYS: readonly string[] = [
  ...STEP_ORDER.flatMap((step) => [
    `dashboard.nextStep.${step}.title`,
    `dashboard.nextStep.${step}.description`,
  ]),
  'dashboard.nextStep.progress',
  'dashboard.nextStep.doneTitle',
  'dashboard.nextStep.unavailableTitle',
  'dashboard.nextStep.help',
];
