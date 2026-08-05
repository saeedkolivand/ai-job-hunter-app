import type { Metadata } from 'next';

import { DocShell } from '@/components/DocShell';
import { PageStyle } from '@/components/PageStyle';
import { TechRadar } from '@/components/tech-radar/TechRadar';
import { readStyle } from '@/lib/styles';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Technology Radar',
  description:
    'What this app is actually built on, and why: a curated Adopt/Trial/Assess/Hold radar of the real stack — Tauri 2, React 19, Rust, Typst, Sentry, and more — with the reasoning behind each choice and links to the ADR where one exists. A CI check fails the build if an entry ever names a dependency that no longer exists.',
  alternates: { canonical: 'https://aijobhunter.app/tech-radar' },
  openGraph: {
    title: 'AI Job Hunter — Technology Radar',
    description:
      'Adopt, Trial, Assess, Hold — the real stack behind this app, with the reasoning behind every choice.',
    url: 'https://aijobhunter.app/tech-radar',
    type: 'website',
    images: [
      {
        url: '/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — a technology radar of the real stack, with the reasoning behind every choice.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'AI Job Hunter — Technology Radar',
    description: 'Adopt, Trial, Assess, Hold — the real stack, with the reasoning behind it.',
  },
  other: { 'theme-color': '#0d0f14' },
};

export default function TechRadarPage() {
  return (
    <>
      <PageStyle css={readStyle('tech-radar.css')} />
      <DocShell
        eyebrow="verified against source, not aspirational"
        title="Technology Radar"
        lede={
          <>
            Adopt, Trial, Assess, Hold — what this app runs on today, and why. Every dependency
            entry is checked in CI against the real <code>package.json</code>/
            <code>Cargo.toml</code> files on every push; an entry naming something removed from the
            stack fails the build instead of quietly going stale.
          </>
        }
      >
        <TechRadar />
      </DocShell>
    </>
  );
}
