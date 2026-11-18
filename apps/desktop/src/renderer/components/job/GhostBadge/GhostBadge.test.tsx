import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { AutopilotGhostSignals } from '@ajh/shared';

import { GhostBadge } from './index';

const SIGNALS: AutopilotGhostSignals = {
  firstSeenAt: 1_700_000_000_000,
  openDays: 74,
  repostCount: 3,
  openLong: false,
  reposted: false,
};

describe('GhostBadge', () => {
  it('renders nothing without signals or when no flag is set', () => {
    const { container, rerender } = render(<GhostBadge />);
    expect(container).toBeEmptyDOMElement();
    rerender(<GhostBadge signals={SIGNALS} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows how long a posting has been open, with a hint-not-verdict tooltip', () => {
    render(<GhostBadge signals={{ ...SIGNALS, openLong: true }} />);
    const label = screen.getByText('Open 74 days');
    expect(label).toBeInTheDocument();
    expect(label.closest('[title]')).toHaveAttribute(
      'title',
      expect.stringContaining('only a hint, not a verdict')
    );
    expect(screen.queryByText(/Reposted/)).not.toBeInTheDocument();
  });

  it('shows the repost count', () => {
    render(<GhostBadge signals={{ ...SIGNALS, reposted: true }} />);
    expect(screen.getByText('Reposted 3×')).toBeInTheDocument();
    expect(screen.queryByText(/Open 74/)).not.toBeInTheDocument();
  });

  it('shows both chips when both flags are set', () => {
    render(<GhostBadge signals={{ ...SIGNALS, openLong: true, reposted: true }} />);
    expect(screen.getByText('Open 74 days')).toBeInTheDocument();
    expect(screen.getByText('Reposted 3×')).toBeInTheDocument();
  });
});
