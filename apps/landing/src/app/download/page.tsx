import type { Metadata, Viewport } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { DownloadBody } from '@/components/download/DownloadBody';
import { DownloadCounts } from '@/components/DownloadCounts';
import { DownloadFreshness } from '@/components/DownloadFreshness';
import { PageStyle } from '@/components/PageStyle';
import versionData from '@/data/version.json';
import { readStyle } from '@/lib/styles';
import type { VersionData } from '@/lib/version';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Download for macOS, Windows, Linux',
  description:
    'Download the AI Job Hunter desktop app for macOS, Windows, and Linux. A local-first, AI-native job-hunting assistant. Free for personal use, source-available, unsigned, and unemployed.',
  robots: { index: true, follow: true },
  alternates: { canonical: 'https://aijobhunter.app/download' },
  openGraph: {
    title: 'TAKE THE APP.',
    description:
      'Get the desktop app for macOS, Windows, and Linux. Plus the browser extension. Local-first. No accounts. Still unemployed.',
    url: 'https://aijobhunter.app/download',
    type: 'website',
    images: [
      {
        url: '/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — download the desktop app for macOS, Windows, and Linux.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'TAKE THE APP.',
    description: 'Get the desktop app for macOS, Windows, and Linux. Plus the browser extension.',
  },
};

export const viewport: Viewport = { themeColor: '#f4ecdc' };

// Bake the download cards from src/data/version.json at build (updated on release
// by scripts/sync-download-page.cjs); DownloadFreshness swaps to a newer GitHub
// release at runtime by mutating `#downloads-block` (rendered by DownloadCards,
// inside DownloadBody's `.platforms` div) in place.
export default function DownloadPage() {
  const data = versionData as VersionData;

  return (
    <>
      <PageStyle css={readStyle('marketing-tokens.css')} />
      <PageStyle css={readStyle('marketing-base.css')} />
      <PageStyle css={readStyle('download.css')} />
      <DownloadBody version={data.version} installers={data.installers} />
      <DownloadFreshness baked={data.version} />
      <DownloadCounts />
      <ClientScripts srcs={['/scripts/download-0.js']} />
    </>
  );
}
