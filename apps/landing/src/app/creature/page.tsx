import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { GoogleFonts } from '@/components/GoogleFonts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import { readContent } from '@/lib/content';

const FONTS =
  'https://fonts.googleapis.com/css2?family=Anton&family=Caveat:wght@600;700&family=Gloria+Hallelujah&family=Patrick+Hand&family=Space+Mono:wght@400;700&display=swap';

export const metadata: Metadata = {
  title: 'THE CREATURE — a hand-drawn doodle short film',
  description:
    'a doodle engineer accidentally summons a tiny recruiter creature. each application makes it bigger. a hand-drawn short film from the AI Job Hunter notebook. runtime ~2:40.',
};

export default function CreaturePage() {
  return (
    <>
      <GoogleFonts href={FONTS} gstatic={false} />
      <PageStyle css={readContent('creature', 'styles.css')} />
      <RawHtml html={readContent('creature', 'body.html')} />
      <ClientScripts srcs={['/scripts/creature-0.js', '/scripts/creature-1.js']} />
    </>
  );
}
