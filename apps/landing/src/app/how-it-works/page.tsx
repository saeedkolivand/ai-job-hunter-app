import type { Metadata } from 'next';

import { ClientScripts } from '@/components/ClientScripts';
import { PageStyle } from '@/components/PageStyle';
import { RawHtml } from '@/components/RawHtml';
import { readContent } from '@/lib/content';

// This page uses system fonts only (no Google-Fonts <link> in the original head).
export const metadata: Metadata = {
  title: 'AI Job Hunter — How It Works (End to End)',
};

export default function HowItWorksPage() {
  return (
    <>
      <PageStyle css={readContent('how-it-works', 'styles.css')} />
      <RawHtml html={readContent('how-it-works', 'body.html')} />
      <ClientScripts srcs={['/scripts/how-it-works-0.js', '/scripts/how-it-works-1.js']} />
    </>
  );
}
