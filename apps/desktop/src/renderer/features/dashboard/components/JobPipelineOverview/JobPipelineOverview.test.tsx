/**
 * JobPipelineOverview — Saved / Applied / Total come from the `applications`
 * table (same source as the Applications page); Viewed stays on interactions.
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

// Spread the real module rather than listing icons: the allowlist this
// component filters with now lives in the shared `features/dashboard/constants`
// module, which also holds the quick-action icons — an exhaustive stub breaks
// on an icon this component never renders.
vi.mock('lucide-react', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  Bookmark: () => null,
  Briefcase: () => null,
  CheckCircle: () => null,
  Eye: () => null,
  TrendingUp: () => null,
}));

type App = { status: string };

let mockApps: App[] = [];
let mockViewed: unknown[] = [];

vi.mock('@/services', () => ({
  useApplications: () => ({ data: mockApps }),
  useInteractions: () => ({ data: mockViewed }),
}));

import { JobPipelineOverview } from './index';

const tile = (label: string) => screen.getByText(label).previousElementSibling?.textContent;

describe('JobPipelineOverview — counts come from applications', () => {
  it('derives Saved / Applied / Total from the applications list, not interactions', () => {
    mockApps = [
      { status: 'saved' },
      { status: 'saved' },
      { status: 'applied' },
      { status: 'rejected' },
      { status: 'interviewing' },
    ];
    mockViewed = [{}, {}, {}];
    render(<JobPipelineOverview />);

    expect(tile('dashboard.savedJobs')).toBe('2');
    expect(tile('dashboard.applied')).toBe('1');
    expect(tile('dashboard.viewed')).toBe('3');
    expect(tile('dashboard.totalTracked')).toBe('5');
    expect(screen.queryByText('dashboard.noJobsTracked')).not.toBeInTheDocument();
  });

  it('counts applications even when there are ZERO interactions', () => {
    mockApps = [{ status: 'saved' }, { status: 'applied' }, { status: 'applied' }];
    mockViewed = [];
    render(<JobPipelineOverview />);

    expect(tile('dashboard.savedJobs')).toBe('1');
    expect(tile('dashboard.applied')).toBe('2');
    expect(tile('dashboard.totalTracked')).toBe('3');
  });

  it('shows the empty state when there are no applications', () => {
    mockApps = [];
    mockViewed = [];
    render(<JobPipelineOverview />);

    expect(tile('dashboard.totalTracked')).toBe('0');
    expect(screen.getByText('dashboard.noJobsTracked')).toBeInTheDocument();
  });
});
