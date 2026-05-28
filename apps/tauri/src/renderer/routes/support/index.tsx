import { createFileRoute } from '@tanstack/react-router';

import { SupportPage } from './components/SupportPage';

export const Route = createFileRoute('/support/')({ component: SupportPage });
