import type { AutopilotFoundJob } from '@ajh/shared';

import { matchBandDescriptionKey, scoreTier } from '@/lib/match-band';

/**
 * Every metric a found-job score can be rendered as (ADR-020 addendum).
 *
 * A runtime tuple, not just a type: each variant owns an
 * `autopilot.scoreLabel.*` / `autopilot.scoreAbbr.*` key built by template
 * string, which TypeScript cannot check. Exported so the i18n test enumerates
 * the REAL set — restating the two strings there is how a third variant would
 * ship with no localized label.
 */
export const SCORE_VARIANTS = ['coverage', 'combined'] as const;
export type ScoreVariant = (typeof SCORE_VARIANTS)[number];

/**
 * The band variant a found job's score should render as. `'combined'` ONLY when
 * the backend says that job's score came from the semantic+ATS kernel — the
 * two metrics have different meanings AND different tier cut points, so showing
 * a keyword number on the combined scale (or vice versa) mislabels it. A job
 * that degraded back to keyword-only mid-run reports `'keyword'` and is
 * rendered as such (ADR-020 addendum).
 */
export function scoreVariant(job: AutopilotFoundJob): ScoreVariant {
  return job.scoreSource === 'combined' ? 'combined' : 'coverage';
}

/**
 * Hover/screen-reader copy for a found job's score: WHICH metric it is
 * ("Keyword Coverage %" vs "Match %" — ADR-020 asked for this distinction and
 * it had never been surfaced), what the tier means, and — when provisional —
 * why the number is only an estimate.
 *
 * All three, not one of them. They answer different questions: the metric name
 * says what is being measured, the tier description says what "High" is
 * claiming, and `provisionalScoreHint` says how much to trust the number behind
 * it — so dropping any leaves a real gap. Composed here rather than inside
 * `MatchBand` because this wrapper owns the `title` and the sr-only span;
 * letting the band render its own would put a second `title` inside this one
 * (the inner wins on hover over the badge, hiding the rest) and announce twice.
 */
export function scoreDetail(t: (key: string) => string, job: AutopilotFoundJob): string {
  const variant = scoreVariant(job);
  const label = t(`autopilot.scoreLabel.${variant}`);
  const tier = t(matchBandDescriptionKey(scoreTier(job.score ?? 0, variant).key, variant));
  const provisional = job.scoreProvisional ? ` ${t('autopilot.provisionalScoreHint')}` : '';
  return `${label}: ${tier}${provisional}`;
}

/** A card's found-jobs sort choice — `'relevance'` is the stored rank order. */
export type FoundJobsSortBy = 'relevance' | 'newest' | 'oldest';

/**
 * View-side date sort for a card's found jobs. NEVER mutates `jobs` — the
 * persisted `Autopilot.foundJobs` order feeds AI-note recipient selection on
 * the backend (ADR-020), so this always sorts a fresh copy and returns it.
 * Exported so the mutation invariant + banding are directly unit-testable
 * without going through the component memo.
 *
 * Mirrors JobsPage's postedAt sort (JobsPage/index.tsx:288-300): a dated band
 * (sorted by `postedAt`) leads, an undated band trails instead of
 * interleaving, and a `url` tiebreak keeps equal-timestamp (or all-undated)
 * rows in a stable order across renders. Found jobs carry no `id` — `url` is
 * already the row's own render key and is unique per posting.
 * Deliberately NO `foundAt` (capture-time) fallback for undated rows — see
 * JobsPage:278-284: a just-scraped stale posting would otherwise jump above a
 * genuinely-recent one.
 */
export function sortFoundJobsByDate(
  jobs: AutopilotFoundJob[],
  sortBy: Exclude<FoundJobsSortBy, 'relevance'>
): AutopilotFoundJob[] {
  const byUrl = (x: AutopilotFoundJob, y: AutopilotFoundJob) =>
    x.url < y.url ? -1 : x.url > y.url ? 1 : 0;
  return [...jobs].sort((a, b) => {
    if (typeof a.postedAt !== 'number' || typeof b.postedAt !== 'number') {
      if (typeof a.postedAt === 'number') return -1; // a dated, b undated
      if (typeof b.postedAt === 'number') return 1; // b dated, a undated
      return byUrl(a, b); // both undated
    }
    const cmp = sortBy === 'oldest' ? a.postedAt - b.postedAt : b.postedAt - a.postedAt;
    return cmp || byUrl(a, b);
  });
}
