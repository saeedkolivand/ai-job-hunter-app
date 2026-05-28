import { createFileRoute } from '@tanstack/react-router';

import { ResumesPage } from '@/features/resumes/components/ResumesPage';

export const Route = createFileRoute('/resumes')({ component: ResumesPage });
