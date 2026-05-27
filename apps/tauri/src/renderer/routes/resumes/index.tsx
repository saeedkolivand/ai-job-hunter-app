import { createFileRoute } from '@tanstack/react-router';

import { ResumesPage } from './resumes/components/ResumesPage';

export const Route = createFileRoute('/resumes')({ component: ResumesPage });
