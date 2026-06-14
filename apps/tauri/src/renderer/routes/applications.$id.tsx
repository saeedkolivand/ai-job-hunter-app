import { createFileRoute } from '@tanstack/react-router';

import { ApplicationDetailPage } from '@/features/applications/components/ApplicationDetailPage';

export const Route = createFileRoute('/applications/$id')({
  component: ApplicationDetailPage,
});
