import type { Metadata } from 'next';

import { DocShell } from '@/components/DocShell';
import { GoogleFonts } from '@/components/GoogleFonts';
import { CspMeta } from '@/components/mission-control/CspMeta';
import { MissionControl } from '@/components/mission-control/MissionControl';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

const FONTS =
  'https://fonts.googleapis.com/css2?family=Caveat:wght@600;700&family=Space+Grotesk:wght@400;500;700&family=Space+Mono:wght@400;700&display=swap';

// Internal ops dashboard on the marketing domain — keep it out of search, like
// the ci-dashboard it replaces.
export const metadata: Metadata = {
  title: 'AI Job Hunter — Mission Control',
  description:
    'A verdict-first, full-repo dashboard: delivery (DORA-lite), work, quality (CHAOSS), and community — live from the GitHub API, client-side only.',
  robots: { index: false, follow: false },
  other: { 'theme-color': '#0d0f14' },
};

export default function MissionControlPage() {
  return (
    <>
      <CspMeta />
      <GoogleFonts href={FONTS} />
      <PageStyle css={readStyle('mission-control.css')} />
      <DocShell
        eyebrow="the whole repo, one screen"
        title="Mission Control"
        lede={
          <>
            Everything the robot&rsquo;s repo is doing right now — delivery, open work, quality, and
            community — read straight from the GitHub API in your browser. Sign in with a token for
            the safe tier of write actions.
          </>
        }
        wide
      >
        <MissionControl />
      </DocShell>
    </>
  );
}
