/**
 * JobsPage — cross-board cluster + agency filtering (ADR-029).
 *
 * Exercises the `filtered` memo via the `filtered` list JobsPage forwards to
 * JobsResults (the same capture seam the sort tests use):
 *   - clusterCanonical === false rows are hidden (collapsed into the canonical).
 *   - Unannotated rows (no cluster fields) always show.
 *   - `hideAgency` (session state) removes isAgency rows.
 *   - The hide-agency toggle writes hideAgency via setJobs.
 *
 * The session-store mock is a mutable container so hideAgency can be toggled
 * per test. All heavy dependencies are module-mocked. `mergePostings` is real —
 * fixtures use distinct urls/ids so nothing collapses at the stage-1 dedup.
 */

import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

// Mutable jobs slice — mutate before render to drive hideAgency.
const jobsState = {
  filter: '',
  sortBy: 'newest' as const,
  viewMode: 'list' as const,
  selectedId: null as string | null,
  listScrollTop: 0,
  hideAgency: false,
};

const setJobsSpy = vi.fn();

// usePostings container + captured `filtered` forwarded to JobsResults.
const postingsContainer: { data: Array<Record<string, unknown>> } = { data: [] };
const resultsProps = { filtered: undefined as unknown };

vi.mock('@/features/jobs/hooks/useScraping', () => ({
  useScraping: () => ({
    scraping: false,
    scrapeOutcome: null,
    livePostings: [],
    setLivePostings: vi.fn(),
    scrapeJobRef: { current: 'job-1' },
    replacePendingRef: { current: false },
    startScrape: vi.fn(),
    cancelScrape: vi.fn(),
    noteScrapeFinished: vi.fn(),
  }),
}));

vi.mock('@/services', () => ({
  usePostings: () => postingsContainer,
  useClearPostings: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useInvalidatePostings: () => vi.fn(),
  useJobPreferences: () => ({ data: undefined }),
  useGeocodeSuggest: () => vi.fn().mockResolvedValue([]),
  useJobEvents: () => {},
}));

vi.mock('@/hooks/useDefaultResumeId', () => ({ useDefaultResumeId: () => null }));

vi.mock('@/store/session-store', () => ({
  useSessionStore: () => ({
    jobs: jobsState,
    setJobs: (...args: unknown[]) => setJobsSpy(...args),
    setSettings: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => (ts: number) => String(ts),
}));

vi.mock('@/components/layout/PageHeader', () => ({
  PageHeader: ({ title, actions }: { title: string; actions?: ReactNode }) => (
    <div>
      {title}
      {actions}
    </div>
  ),
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/features/jobs/components/JobsResults', () => ({
  JobsResults: ({ filtered }: { filtered?: unknown }) => {
    resultsProps.filtered = filtered;
    return <div data-testid={TEST_IDS.jobs.jobsResults} />;
  },
}));

vi.mock('@/components/scrape/BoardSummaryChips', () => ({
  BoardSummaryChips: () => null,
  sanitizeReason: (raw: string) => raw,
}));

vi.mock('@/features/jobs/components/ScrapeForm', () => ({
  ScrapeForm: () => null,
}));

vi.mock('@/features/jobs/providers', () => ({
  MatchScoresProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => {
  const Tag = Object.assign(({ children }: { children: ReactNode }) => <span>{children}</span>, {
    CheckableTag: ({
      children,
      checked,
      onChange,
    }: {
      children: ReactNode;
      checked: boolean;
      onChange?: (v: boolean) => void;
    }) => (
      <button type="button" aria-pressed={checked} onClick={() => onChange?.(!checked)}>
        {children}
      </button>
    ),
  });
  return {
    Button: ({ children, onClick }: { children: ReactNode; onClick?: () => void }) => (
      <div role="button" onClick={onClick}>
        {children}
      </div>
    ),
    ConfirmModal: () => null,
    Dropdown: () => null,
    Input: () => null,
    SegmentedControl: () => null,
    Tag,
    useNotification: () => ({ success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() }),
  };
});

import { JobsPage } from './index';

function post(id: string, extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: `Role ${id}`,
    company: 'Acme',
    description: '',
    postedAt: 1000,
    capturedAt: 0,
    ...extra,
  };
}

function filteredIds(): string[] {
  return (resultsProps.filtered as Array<{ id: string }>).map((p) => p.id);
}

beforeEach(() => {
  setJobsSpy.mockClear();
  resultsProps.filtered = undefined;
  jobsState.hideAgency = false;
  postingsContainer.data = [
    post('canonical', { clusterId: 'canonical', clusterCanonical: true }),
    post('collapsed', { clusterId: 'canonical', clusterCanonical: false }),
    post('unannotated'),
    post('agency', { clusterId: 'agency', clusterCanonical: true, isAgency: true }),
  ];
});

describe('JobsPage — cross-board cluster filter', () => {
  it('hides clusterCanonical=false rows but keeps canonical + unannotated', () => {
    render(<JobsPage />);
    const ids = new Set(filteredIds());
    expect(ids.has('canonical')).toBe(true);
    expect(ids.has('unannotated')).toBe(true);
    expect(ids.has('collapsed')).toBe(false);
  });

  it('shows agency rows when hideAgency is false', () => {
    render(<JobsPage />);
    expect(new Set(filteredIds()).has('agency')).toBe(true);
  });

  it('hides agency rows when hideAgency is true (canonical/unannotated still shown)', () => {
    jobsState.hideAgency = true;
    render(<JobsPage />);
    const ids = new Set(filteredIds());
    expect(ids.has('agency')).toBe(false);
    expect(ids.has('canonical')).toBe(true);
    expect(ids.has('unannotated')).toBe(true);
    expect(ids.has('collapsed')).toBe(false);
  });

  it('the hide-agency toggle writes hideAgency via setJobs', async () => {
    const user = userEvent.setup();
    render(<JobsPage />);
    const toggle = within(screen.getByTestId(TEST_IDS.jobs.hideAgencyToggle)).getByRole('button');
    await act(async () => {
      await user.click(toggle);
    });
    expect(setJobsSpy).toHaveBeenCalledWith({ hideAgency: true });
  });
});
