/**
 * BestMatchRow — action wiring (View/Save/Apply/Dismiss call back with the
 * row's own match) + the mixed-score-scale abbreviation, mirroring
 * AutopilotCard's own `mixedScoreSources` treatment this row is asked to
 * copy rather than reinvent.
 */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AutopilotBestMatch } from '@ajh/shared';

import type * as MatchBandModule from '@/lib/match-band';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  Button: ({
    children,
    onClick,
    title,
  }: {
    children?: ReactNode;
    onClick?: () => void;
    title?: string;
  }) => (
    <button type="button" onClick={onClick} title={title}>
      {children}
    </button>
  ),
  Tag: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

vi.mock('lucide-react', () => ({
  Bookmark: () => null,
  ExternalLink: () => null,
  Sparkles: () => null,
  Wand2: () => null,
  X: () => null,
}));

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
  useRouterState: () => '/best-matches',
}));

vi.mock('@/store/session-store', () => ({
  useSessionStore: () => vi.fn(),
}));

vi.mock('@/components/job/AgencyChip', () => ({ AgencyChip: () => null }));
vi.mock('@/components/job/ClusterSourceChips', () => ({ ClusterSourceChips: () => null }));
vi.mock('@/lib/trust-badge', () => ({ TrustBadge: () => null }));
vi.mock('@/lib/match-band', async (importOriginal) => {
  const actual = await importOriginal<typeof MatchBandModule>();
  return {
    ...actual,
    MatchBand: () => <span data-testid="match-band" />,
  };
});
vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '2h ago',
}));

import { BestMatchRow } from './index';

function makeMatch(overrides: Partial<AutopilotBestMatch> = {}): AutopilotBestMatch {
  return {
    key: 'k1',
    title: 'Backend Engineer',
    company: 'Acme',
    url: 'https://example.com/job/1',
    location: 'Berlin',
    score: 80,
    scoreSource: 'combined',
    foundAt: 0,
    sources: [],
    ...overrides,
  };
}

const noop = { onView: vi.fn(), onSave: vi.fn(), onApply: vi.fn(), onDismiss: vi.fn() };

describe('BestMatchRow — action wiring', () => {
  it("View calls onView with this row's match", async () => {
    const user = userEvent.setup();
    const onView = vi.fn();
    const match = makeMatch();
    render(<BestMatchRow match={match} mixedScoreSources={false} {...noop} onView={onView} />);

    await user.click(screen.getByTitle('bestMatches.row.view'));
    expect(onView).toHaveBeenCalledWith(match);
  });

  it("Save calls onSave with this row's match", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const match = makeMatch();
    render(<BestMatchRow match={match} mixedScoreSources={false} {...noop} onSave={onSave} />);

    await user.click(screen.getByTitle('bestMatches.row.save'));
    expect(onSave).toHaveBeenCalledWith(match);
  });

  it("Apply calls onApply with this row's match", async () => {
    const user = userEvent.setup();
    const onApply = vi.fn();
    const match = makeMatch();
    render(<BestMatchRow match={match} mixedScoreSources={false} {...noop} onApply={onApply} />);

    await user.click(screen.getByTitle('bestMatches.row.apply'));
    expect(onApply).toHaveBeenCalledWith(match);
  });

  it("Dismiss calls onDismiss with this row's match", async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    const match = makeMatch();
    render(
      <BestMatchRow match={match} mixedScoreSources={false} {...noop} onDismiss={onDismiss} />
    );

    await user.click(screen.getByTitle('bestMatches.row.dismiss'));
    expect(onDismiss).toHaveBeenCalledWith(match);
  });
});

describe('BestMatchRow — mixed score sources', () => {
  it('shows the scale abbreviation when the rendered list mixes both scales', () => {
    render(
      <BestMatchRow match={makeMatch({ scoreSource: 'combined' })} mixedScoreSources {...noop} />
    );
    expect(screen.getByText('autopilot.scoreAbbr.combined')).toBeInTheDocument();
  });

  it('shows nothing extra when the list does not mix scales', () => {
    render(
      <BestMatchRow
        match={makeMatch({ scoreSource: 'combined' })}
        mixedScoreSources={false}
        {...noop}
      />
    );
    expect(screen.queryByText('autopilot.scoreAbbr.combined')).not.toBeInTheDocument();
  });
});

describe('BestMatchRow — plain-text rendering (ADR-010, untrusted content)', () => {
  it('renders scraped title/company as plain text, not HTML', () => {
    const match = makeMatch({ title: '<img src=x onerror=alert(1)>', company: 'Acme & Co' });
    render(<BestMatchRow match={match} mixedScoreSources={false} {...noop} />);

    expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeInTheDocument();
    expect(document.querySelector('img')).not.toBeInTheDocument();
  });
});
