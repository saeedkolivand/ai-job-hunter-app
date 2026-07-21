import type { Metadata } from 'next';

import { DocShell } from '@/components/DocShell';
import { Fonts } from '@/components/Fonts';
import { CspMeta } from '@/components/mission-control/CspMeta';
import { MissionControl } from '@/components/mission-control/MissionControl';
import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

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
      <Fonts />
      <PageStyle css={readStyle('mission-control.css')} />
      <DocShell
        eyebrow="the whole repo, one screen"
        title="Mission Control"
        lede={
          <>
            Everything the robot&rsquo;s repo is doing right now — delivery, open work, quality, and
            community — served in your browser from a nightly snapshot, with the live GitHub API as
            the fallback. Sign in with a token to unlock the safe tier of write actions — and a
            higher rate limit whenever reads fall back to the live API.
          </>
        }
        wide
      >
        <MissionControl />
      </DocShell>
    </>
  );
}
