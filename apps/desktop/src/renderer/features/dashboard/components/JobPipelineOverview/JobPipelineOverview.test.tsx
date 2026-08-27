/**
 * JobPipelineOverview — "Total tracked" tile must not count `dismissed`
 * interactions (a dismissal is the user explicitly rejecting a job — the
 * opposite of tracking it), and the zero-state must still fire once every
 * remaining interaction is a dismissal.
 */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  GlassCard: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

vi.mock('lucide-react', () => ({
  Bookmark: () => null,
  Briefcase: () => null,
  CheckCircle: () => null,
  Eye: () => null,
  TrendingUp: () => null,
}));

type Row = { interactionType: string };

let mockAll: Row[] = [];

vi.mock('@/services', () => ({
  useInteractions: (type?: string) => {
    if (!type) return { data: mockAll };
    return { data: mockAll.filter((r) => r.interactionType === type) };
  },
}));

import { JobPipelineOverview } from './index';

describe('JobPipelineOverview — totalTracked excludes dismissed', () => {
  it('does not count a dismissed record toward "Total tracked"', () => {
    mockAll = [{ interactionType: 'viewed' }, { interactionType: 'dismissed' }];
    render(<JobPipelineOverview />);

    // Both the "viewed" stat and "Total tracked" read 1 (the dismissed record
    // excluded from the total) — if dismissed were still counted, the total
    // would read 2 and no '2' would exist anywhere in the grid otherwise.
    expect(screen.getAllByText('1')).toHaveLength(2);
    expect(screen.queryByText('2')).not.toBeInTheDocument();
  });

  it('still shows the empty state when every interaction present is a dismissal', () => {
    mockAll = [{ interactionType: 'dismissed' }, { interactionType: 'dismissed' }];
    render(<JobPipelineOverview />);

    expect(screen.getByText('dashboard.noJobsTracked')).toBeInTheDocument();
  });

  it('does NOT show the empty state when a non-dismissed interaction exists', () => {
    mockAll = [{ interactionType: 'dismissed' }, { interactionType: 'bookmarked' }];
    render(<JobPipelineOverview />);

    expect(screen.queryByText('dashboard.noJobsTracked')).not.toBeInTheDocument();
  });

  it('counts viewed, opened, applied AND bookmarked toward the total', () => {
    mockAll = [
      { interactionType: 'viewed' },
      { interactionType: 'opened' },
      { interactionType: 'applied' },
      { interactionType: 'bookmarked' },
      { interactionType: 'dismissed' },
    ];
    render(<JobPipelineOverview />);

    expect(screen.getByText('4')).toBeInTheDocument();
  });
});
