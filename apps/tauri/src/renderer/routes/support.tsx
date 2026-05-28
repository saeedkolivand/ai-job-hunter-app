import { createFileRoute } from '@tanstack/react-router';

import { SupportPage } from '@/features/support/components/SupportPage';

export const Route = createFileRoute('/support')({ component: SupportPage });
