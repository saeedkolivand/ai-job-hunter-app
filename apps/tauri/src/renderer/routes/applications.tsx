import { createFileRoute } from '@tanstack/react-router';

import { ApplicationsPage } from '@/features/applications/components/ApplicationsPage';

export const Route = createFileRoute('/applications')({ component: ApplicationsPage });
