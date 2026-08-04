import type { Metadata } from 'next';

import { AgentFleet } from '@/components/agent-system/AgentFleet';
import { DocShell } from '@/components/DocShell';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

export const metadata: Metadata = {
  title: 'AI Job Hunter — The Agent Fleet',
  description:
    "How the AI Job Hunter repo's .claude/ agent system works: specialized agents, paired author + critic per domain, routed by area, run down an assembly line, reviewed independently, closed out by a steward.",
  alternates: { canonical: 'https://aijobhunter.app/agent-system' },
  openGraph: {
    title: 'AI Job Hunter — The Agent Fleet',
    description:
      'Paired author + critic per domain. The .claude/ system that builds and reviews this repo, explained.',
    url: 'https://aijobhunter.app/agent-system',
    type: 'website',
    images: [
      {
        url: '/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — the .claude/ agent fleet that builds and reviews this repo.',
      },
    ],
  },
  twitter: {
    card: 'summary',
    title: 'AI Job Hunter — The Agent Fleet',
    description:
      'Paired author + critic per domain. The .claude/ system that builds and reviews this repo.',
  },
  other: { 'theme-color': '#0d0f14' },
};

export default function AgentSystemPage() {
  return (
    <>
      <PageStyle css={readStyle('agent-system.css')} />
      <DocShell>
        <AgentFleet />
      </DocShell>
    </>
  );
}
