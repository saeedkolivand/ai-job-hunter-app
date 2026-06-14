import { createFileRoute } from '@tanstack/react-router';

import { ApplyPageRoute } from '@/features/autopilot/components/ApplyPageRoute';

export const Route = createFileRoute('/autopilot/apply')({
  component: ApplyPageRoute,
});
