import { createFileRoute } from '@tanstack/react-router';

import { ResumeBuilderPage } from '@/features/resume-builder/components/ResumeBuilderPage';

export const Route = createFileRoute('/build')({ component: ResumeBuilderPage });
