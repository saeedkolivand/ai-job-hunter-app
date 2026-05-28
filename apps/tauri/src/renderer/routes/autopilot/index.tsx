import { createFileRoute } from '@tanstack/react-router';

import { AutopilotPage } from './components/AutopilotPage';

export const Route = createFileRoute('/autopilot/')({ component: AutopilotPage });
