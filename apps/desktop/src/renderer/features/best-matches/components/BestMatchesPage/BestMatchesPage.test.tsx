/**
 * BestMatchesPage — the honest-empty-state branch (`hasEverRun`) and the
 * truncation / salary-caption copy, none of which are covered by
 * `useBestMatchActions`/`sortBestMatches`'s own unit tests.
 */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { Autopilot, AutopilotBestMatch } from '@ajh/shared';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      `${key}${opts ? `:${JSON.stringify(opts)}` : ''}`,
  }),
}));

vi.mock('@ajh/ui', () => ({
  Dropdown: ({ 'aria-label': ariaLabel }: { 'aria-label'?: string }) => (
    <div role="group" aria-label={ariaLabel} />
  ),
  EmptyState: ({ title, description }: { title: string; description?: string }) => (
    <div data-testid="empty-state">
      <p>{title}</p>
      <p>{description}</p>
    </div>
  ),
  ErrorState: ({ title }: { title?: string }) => <div data-testid="error-state">{title}</div>,
  RowSkeleton: () => <div data-testid="row-skeleton" />,
}));

vi.mock('lucide-react', () => ({
  ArrowDownWideNarrow: () => null,
  Sparkles: () => null,
}));

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children, actions }: { children: ReactNode; actions?: ReactNode }) => (
    <div>
      {actions}
      {children}
    </div>
  ),
}));

vi.mock('@/components/job/BestMatchRow', () => ({
  BestMatchRow: ({ match }: { match: AutopilotBestMatch }) => (
    <div data-testid="best-match-row">{match.key}</div>
  ),
  DismissedBestMatchRow: () => <div data-testid="dismissed-row" />,
}));

const mockHandleView = vi.fn();
const mockHandleSave = vi.fn();
const mockHandleApply = vi.fn();
const mockHandleDismiss = vi.fn();
const mockUndoDismiss = vi.fn();

vi.mock('@/hooks/use-best-match-actions', () => ({
  useBestMatchActions: () => ({
    dismissedKeys: new Set<string>(),
    handleView: mockHandleView,
    handleSave: mockHandleSave,
    handleApply: mockHandleApply,
    handleDismiss: mockHandleDismiss,
    undoDismiss: mockUndoDismiss,
  }),
}));

let mockBestMatchesData:
  { matches: AutopilotBestMatch[]; total: number; autopilotCount: number } | undefined;
let mockBestMatchesLoading = false;
let mockBestMatchesError = false;
let mockAutopilots: Autopilot[] = [];

vi.mock('@/services', () => ({
  useBestMatches: () => ({
    data: mockBestMatchesData,
    isLoading: mockBestMatchesLoading,
    isError: mockBestMatchesError,
    refetch: vi.fn(),
  }),
  useAutopilots: () => ({ data: mockAutopilots }),
}));

import { BestMatchesPage } from './index';

function makeMatch(overrides: Partial<AutopilotBestMatch> = {}): AutopilotBestMatch {
  return {
    key: 'k1',
    title: 'Engineer',
    company: 'Acme',
    url: 'https://example.com/1',
    score: 80,
    scoreSource: 'combined',
    foundAt: 0,
    sources: [],
    ...overrides,
  };
}

function makeAutopilot(overrides: Partial<Autopilot> = {}): Autopilot {
  return {
    _id: 'ap-1',
    name: 'Berlin roles',
    status: 'active',
    target: { boards: ['linkedin'], query: 'engineer', pages: 1 },
    filter: { minMatchScore: 0 },
    schedule: 'daily',
    totalFound: 0,
    totalApplied: 0,
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}

describe('BestMatchesPage — empty states', () => {
  it('shows the "no autopilot has run" message when no autopilot has ever run', () => {
    mockBestMatchesData = { matches: [], total: 0, autopilotCount: 0 };
    mockAutopilots = [makeAutopilot({ lastRunAt: undefined })];

    render(<BestMatchesPage />);

    expect(screen.getByText('bestMatches.empty.noRuns.title')).toBeInTheDocument();
  });

  it('shows the "nothing cleared the bar" message when autopilots HAVE run but nothing qualified', () => {
    mockBestMatchesData = { matches: [], total: 0, autopilotCount: 0 };
    mockAutopilots = [makeAutopilot({ lastRunAt: Date.now() })];

    render(<BestMatchesPage />);

    expect(screen.getByText('bestMatches.empty.noneQualified.title')).toBeInTheDocument();
  });

  it('shows the "no autopilot has run" message when there are no autopilots at all', () => {
    mockBestMatchesData = { matches: [], total: 0, autopilotCount: 0 };
    mockAutopilots = [];

    render(<BestMatchesPage />);

    expect(screen.getByText('bestMatches.empty.noRuns.title')).toBeInTheDocument();
  });
});

describe('BestMatchesPage — truncation + salary caption', () => {
  it('shows the truncated notice only when total exceeds matches.length', () => {
    mockBestMatchesData = { matches: [makeMatch()], total: 5, autopilotCount: 1 };
    mockAutopilots = [makeAutopilot({ lastRunAt: Date.now() })];

    render(<BestMatchesPage />);

    expect(screen.getByText(/bestMatches\.truncated/)).toBeInTheDocument();
  });

  it('does NOT show the truncated notice when total equals matches.length', () => {
    mockBestMatchesData = { matches: [makeMatch()], total: 1, autopilotCount: 1 };
    mockAutopilots = [makeAutopilot({ lastRunAt: Date.now() })];

    render(<BestMatchesPage />);

    expect(screen.queryByText(/bestMatches\.truncated/)).not.toBeInTheDocument();
  });

  it('renders one row per match when matches exist', () => {
    mockBestMatchesData = {
      matches: [makeMatch({ key: 'a' }), makeMatch({ key: 'b' })],
      total: 2,
      autopilotCount: 1,
    };
    mockAutopilots = [makeAutopilot({ lastRunAt: Date.now() })];

    render(<BestMatchesPage />);

    expect(screen.getAllByTestId('best-match-row')).toHaveLength(2);
  });
});

describe('BestMatchesPage — loading / error', () => {
  it('renders skeletons while loading', () => {
    mockBestMatchesLoading = true;
    mockBestMatchesError = false;
    mockBestMatchesData = undefined;
    mockAutopilots = [];

    render(<BestMatchesPage />);

    expect(screen.getAllByTestId('row-skeleton').length).toBeGreaterThan(0);
    mockBestMatchesLoading = false;
  });

  it('renders the error state when the query fails', () => {
    mockBestMatchesLoading = false;
    mockBestMatchesError = true;
    mockBestMatchesData = undefined;
    mockAutopilots = [];

    render(<BestMatchesPage />);

    expect(screen.getByTestId('error-state')).toBeInTheDocument();
    mockBestMatchesError = false;
  });
});
