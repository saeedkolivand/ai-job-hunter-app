import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { HowItWorksBody } from '@/components/how-it-works/HowItWorksBody';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

// Docs-tier reskin: the self-hosted shell fonts (Space Grotesk / Caveat / Space
// Mono) layered with the shell palette over the original slate styles (docs-tokens
// → original styles.css → shell override; last :root wins). The copy, the DOM the
// 55 KB pipeline player queries, and the console egg are all unchanged.
export const metadata: Metadata = {
  title: 'AI Job Hunter — How It Works (End to End)',
};

export default function HowItWorksPage() {
  return (
    <>
      <PageStyle css={readStyle('docs-tokens.css')} />
      <PageStyle css={readStyle('how-it-works.css')} />
      <PageStyle css={readStyle('how-it-works-shell.css')} />
      <HowItWorksBody />
      <ClientScripts srcs={['/scripts/how-it-works-0.js', '/scripts/how-it-works-1.js']} />
    </>
  );
}
