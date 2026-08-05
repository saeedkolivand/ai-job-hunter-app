import type { Metadata, Viewport } from 'next';

import { AccessibilityBody } from '@/components/accessibility/AccessibilityBody';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

export const metadata: Metadata = {
  title: 'AI Job Hunter — Accessibility Statement',
  description:
    'The WCAG 2.2 AA conformance status of the AI Job Hunter landing site, desktop app and browser extension, known barriers, and how to report one.',
  robots: { index: true, follow: true },
  alternates: { canonical: 'https://aijobhunter.app/accessibility' },
  openGraph: {
    title: 'AI Job Hunter — Accessibility Statement',
    description:
      'The WCAG 2.2 AA conformance status of the AI Job Hunter landing site, desktop app and browser extension, known barriers, and how to report one.',
    url: 'https://aijobhunter.app/accessibility',
    type: 'website',
    images: [
      {
        url: '/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — WCAG 2.2 AA accessibility statement and known barriers.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'AI Job Hunter — Accessibility Statement',
    description: 'WCAG 2.2 AA conformance status, known barriers, and how to report one.',
  },
};

export const viewport: Viewport = { themeColor: '#f4ecdc' };

export default function AccessibilityPage() {
  return (
    <>
      <PageStyle css={readStyle('marketing-tokens.css')} />
      <PageStyle css={readStyle('marketing-base.css')} />
      {/* privacy.css is a generic document-page stylesheet (contrast-checked
          link colour, .card/.flag/.note, focus-visible ring, reduced-motion
          block) despite the name — reused as-is rather than duplicated. */}
      <PageStyle css={readStyle('privacy.css')} />
      <AccessibilityBody />
    </>
  );
}
