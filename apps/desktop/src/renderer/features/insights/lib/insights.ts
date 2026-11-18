import type { InsightsDimension, InsightsOutcomeCounts, InsightsOutcomeGroup } from '@ajh/shared';

/** Sections in display order. */
export const DIMENSIONS = [
  'board',
  'tailored',
  'coverLetter',
  'targetLanguage',
  'month',
  'template',
] as const satisfies readonly InsightsDimension[];

/**
 * Fewest decided applications before the headline shows a percentage. Mirrors the
 * backend's per-group `MIN_GROUP_SAMPLE`, so the headline is no braver than a group.
 */
export const HEADLINE_MIN_DECIDED = 5;

/** Applications with a known outcome (pending ones are excluded from every rate). */
export const decidedOf = (c: InsightsOutcomeCounts) => c.positiveReply + c.rejected + c.noReply;

export interface Headline {
  counts: InsightsOutcomeCounts;
  decided: number;
  /** 0..1, or `null` below {@link HEADLINE_MIN_DECIDED}. */
  rate: number | null;
}

/**
 * Overall totals. Every application lands in exactly one `month` group, so summing
 * that dimension counts each application once.
 */
export function headlineOf(groups: readonly InsightsOutcomeGroup[]): Headline {
  const counts: InsightsOutcomeCounts = { positiveReply: 0, rejected: 0, noReply: 0, pending: 0 };
  for (const g of groups) {
    if (g.dimension !== 'month') continue;
    counts.positiveReply += g.counts.positiveReply;
    counts.rejected += g.counts.rejected;
    counts.noReply += g.counts.noReply;
    counts.pending += g.counts.pending;
  }
  const decided = decidedOf(counts);
  return {
    counts,
    decided,
    rate: decided >= HEADLINE_MIN_DECIDED ? counts.positiveReply / decided : null,
  };
}

/** The backend's marker for "value not recorded" (no generation / no export yet). */
export const UNKNOWN_KEY = 'unknown';

/** One dimension's groups: months stay chronological, the rest biggest first, unknown always last. */
export function groupsOf(
  groups: readonly InsightsOutcomeGroup[],
  dimension: InsightsDimension
): InsightsOutcomeGroup[] {
  const rows = groups.filter((g) => g.dimension === dimension);
  if (dimension === 'month') return rows.sort((a, b) => a.key.localeCompare(b.key));
  const size = (g: InsightsOutcomeGroup) => decidedOf(g.counts) + g.counts.pending;
  const unknownLast = (g: InsightsOutcomeGroup) => (g.key === UNKNOWN_KEY ? 1 : 0);
  return rows.sort(
    (a, b) => unknownLast(a) - unknownLast(b) || size(b) - size(a) || a.key.localeCompare(b.key)
  );
}
