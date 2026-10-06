import type { Autopilot, AutopilotRunStatus } from '@ajh/shared';

/**
 * Persisted run-outcome → badge label key + color. A `Partial` map IS the
 * graceful fallback: a happy `completed`/`inProgress` — or any unknown/future
 * `runStatus` — is simply absent from the map, so no badge renders and nothing
 * ever prints a raw enum string. `failed` reads as an error (red);
 * `completedWithErrors` (some boards failed/truncated) and `interrupted` read as
 * warnings (amber).
 */
const RUN_STATUS_BADGE: Partial<
  Record<AutopilotRunStatus, { labelKey: string; className: string }>
> = {
  failed: { labelKey: 'autopilot.badge.failed', className: 'bg-red-400/15 text-red-300' },
  completedWithErrors: {
    labelKey: 'autopilot.badge.completedWithErrors',
    className: 'bg-amber-400/15 text-amber-300',
  },
  interrupted: {
    labelKey: 'autopilot.badge.interrupted',
    className: 'bg-amber-400/15 text-amber-300',
  },
};

/**
 * Cry-wolf guard (PR B carry-over 2): a `failed` run whose boards were ALL merely
 * skipped (needs-login / needs-keys / needs-company) — none actually errored —
 * isn't a failure, it's an unconfigured run. Present it neutrally + actionably
 * ("needs configuration") instead of a red "Failed", with the per-board chip
 * strip below spelling out exactly what to configure.
 */
const NEEDS_CONFIG_BADGE = {
  labelKey: 'autopilot.badge.needsConfig',
  className: 'bg-foreground/[0.06] text-foreground/70',
};

/** Badges that carry a hover/focus explainer now that the chip strip exists. */
const BADGE_HINT_KEY = {
  completedWithErrors: 'autopilot.badge.completedWithErrorsHint',
  needsConfig: 'autopilot.badge.needsConfigHint',
} as const;

export interface RunOutcomeBadge {
  labelKey: string;
  className: string;
}

/**
 * The persisted run-outcome badge (failed / completedWithErrors / interrupted)
 * and its optional hover/focus explainer. `needsConfig` overrides the red
 * `failed` badge with a neutral one; the badge is undefined for the happy path
 * and any unknown/future status — the explicit graceful fallback (renders
 * nothing rather than a raw enum). The per-board detail itself lives behind the
 * info icon next to "Found N".
 */
export function describeRunOutcome(
  runStatus: Autopilot['runStatus'],
  summaries: NonNullable<Autopilot['lastRunSummaries']>
): { badge: RunOutcomeBadge | undefined; hintKey: string | undefined } {
  // Cry-wolf guard (PR B carry-over 2): a `failed` run whose boards were ALL
  // merely skipped (none errored) is an UNCONFIGURED run, not a failure.
  const needsConfig =
    runStatus === 'failed' &&
    summaries.length > 0 &&
    summaries.every((s) => Boolean(s.skipped) && !s.error);
  const badge = needsConfig
    ? NEEDS_CONFIG_BADGE
    : runStatus
      ? RUN_STATUS_BADGE[runStatus]
      : undefined;
  const hintKey = needsConfig
    ? BADGE_HINT_KEY.needsConfig
    : runStatus === 'completedWithErrors'
      ? BADGE_HINT_KEY.completedWithErrors
      : undefined;
  return { badge, hintKey };
}
