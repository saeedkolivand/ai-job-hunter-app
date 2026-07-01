import { createFileRoute } from '@tanstack/react-router';

import { DocumentsPage } from '@/features/documents/components/DocumentsPage';

export const Route = createFileRoute('/documents')({ component: DocumentsPage });
