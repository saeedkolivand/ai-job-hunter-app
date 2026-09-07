import type { Metadata, Viewport } from 'next';

import { PageStyle } from '@/components/PageStyle';
import { TermsBody } from '@/components/terms/TermsBody';
import { readStyle } from '@/lib/styles';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Terms of Use',
  description:
    'The terms for using the AI Job Hunter desktop app, browser extension and website. Apache-2.0, provided as is, no accounts — and you stay responsible for what you send to employers.',
  robots: { index: true, follow: true },
  alternates: { canonical: 'https://aijobhunter.app/terms' },
  openGraph: {
    title: 'AI Job Hunter — Terms of Use',
    description:
      'The terms for using the AI Job Hunter desktop app, browser extension and website. Apache-2.0, provided as is, no accounts — and you stay responsible for what you send to employers.',
    url: 'https://aijobhunter.app/terms',
    type: 'website',
    images: [
      {
        url: '/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — terms of use: Apache-2.0, provided as is, no accounts.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'AI Job Hunter — Terms of Use',
    description:
      'Apache-2.0, provided as is, no accounts. What you can expect from the software, and what it expects from you.',
  },
};

export const viewport: Viewport = { themeColor: '#f4ecdc' };

export default function TermsPage() {
  return (
    <>
      <PageStyle css={readStyle('marketing-tokens.css')} />
      <PageStyle css={readStyle('marketing-base.css')} />
      {/* privacy.css is a generic document-page stylesheet (contrast-checked
          link colour, .card/.flag/.note, focus-visible ring, reduced-motion
          block) despite the name — reused as-is rather than duplicated, the
          same way app/accessibility/page.tsx does. */}
      <PageStyle css={readStyle('privacy.css')} />
      <TermsBody />
    </>
  );
}
