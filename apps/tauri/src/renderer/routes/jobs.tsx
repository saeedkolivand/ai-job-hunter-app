import { createFileRoute } from '@tanstack/react-router';

import { JobsPage } from '@/features/jobs/components/JobsPage';

export const Route = createFileRoute('/jobs')({ component: JobsPage });
