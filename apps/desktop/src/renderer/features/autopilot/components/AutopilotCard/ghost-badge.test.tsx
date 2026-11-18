/**
 * AutopilotCard — the ghost-job badge is mounted in each found-job row with that
 * job's own `ghostSignals` (the badge itself is covered by GhostBadge.test.tsx).
 * Shared mocks live in ./test-render.
 */

import { describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import type { AutopilotGhostSignals } from '@ajh/shared';

import { expandCard, makeAutopilot, makeJob, renderCard } from './test-render';

vi.mock('@/components/job/GhostBadge', () => ({
  GhostBadge: ({ signals }: { signals?: AutopilotGhostSignals }) =>
    signals ? <span data-testid="ghost-badge">open {signals.openDays}</span> : null,
}));

const SIGNALS: AutopilotGhostSignals = {
  firstSeenAt: 0,
  openDays: 74,
  repostCount: 0,
  openLong: true,
  reposted: false,
};

describe('AutopilotCard — ghost badge mount', () => {
  it('renders the badge for a found job that carries ghost signals', async () => {
    renderCard(
      makeAutopilot([{ ...makeJob('https://example.com/job/ghost'), ghostSignals: SIGNALS }])
    );
    await expandCard();
    expect(screen.getByTestId('ghost-badge')).toHaveTextContent('open 74');
  });

  it('renders no badge for a found job without ghost signals', async () => {
    renderCard(makeAutopilot([makeJob('https://example.com/job/plain')]));
    await expandCard();
    expect(screen.queryByTestId('ghost-badge')).not.toBeInTheDocument();
  });
});
