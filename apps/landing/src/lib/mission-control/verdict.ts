// The hero verdict: one plain-language line synthesizing the whole-repo state
// from a handful of signals, in the site's voice. Pure + ordered by severity, so
// the highest-priority truth always wins. Humor is allowed; the tone drives the
// accent color.

export interface VerdictSignals {
  // Latest completed gating run on main failed / errored / was cancelled.
  gatingRed: boolean;
  // A gating run has run at all (false ⇒ we simply don't know yet).
  gatingKnown: boolean;
  criticalIssueCount: number;
  daysSinceRelease: number | null;
  openPrCount: number;
  stalePrCount: number;
  staleDays: number;
}

type VerdictTone = 'ok' | 'warn' | 'bad';

export interface Verdict {
  tone: VerdictTone;
  line: string;
  sub: string;
}

export function computeVerdict(s: VerdictSignals): Verdict {
  if (s.gatingKnown && s.gatingRed) {
    return {
      tone: 'bad',
      line: 'The build is red. Everything else is set dressing.',
      sub: 'Fix CI on main before touching anything else.',
    };
  }
  if (s.criticalIssueCount > 0) {
    return {
      tone: 'bad',
      line: `${s.criticalIssueCount} critical issue${s.criticalIssueCount === 1 ? '' : 's'} open. The robot is on fire.`,
      sub: 'Triage the critical label first.',
    };
  }
  if (s.daysSinceRelease !== null && s.daysSinceRelease > 21) {
    return {
      tone: 'warn',
      line: `Shipping has stalled — ${s.daysSinceRelease} days since the last release.`,
      sub: 'Green, but nothing has left the building in a while.',
    };
  }
  if (s.stalePrCount > 0) {
    return {
      tone: 'warn',
      line: `Green build, but ${s.stalePrCount} PR${s.stalePrCount === 1 ? '' : 's'} ${s.stalePrCount === 1 ? 'is' : 'are'} gathering dust.`,
      sub: `Open longer than ${s.staleDays} days with no merge.`,
    };
  }
  return {
    tone: 'ok',
    line: 'All green. Still no job, but all green.',
    sub: `${s.openPrCount} open PR${s.openPrCount === 1 ? '' : 's'} · CI passing · nothing on fire.`,
  };
}

// A conclusion string counts as "red" only if the run finished in a genuine
// failure state. Null (never ran) is not red — see `gatingKnown`. `success`,
// `neutral`, and `skipped` are success-equivalent for dependent checks per
// GitHub's semantics — and this repo's claude-review self-skips on concurrency,
// so treating `skipped` as red would falsely redden the verdict.
export function isRedConclusion(conclusion: string | null): boolean {
  return (
    conclusion !== null &&
    conclusion !== 'success' &&
    conclusion !== 'neutral' &&
    conclusion !== 'skipped'
  );
}
