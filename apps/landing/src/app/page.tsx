import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { GoogleFonts } from '@/components/GoogleFonts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import { readContent } from '@/lib/content';

const FONTS =
  'https://fonts.googleapis.com/css2?family=Anton&family=Caveat:wght@600;700&family=Gloria+Hallelujah&family=Patrick+Hand&family=Space+Mono:wght@400;700&display=swap';

export const metadata: Metadata = {
  title: 'AI Job Hunter — please hire him',
  description:
    'Covers 24 job boards (direct scrapers + Adzuna/JSearch aggregator), writes your cover letters, does everything but hit submit. A real desktop app. Also a cry for help.',
  alternates: { canonical: 'https://aijobhunter.app/' },
  verification: { google: 'kP-_YvYx7Q5rIN5F8DIwbG3-oKVXMjr9BlNa1holc0M' },
  openGraph: {
    title: 'IT DOES EVERYTHING ELSE.',
    description:
      'I sent 1,000 applications and got 0 replies. So I built a robot. It does everything but press send.',
    type: 'website',
    url: 'https://aijobhunter.app/',
    images: [
      {
        url: 'https://aijobhunter.app/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — IT DOES EVERYTHING ELSE. Covers 24 job boards, writes your cover letters, does everything but hit submit.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'IT DOES EVERYTHING ELSE.',
    description:
      'I sent 1,000 applications and got 0 replies. So I built a robot. It does everything but press send.',
    images: ['https://aijobhunter.app/og-card.jpg'],
  },
};

export default function HomePage() {
  return (
    <>
      <GoogleFonts href={FONTS} />
      <PageStyle css={readContent('home', 'styles.css')} />
      <RawHtml html={readContent('home', 'body.html')} />
      <ClientScripts srcs={['/scripts/home-0.js']} />
    </>
  );
}
