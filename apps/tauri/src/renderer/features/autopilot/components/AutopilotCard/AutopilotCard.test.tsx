/**
 * AutopilotCard — handleHeaderToggle, handleHeaderKeyDown, handleJobClick tests.
 *
 * Strategy:
 *  - All service hooks and heavy sub-components are stubbed at module level.
 *  - motion/react AnimatePresence is shimmed so animated panels appear
 *    synchronously in jsdom (no CSS transitions).
 *  - useInteractions returns controlled data so viewedUrls can be exercised.
 *  - usePersistJob and useOpenExternal are spies — tests assert call args.
 *  - The header div carries role="button" when foundJobs.length > 0.
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
}));

// ── motion/react — render children synchronously, no animation ───────────────

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── lucide-react ──────────────────────────────────────────────────────────────

vi.mock('lucide-react', () => ({
  Briefcase: () => null,
  Check: () => null,
  ChevronUp: () => null,
  ExternalLink: () => null,
  Eye: () => null,
  Pause: () => null,
  Pencil: () => null,
  Play: () => null,
  RotateCcw: () => null,
  Trash2: () => null,
  Wand2: () => null,
}));

// ── @ajh/ui ───────────────────────────────────────────────────────────────────

vi.mock('@ajh/ui', () => ({
  ActionMenu: () => null,
  Button: ({
    children,
    onClick,
    disabled,
    'aria-label': ariaLabel,
    title,
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
    'aria-label'?: string;
    title?: string;
  }) => (
    <div
      role="button"
      onClick={onClick}
      aria-label={ariaLabel}
      title={title}
      aria-disabled={disabled}
    >
      {children}
    </div>
  ),
  ConfirmModal: () => null,
  GlassCard: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Tag: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  cn: (...args: string[]) => args.filter(Boolean).join(' '),
  transition: { fast: {}, normal: {} },
}));

// ── MatchBand stub ────────────────────────────────────────────────────────────

vi.mock('@/lib/match-band', () => ({
  MatchBand: ({ value, variant }: { value: number; variant?: string }) => (
    <span data-testid="match-band" data-value={value} data-variant={variant ?? 'combined'} />
  ),
}));

// ── timeAgo ───────────────────────────────────────────────────────────────────

vi.mock('@/lib/time', () => ({
  timeAgo: () => '3 min ago',
}));

// ── autopilot-run.machine ─────────────────────────────────────────────────────

vi.mock('@/lib/machines/autopilot-run.machine', () => ({
  RUN_STATE_LABEL: { idle: 'Idle', scraping: 'Scraping', ranking: 'Ranking', error: 'Error' },
}));

// ── service hooks — spies controlled per-test ─────────────────────────────────

const mockOpenExternal = vi.fn().mockResolvedValue(undefined);
const mockPersistJobAsync = vi.fn().mockResolvedValue(undefined);

// viewedData / openedData are controlled via these refs.
let stubbedViewedData: { url?: string }[] = [];
let stubbedOpenedData: { url?: string }[] = [];

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: mockOpenExternal, mutateAsync: mockOpenExternal }),
  usePersistJob: () => ({ mutateAsync: mockPersistJobAsync }),
  useInteractions: (type: string) => ({
    data: type === 'viewed' ? stubbedViewedData : stubbedOpenedData,
  }),
}));

// ── component under test ──────────────────────────────────────────────────────

import { AutopilotCard } from './index';

// ── fixtures ──────────────────────────────────────────────────────────────────

function makeAutopilot(foundJobs: AutopilotFoundJob[] = []): Autopilot {
  return {
    _id: 'ap-1',
    name: 'My Autopilot',
    status: 'active',
    target: { boards: ['linkedin'], query: 'engineer', pages: 1 },
    filter: { minMatchScore: 0 },
    schedule: 'daily',
    totalFound: foundJobs.length,
    totalApplied: 0,
    createdAt: 0,
    updatedAt: 0,
    foundJobs,
  };
}

function makeJob(url = 'https://example.com/job/1', score?: number): AutopilotFoundJob {
  return {
    title: 'Software Engineer',
    company: 'Acme',
    url,
    foundAt: 0,
    score,
  };
}

const defaultProps = {
  runState: 'idle' as const,
  stepLogs: [],
  onRun: vi.fn(),
  onTogglePause: vi.fn(),
  onEdit: vi.fn(),
  onDelete: vi.fn(),
  onApply: vi.fn(),
};

function renderCard(autopilot: Autopilot, extraProps = {}) {
  return render(<AutopilotCard autopilot={autopilot} {...defaultProps} {...extraProps} />);
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockOpenExternal.mockClear();
  mockPersistJobAsync.mockClear();
  stubbedViewedData = [];
  stubbedOpenedData = [];
});

// ─────────────────────────────────────────────────────────────────────────────
// handleHeaderToggle
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleHeaderToggle', () => {
  it('clicking the header toggles showFound when foundJobs.length > 0', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    // Found jobs panel is initially hidden.
    expect(screen.queryByText('autopilot.foundJobs · 1')).not.toBeInTheDocument();

    // The header div carries aria-expanded — use that as the unique selector.
    const headerDiv = document.querySelector('[aria-expanded]') as HTMLElement;
    expect(headerDiv).not.toBeNull();
    await user.click(headerDiv);

    // Panel is now visible: the inner heading contains the count.
    expect(screen.getByText(/autopilot\.foundJobs · 1/)).toBeInTheDocument();

    // Click again to collapse.
    await user.click(headerDiv);
    expect(screen.queryByText(/autopilot\.foundJobs · 1/)).not.toBeInTheDocument();
  });

  it('does NOT toggle when foundJobs is empty (no role=button on header)', () => {
    renderCard(makeAutopilot([]));
    // No header button role exists when there are no found jobs.
    expect(
      screen.queryByRole('button', { name: /autopilot.foundJobs: My Autopilot/i })
    ).not.toBeInTheDocument();
  });

  it('header carries aria-expanded=false initially when foundJobs present', () => {
    renderCard(makeAutopilot([makeJob()]));
    const header = document.querySelector('[aria-expanded]');
    expect(header).not.toBeNull();
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });

  it('aria-expanded becomes true after toggle', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);
    expect(header).toHaveAttribute('aria-expanded', 'true');
  });

  it('aria-label switches between foundJobs and collapse on toggle', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    // Initially: "autopilot.foundJobs: My Autopilot"
    expect(header).toHaveAttribute('aria-label', 'autopilot.foundJobs: My Autopilot');

    await user.click(header);
    // After expand: "autopilot.collapse: My Autopilot"
    expect(header).toHaveAttribute('aria-label', 'autopilot.collapse: My Autopilot');
  });

  it('clicking the actions cluster (stopPropagation) does NOT toggle showFound', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    expect(header).toHaveAttribute('aria-expanded', 'false');

    // The Run button is inside the actions cluster which stopPropagation.
    // After clicking Run, aria-expanded should still be false.
    const runButton = screen.getByRole('button', { name: /autopilot\.wizard\.run/i });
    await user.click(runButton);

    expect(header).toHaveAttribute('aria-expanded', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleHeaderKeyDown
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleHeaderKeyDown', () => {
  it('Enter key toggles showFound when foundJobs.length > 0', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard('{Enter}');

    expect(header).toHaveAttribute('aria-expanded', 'true');
  });

  it('Space key toggles showFound when foundJobs.length > 0', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard(' ');

    expect(header).toHaveAttribute('aria-expanded', 'true');
  });

  it('Enter then Enter collapses again', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard('{Enter}');
    expect(header).toHaveAttribute('aria-expanded', 'true');

    await user.keyboard('{Enter}');
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });

  it('Tab key does NOT toggle showFound', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    // Tab moves focus — no toggle expected.
    await user.keyboard('{Tab}');
    // aria-expanded remains false regardless of where focus went.
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });

  it('ArrowDown key does NOT toggle showFound', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard('{ArrowDown}');
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleJobClick — openExternal + persistJob + viewed badge
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleJobClick', () => {
  it('calls openExternal.mutate with the job url', async () => {
    const user = userEvent.setup();
    const job = makeJob('https://example.com/job/42');
    renderCard(makeAutopilot([job]));

    // Expand the header to show the found-jobs panel.
    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);

    const jobButton = screen.getByTitle('autopilot.viewJob');
    await user.click(jobButton);

    expect(mockOpenExternal).toHaveBeenCalledWith('https://example.com/job/42');
  });

  it('calls persistJob.mutateAsync with interactionType: viewed and the job url', async () => {
    const user = userEvent.setup();
    const job = makeJob('https://example.com/job/42');
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);

    const jobButton = screen.getByTitle('autopilot.viewJob');
    await act(async () => {
      await user.click(jobButton);
    });

    expect(mockPersistJobAsync).toHaveBeenCalledTimes(1);
    const callArg = mockPersistJobAsync.mock.calls[0]?.[0] as Record<string, unknown> | undefined;
    expect(callArg?.interactionType).toBe('viewed');
    expect((callArg?.job as Record<string, unknown> | undefined)?.url).toBe(
      'https://example.com/job/42'
    );
  });

  it('shows the Eye/viewed badge for a url that is in viewedUrls', async () => {
    const jobUrl = 'https://example.com/job/viewed';
    stubbedViewedData = [{ url: jobUrl }];

    renderCard(makeAutopilot([makeJob(jobUrl)]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    // Expand found-jobs panel.
    await act(async () => {
      header.click();
    });

    // The viewed badge (t('jobs.viewed') → 'jobs.viewed') should appear.
    expect(screen.getByText('jobs.viewed')).toBeInTheDocument();
  });

  it('shows the viewed badge for a url from openedData (opened counts as viewed)', async () => {
    const jobUrl = 'https://example.com/job/opened';
    stubbedOpenedData = [{ url: jobUrl }];

    renderCard(makeAutopilot([makeJob(jobUrl)]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.getByText('jobs.viewed')).toBeInTheDocument();
  });

  it('does NOT show the viewed badge for an unvisited url', async () => {
    stubbedViewedData = [];
    stubbedOpenedData = [];

    renderCard(makeAutopilot([makeJob('https://example.com/job/unseen')]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Found-jobs render the coverage MatchBand
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — found-jobs MatchBand variant', () => {
  it('renders MatchBand with variant=coverage when job.score is present', async () => {
    const job = makeJob('https://example.com/job/scored', 72);
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    const band = screen.getByTestId('match-band');
    expect(band).toHaveAttribute('data-variant', 'coverage');
    expect(band).toHaveAttribute('data-value', '72');
  });

  it('does NOT render MatchBand when job.score is absent', async () => {
    const job = makeJob('https://example.com/job/no-score'); // no score property
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.queryByTestId('match-band')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleJobClick — persistJob rejection (swallowed catch; #3)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleJobClick persistJob rejection', () => {
  it('openExternal.mutate still fires when persistJob.mutateAsync rejects', async () => {
    mockPersistJobAsync.mockRejectedValueOnce(new Error('network'));

    const user = userEvent.setup();
    const job = makeJob('https://example.com/job/persist-fail');
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);

    await act(async () => {
      await user.click(screen.getByTitle('autopilot.viewJob'));
    });

    // openExternal fires before the try/catch around persistJob.
    expect(mockOpenExternal).toHaveBeenCalledWith('https://example.com/job/persist-fail');
    // No unhandled rejection — test runner would fail if one escaped.
  });
});
