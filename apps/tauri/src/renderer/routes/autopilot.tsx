import { createFileRoute } from '@tanstack/react-router';

import { AutopilotPage } from '@/features/autopilot/components/AutopilotPage';

export const Route = createFileRoute('/autopilot')({ component: AutopilotPage });
