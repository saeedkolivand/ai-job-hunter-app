import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { CreatureBody } from '@/components/creature/CreatureBody';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

export const metadata: Metadata = {
  title: 'THE CREATURE — a hand-drawn doodle short film',
  description:
    'a doodle engineer accidentally summons a tiny recruiter creature. each application makes it bigger. a hand-drawn short film from the AI Job Hunter notebook. runtime ~2:40.',
};

export default function CreaturePage() {
  return (
    <>
      <PageStyle css={readStyle('marketing-tokens.css')} />
      <PageStyle css={readStyle('creature.css')} />
      <CreatureBody />
      <ClientScripts srcs={['/scripts/creature-0.js', '/scripts/creature-1.js']} />
    </>
  );
}
