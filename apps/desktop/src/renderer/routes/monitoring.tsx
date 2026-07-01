import { createFileRoute } from '@tanstack/react-router';

import { MonitoringPage } from '@/features/monitoring/components/MonitoringPage';

export const Route = createFileRoute('/monitoring')({ component: MonitoringPage });
