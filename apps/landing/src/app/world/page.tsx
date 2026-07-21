import type { Metadata } from 'next';

import { Fonts } from '@/components/Fonts';

import { WorldClient } from './WorldClient';

export const metadata: Metadata = {
  title: 'AI Job Hunter — fly through the world',
  description:
    'Scroll through a papercraft diorama of one very bad job hunt — the slump, the doomscroll, the robot he built to fix it, and the offer at the end. No cuts, just scroll.',
  alternates: { canonical: 'https://aijobhunter.app/world' },
  openGraph: {
    title: 'FLY THROUGH THE WORLD.',
    description:
      'A paper world built from one very bad job hunt. Scroll to fly through the slump, the doomscroll, the robot, and the offer at the end.',
    type: 'website',
    url: 'https://aijobhunter.app/world',
    images: [
      {
        url: 'https://aijobhunter.app/og-card.jpg',
        width: 1200,
        height: 630,
        alt: 'AI Job Hunter — a papercraft world you scroll through, from the slump to the offer.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'FLY THROUGH THE WORLD.',
    description:
      'A paper world built from one very bad job hunt. Scroll to fly through the slump, the doomscroll, the robot, and the offer at the end.',
    images: ['https://aijobhunter.app/og-card.jpg'],
  },
};

export default function WorldPage() {
  return (
    <>
      <Fonts />
      <WorldClient />
    </>
  );
}
