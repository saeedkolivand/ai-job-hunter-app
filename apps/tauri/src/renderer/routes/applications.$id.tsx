import { createFileRoute } from '@tanstack/react-router';

import { ApplicationDetailPage } from '@/features/applications/components/ApplicationDetailPage';

export const DETAIL_TABS = ['overview', 'timeline', 'brief', 'documents', 'interview'] as const;
export type DetailTab = (typeof DETAIL_TABS)[number];

/** Closed set of origins the Back button can return to (deep-links omit it). */
const FROMS = ['jobs', 'autopilot', 'applications'] as const;

export const Route = createFileRoute('/applications/$id')({
  // `?tab=<overview|timeline|brief|documents>` keeps the active detail tab in the
  // URL so it survives reloads / back-forward and is deep-linkable. Unknown values
  // fall back to `overview`. Optional so plain navigations (e.g. opening a row)
  // need not supply it — the page coalesces a missing value to `overview`.
  // `?from=<jobs|autopilot|applications>` records where this detail was opened from
  // so the Back button returns to that origin with an origin-aware label. Unknown /
  // absent values fall back to the Applications list (deep-links / notifications).
  validateSearch: (
    s: Record<string, unknown>
  ): { tab?: DetailTab; from?: 'jobs' | 'autopilot' | 'applications' } => ({
    tab: (DETAIL_TABS as readonly string[]).includes(s.tab as string)
      ? (s.tab as DetailTab)
      : undefined,
    from: FROMS.includes(s.from as (typeof FROMS)[number])
      ? (s.from as (typeof FROMS)[number])
      : undefined,
  }),
  component: ApplicationDetailPage,
});
