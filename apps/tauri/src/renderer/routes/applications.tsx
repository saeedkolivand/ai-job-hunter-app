import { createFileRoute } from '@tanstack/react-router';

import { ApplicationsPage } from '@/features/applications/components/ApplicationsPage';

export const Route = createFileRoute('/applications')({
  // `?highlight=<applicationId>` deep-links from a notification's "View" to a
  // just-imported application: expand its stage section + flash the row once.
  // The page consumes it once, then clears it.
  validateSearch: (s: Record<string, unknown>): { highlight?: string } => ({
    highlight: typeof s.highlight === 'string' ? s.highlight : undefined,
  }),
  component: ApplicationsPage,
});
