import { createFileRoute } from '@tanstack/react-router';

import { AIWorkspace } from '@/features/ai-workspace/components/AIWorkspace';

export const Route = createFileRoute('/ai')({ component: AIWorkspace });
