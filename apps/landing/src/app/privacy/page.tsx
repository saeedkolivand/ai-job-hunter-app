import type { Metadata, Viewport } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { PageStyle } from '@/components/PageStyle';
import { PrivacyBody } from '@/components/privacy/PrivacyBody';
import { readStyle } from '@/lib/styles';

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
      <PageStyle css={readStyle('marketing-tokens.css')} />
      <PageStyle css={readStyle('marketing-base.css')} />
      <PageStyle css={readStyle('privacy.css')} />
      <PrivacyBody />
      <ClientScripts srcs={['/scripts/privacy-0.js']} />
    </>
  );
}
