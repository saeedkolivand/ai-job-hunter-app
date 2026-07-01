import { createFileRoute } from '@tanstack/react-router';

import { AIGeneratePage } from '@/features/ai-generate/components/AIGeneratePage';

export const Route = createFileRoute('/ai-generate')({ component: AIGeneratePage });
