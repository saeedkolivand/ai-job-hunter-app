import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { HowItWorksBody } from '@/components/how-it-works/HowItWorksBody';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

// Docs-tier reskin: the self-hosted shell fonts (Space Grotesk / Caveat / Space
// Mono) layered with the shell palette over the original slate styles (docs-tokens
// → original styles.css → shell override; last :root wins). The copy, the DOM the
// 55 KB pipeline player queries, and the console egg are all unchanged.
export const metadata: Metadata = {
  title: 'AI Job Hunter — How It Works, Step by Step',
  description:
    'A layer-by-layer, click-through map of the AI Job Hunter pipeline — React UI, Rust core, AI models, scrapers, and disk — from job board to finished application.',
  alternates: { canonical: 'https://aijobhunter.app/how-it-works' },
  openGraph: {
    title: 'SEE THE WHOLE PIPELINE.',
    description:
      'Click through every layer of the AI Job Hunter pipeline, from job board to finished application.',
    url: 'https://aijobhunter.app/how-it-works',
    type: 'website',
    images: [
      {
        url: '/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — How It Works, click-through architecture of the whole pipeline.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'SEE THE WHOLE PIPELINE.',
    description: 'Click through every layer, from job board to finished application.',
  },
};

export default function HowItWorksPage() {
  return (
    <>
      <PageStyle css={readStyle('docs-tokens.css')} />
      <PageStyle css={readStyle('how-it-works.css')} />
      <PageStyle css={readStyle('how-it-works-shell.css')} />
      <HowItWorksBody />
      <ClientScripts srcs={['/scripts/how-it-works-0.js', '/scripts/how-it-works-1.js']} />
    </>
  );
}
