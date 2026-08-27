/**
 * BestMatchesPreview — renders nothing when there are no qualifying matches;
 * caps at 3 rows and links to the full `/best-matches` list otherwise.
 */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { AutopilotBestMatch } from '@ajh/shared';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
  }),
}));

vi.mock('lucide-react', () => ({ Sparkles: () => null }));

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children, to }: { children: ReactNode; to: string }) => <a href={to}>{children}</a>,
}));

vi.mock('@/components/job/BestMatchRow', () => ({
  BestMatchRow: ({ match }: { match: AutopilotBestMatch }) => (
    <div data-testid="best-match-row">{match.key}</div>
  ),
  DismissedBestMatchRow: () => <div data-testid="dismissed-row" />,
}));

vi.mock('@/hooks/use-best-match-actions', () => ({
  useBestMatchActions: () => ({
    dismissedKeys: new Set<string>(),
    handleView: vi.fn(),
    handleSave: vi.fn(),
    handleApply: vi.fn(),
    handleDismiss: vi.fn(),
    undoDismiss: vi.fn(),
  }),
}));

let mockData: { matches: AutopilotBestMatch[]; total: number; autopilotCount: number } | undefined;

vi.mock('@/services', () => ({
  useBestMatches: () => ({ data: mockData }),
}));

import { BestMatchesPreview } from './index';

function makeMatch(key: string): AutopilotBestMatch {
  return {
    key,
    title: 'Engineer',
    company: 'Acme',
    url: `https://example.com/${key}`,
    score: 80,
    scoreSource: 'combined',
    foundAt: 0,
    sources: [],
  };
}

describe('BestMatchesPreview', () => {
  it('renders nothing when total is 0', () => {
    mockData = { matches: [], total: 0, autopilotCount: 0 };
    const { container } = render(<BestMatchesPreview />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when the query has not resolved yet', () => {
    mockData = undefined;
    const { container } = render(<BestMatchesPreview />);
    expect(container).toBeEmptyDOMElement();
  });

  it('caps the strip at 3 rows even when more qualify', () => {
    mockData = {
      matches: [makeMatch('a'), makeMatch('b'), makeMatch('c'), makeMatch('d'), makeMatch('e')],
      total: 5,
      autopilotCount: 2,
    };
    render(<BestMatchesPreview />);
    expect(screen.getAllByTestId('best-match-row')).toHaveLength(3);
  });

  it('links to /best-matches with the full total, not just the previewed count', () => {
    mockData = {
      matches: [makeMatch('a'), makeMatch('b'), makeMatch('c'), makeMatch('d'), makeMatch('e')],
      total: 5,
      autopilotCount: 2,
    };
    render(<BestMatchesPreview />);
    const link = screen.getByRole('link');
    expect(link).toHaveAttribute('href', '/best-matches');
    expect(link.textContent).toContain('"count":5');
  });
});
