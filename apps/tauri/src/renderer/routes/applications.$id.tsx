import { createFileRoute } from '@tanstack/react-router';

import { ApplicationDetailPage } from '@/features/applications/components/ApplicationDetailPage';

export const DETAIL_TABS = ['overview', 'timeline', 'brief', 'documents'] as const;
export type DetailTab = (typeof DETAIL_TABS)[number];

export const Route = createFileRoute('/applications/$id')({
  // `?tab=<overview|timeline|brief|documents>` keeps the active detail tab in the
  // URL so it survives reloads / back-forward and is deep-linkable. Unknown values
  // fall back to `overview`. Optional so plain navigations (e.g. opening a row)
  // need not supply it — the page coalesces a missing value to `overview`.
  // `?from=autopilot` records that this detail was opened from the autopilot Apply
  // flow so the Back button returns there instead of the Applications list.
  validateSearch: (s: Record<string, unknown>): { tab?: DetailTab; from?: 'autopilot' } => ({
    tab: (DETAIL_TABS as readonly string[]).includes(s.tab as string)
      ? (s.tab as DetailTab)
      : undefined,
    from: s.from === 'autopilot' ? 'autopilot' : undefined,
  }),
  component: ApplicationDetailPage,
});
