import { createFileRoute } from '@tanstack/react-router';

import { AnalyzePage } from '@/features/analyze/components/AnalyzePage';

export const Route = createFileRoute('/analyze')({ component: AnalyzePage });
