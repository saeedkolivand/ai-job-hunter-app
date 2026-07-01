import { createFileRoute } from '@tanstack/react-router';

import { AutopilotPage } from '@/features/autopilot/components/AutopilotPage';

export const Route = createFileRoute('/autopilot/')({
  // `?focus=<autopilotId>` deep-links from a notification's "View" to a specific
  // autopilot's found-jobs panel. The page consumes it once, then clears it.
  validateSearch: (s: Record<string, unknown>): { focus?: string } => ({
    focus: typeof s.focus === 'string' ? s.focus : undefined,
  }),
  component: AutopilotPage,
});
