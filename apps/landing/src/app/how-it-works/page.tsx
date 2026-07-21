import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { GoogleFonts } from '@/components/GoogleFonts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import { readContent } from '@/lib/content';
import { readStyle } from '@/lib/styles';

// Docs-tier reskin: load the shell fonts (Space Grotesk / Caveat / Space Mono) and
// layer the shell palette over the original slate styles (docs-tokens → original
// styles.css → shell override; last :root wins). The copy, the DOM the 55 KB
// pipeline player queries, and the console egg are all unchanged.
const FONTS =
  'https://fonts.googleapis.com/css2?family=Caveat:wght@600;700&family=Space+Grotesk:wght@400;500;700&family=Space+Mono:wght@400;700&display=swap';

export const metadata: Metadata = {
  title: 'AI Job Hunter — How It Works (End to End)',
};

export default function HowItWorksPage() {
  return (
    <>
      <GoogleFonts href={FONTS} />
      <PageStyle css={readStyle('docs-tokens.css')} />
      <PageStyle css={readContent('how-it-works', 'styles.css')} />
      <PageStyle css={readStyle('how-it-works-shell.css')} />
      <RawHtml html={readContent('how-it-works', 'body.html')} />
      <ClientScripts srcs={['/scripts/how-it-works-0.js', '/scripts/how-it-works-1.js']} />
    </>
  );
}
