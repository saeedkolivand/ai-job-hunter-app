import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import { readContent } from '@/lib/content';

export const metadata: Metadata = {
  title: 'THE CREATURE — a hand-drawn doodle short film',
  description:
    'a doodle engineer accidentally summons a tiny recruiter creature. each application makes it bigger. a hand-drawn short film from the AI Job Hunter notebook. runtime ~2:40.',
};

export default function CreaturePage() {
  return (
    <>
      <PageStyle css={readContent('creature', 'styles.css')} />
      <RawHtml html={readContent('creature', 'body.html')} />
      <ClientScripts srcs={['/scripts/creature-0.js', '/scripts/creature-1.js']} />
    </>
  );
}
