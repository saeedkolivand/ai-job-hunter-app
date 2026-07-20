import type { Metadata, Viewport } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { DownloadFreshness } from '@/components/DownloadFreshness';
import { GoogleFonts } from '@/components/GoogleFonts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import versionData from '@/data/version.json';
import { readContent } from '@/lib/content';
import { buildDownloadsHtml } from '@/lib/downloads';
import type { VersionData } from '@/lib/version';

const FONTS =
  'https://fonts.googleapis.com/css2?family=Anton&family=Gloria+Hallelujah&family=Patrick+Hand&family=Space+Mono:wght@400;700&display=swap';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Download',
  description:
    'Download the AI Job Hunter desktop app for macOS, Windows, and Linux. A local-first, AI-native job-hunting assistant. Free for personal use, source-available, unsigned, and unemployed.',
  robots: { index: true, follow: true },
  alternates: { canonical: 'https://aijobhunter.app/download' },
  openGraph: {
    title: 'AI Job Hunter — Download',
    description:
      'Get the desktop app for macOS, Windows, and Linux. Plus the browser extension. Local-first. No accounts. Still unemployed.',
    url: 'https://aijobhunter.app/download',
    type: 'website',
  },
  twitter: {
    card: 'summary',
    title: 'AI Job Hunter — Download',
    description: 'Get the desktop app for macOS, Windows, and Linux. Plus the browser extension.',
  },
};

export const viewport: Viewport = { themeColor: '#f4ecdc' };

// Bake the download cards from src/data/version.json at build (updated on release
// by scripts/sync-download-page.cjs); DownloadFreshness swaps to a newer GitHub
// release at runtime. The block is wrapped in `#downloads-block` (display:contents
// via .platforms flow) so it stays a flex item of `.platforms`.
export default function DownloadPage() {
  const data = versionData as VersionData;
  const block = `<div id="downloads-block" style="display:contents">${buildDownloadsHtml(
    data.version,
    data.installers
  )}</div>`;
  const body = readContent('download', 'body.html').replace(
    /<!-- downloads:start[\s\S]*?downloads:end -->/,
    block
  );

  return (
    <>
      <GoogleFonts href={FONTS} />
      <PageStyle css={readContent('download', 'styles.css')} />
      <RawHtml html={body} />
      <DownloadFreshness baked={data.version} />
      <ClientScripts srcs={['/scripts/download-0.js']} />
    </>
  );
}
