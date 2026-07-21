import type { Metadata, Viewport } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { Fonts } from '@/components/Fonts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import { readContent } from '@/lib/content';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Privacy Policy',
  description:
    'How the AI Job Hunter desktop app and browser extension handle your data. No accounts, no analytics, no telemetry. Local-first.',
  robots: { index: true, follow: true },
  alternates: { canonical: 'https://aijobhunter.app/privacy' },
  openGraph: {
    title: 'AI Job Hunter — Privacy Policy',
    description:
      'How the AI Job Hunter desktop app and browser extension handle your data. No accounts, no analytics, no telemetry. Local-first.',
    url: 'https://aijobhunter.app/privacy',
    type: 'website',
  },
  twitter: {
    card: 'summary',
    title: 'AI Job Hunter — Privacy Policy',
    description: 'No accounts, no analytics, no telemetry. Local-first. How your data is handled.',
  },
};

export const viewport: Viewport = { themeColor: '#f4ecdc' };

export default function PrivacyPage() {
  return (
    <>
      <Fonts />
      <PageStyle css={readContent('privacy', 'styles.css')} />
      <RawHtml html={readContent('privacy', 'body.html')} />
      <ClientScripts srcs={['/scripts/privacy-0.js']} />
    </>
  );
}
