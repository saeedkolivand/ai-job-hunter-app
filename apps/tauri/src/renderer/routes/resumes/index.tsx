import { createFileRoute } from '@tanstack/react-router';

import { ResumesPage } from './components/ResumesPage';

export const Route = createFileRoute('/resumes')({ component: ResumesPage });
