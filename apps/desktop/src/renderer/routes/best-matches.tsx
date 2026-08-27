import { createFileRoute } from '@tanstack/react-router';

import { BestMatchesPage } from '@/features/best-matches/components/BestMatchesPage';

export const Route = createFileRoute('/best-matches')({ component: BestMatchesPage });
