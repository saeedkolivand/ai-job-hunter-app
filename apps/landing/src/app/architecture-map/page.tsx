import type { Metadata } from 'next';

import { ArchitectureMap } from '@/components/architecture-map/ArchitectureMap';
import { DocShell } from '@/components/DocShell';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Architecture Map',
  description:
    'An interactive map of the AI Job Hunter architecture: the local-first Tauri 2 monorepo, its renderer / service-hook / prompt / contract / Rust-command / domain layers, the scraper and AI-provider registries, and the real files behind every node. Every box maps to source; a drift checker fails CI if it lies.',
  alternates: { canonical: 'https://aijobhunter.app/architecture-map' },
  openGraph: {
    title: 'AI Job Hunter — Architecture Map',
    description:
      'Interactive architecture map of the local-first Tauri 2 monorepo — every node maps to a real file.',
    url: 'https://aijobhunter.app/architecture-map',
    type: 'website',
  },
  twitter: {
    card: 'summary',
    title: 'AI Job Hunter — Architecture Map',
    description: 'Interactive architecture map — every node maps to a real file.',
  },
  other: { 'theme-color': '#0d0f14' },
};

export default function ArchitectureMapPage() {
  return (
    <>
      <PageStyle css={readStyle('architecture-map.css')} />
      <DocShell wide>
        <ArchitectureMap />
      </DocShell>
    </>
  );
}
