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
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Autopilot, AutopilotFoundJob, BoardScrapeSummary } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import type * as MatchBandModule from '@/lib/match-band';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
}));

// ── motion/react — render children synchronously, no animation ───────────────

// jsdom has no real animation engine — fire `onAnimationComplete` once on
// MOUNT (matching a real single enter-animation completing), not on every
// prop-identity change (the real `onAnimationComplete`/`resolvePendingScroll`
// callback is recreated every render). A "latest ref" holds the current
// callback so the effect itself can stay mount-only ([] deps) without going
// stale — this is what lets tests distinguish "enter animation ran" from
// "already mounted, no animation" (the rAF-fallback path).
vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        {
          children,
          onAnimationComplete,
          ...rest
        }: React.HTMLAttributes<HTMLDivElement> & { onAnimationComplete?: () => void },
        ref: React.Ref<HTMLDivElement>
      ) => {
        const onAnimationCompleteRef = React.useRef(onAnimationComplete);
        onAnimationCompleteRef.current = onAnimationComplete;
        React.useEffect(() => {
          onAnimationCompleteRef.current?.();
        }, []);
        return (
          <div ref={ref} {...rest}>
            {children}
          </div>
        );
      }
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
  Info: () => null,
  Pause: () => null,
  Pencil: () => null,
  Play: () => null,
  RotateCcw: () => null,
  Sparkles: () => null,
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
    'data-degraded': dataDegraded,
    'data-testid': dataTestId,
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
    'aria-label'?: string;
    title?: string;
    'data-degraded'?: boolean;
    'data-testid'?: string;
  }) =>
    // Use createElement to avoid the JSXOpeningElement[name="button"] lint rule.
    // A native <button> is required so disabled + keyboard behavior are real.
    // `data-degraded` is forwarded (not the raw className) as the seam for the
    // amber-tone assertion — a data-* seam over a Tailwind class string, per the
    // jsdom-CSS-parsing lesson. `data-testid` is forwarded so the cluster split
    // button is queryable.
    React.createElement(
      'button',
      {
        onClick,
        'aria-label': ariaLabel,
        title,
        disabled,
        'data-degraded': dataDegraded,
        'data-testid': dataTestId,
      },
      children
    ),
  ConfirmModal: () => null,
  // One button per option (native, via createElement — same rule as Button
  // above). `aria-pressed` marks the current value; clicking a button fires
  // onChange directly — no open/close affordance needed for these tests.
  Dropdown: ({
    options,
    value,
    onChange,
    'aria-label': ariaLabel,
  }: {
    options: { value: string; label: string }[];
    value: string;
    onChange: (value: string) => void;
    'aria-label'?: string;
  }) =>
    React.createElement(
      'div',
      { role: 'group', 'aria-label': ariaLabel },
      options.map((o) =>
        React.createElement(
          'button',
          {
            key: o.value,
            type: 'button',
            'aria-pressed': o.value === value,
            onClick: () => onChange(o.value),
          },
          o.label
        )
      )
    ),
  GlassCard: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  // Render both trigger and panel content so the badge label AND its hover
  // explainer are queryable in jsdom (no real hover needed).
  HoverPopover: ({
    trigger,
    children,
  }: {
    trigger: React.ReactNode;
    children: React.ReactNode;
  }) => (
    <span>
      {trigger}
      {children}
    </span>
  ),
  Tag: ({ color, children }: { color?: string; children: React.ReactNode }) => (
    <span data-testid="chip" data-color={color}>
      {children}
    </span>
  ),
  cn: (...args: string[]) => args.filter(Boolean).join(' '),
  transition: { fast: {}, normal: {} },
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));

// ── MatchBand stub ────────────────────────────────────────────────────────────
//
// Keeps the REAL `scoreTier` (via importActual) so the mock's muted/not-muted
// output actually reflects the real component's tier-dependent formula
// (`muted || (subtle && tier !== 'High')`) instead of just echoing whatever
// boolean prop was passed — a naive echo would pass this test file even if the
// real MatchBand left a provisional HIGH score full-color (the CodeRabbit gap).

vi.mock('@/lib/match-band', async (importActual) => {
  const actual = await importActual<typeof MatchBandModule>();
  return {
    ...actual,
    MatchBand: ({
      value,
      variant,
      subtle,
      muted,
      describe = true,
    }: {
      value: number;
      variant?: 'combined' | 'coverage';
      subtle?: boolean;
      muted?: boolean;
      describe?: boolean;
    }) => {
      const tier = actual.scoreTier(value, variant ?? 'combined').key;
      const isMutedStyle = Boolean(muted) || (Boolean(subtle) && tier !== 'High');
      // `describe` is echoed, not re-implemented: the question these tests ask
      // is which value AUTOPILOTCARD passes at each call site (the provisional
      // wrapper owns the copy and must opt out; the bare band must not). What
      // the real MatchBand renders for it is match-band.test.tsx's job.
      return (
        <span
          data-testid="match-band"
          data-value={value}
          data-variant={variant ?? 'combined'}
          data-tier={tier}
          data-muted={isMutedStyle ? 'true' : 'false'}
          data-describe={describe ? 'true' : 'false'}
        />
      );
    },
  };
});

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
const mockSplitMutate = vi.fn();

// viewedData / openedData are controlled via these refs.
let stubbedViewedData: { url?: string }[] = [];
let stubbedOpenedData: { url?: string }[] = [];

/** Board id → live health, as `useBoardsHealth` returns it. Mutated per test. */
let mockBoardHealth = new Map<string, unknown>();

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: mockOpenExternal, mutateAsync: mockOpenExternal }),
  usePersistJob: () => ({ mutateAsync: mockPersistJobAsync }),
  useMarkNotDuplicate: () => ({ mutate: mockSplitMutate, isPending: false }),
  useInteractions: (type: string) => ({
    data: type === 'viewed' ? stubbedViewedData : stubbedOpenedData,
  }),
  // Track B1 — the card reads the LIVE per-board reliability verdict rather than
  // taking it off the stored run record. Empty by default here; the health
  // suite below overrides it.
  useBoardsHealth: () => ({ data: mockBoardHealth }),
}));

// Cluster/agency chips are covered in their own suites; stubbed here so this
// suite's fixtures (no cluster data) don't need extra provider wiring.
vi.mock('@/components/job/ClusterSourceChips', () => ({
  ClusterSourceChips: () => null,
}));

vi.mock('@/components/job/AgencyChip', () => ({
  AgencyChip: () => null,
}));

// ── component under test ──────────────────────────────────────────────────────

import { AutopilotCard, sortFoundJobsByDate } from './index';

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

// Build an autopilot with a persisted run outcome. Takes a plain `string` so an
// unknown/future status can be exercised (the graceful-fallback path) — narrowed
// to the union via `as`, which is valid for string → string-literal.
function withRunStatus(status: string): Autopilot {
  return { ...makeAutopilot(), runStatus: status as Autopilot['runStatus'] };
}

// Autopilot with a persisted run outcome AND its per-board summaries (PR B) so
// the chip strip + needs-configuration guard can be exercised.
function withRun(status: string, summaries: BoardScrapeSummary[]): Autopilot {
  return {
    ...makeAutopilot(),
    runStatus: status as Autopilot['runStatus'],
    lastRunSummaries: summaries,
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
  mockSplitMutate.mockClear();
  stubbedViewedData = [];
  stubbedOpenedData = [];
  mockBoardHealth = new Map<string, unknown>();
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
// Provisional score marker (PR H, audit root cause 6) — a snippet-based score
// is muted + tilde-prefixed + carries a hover hint; an exact score is plain.
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — provisional score marker', () => {
  it('renders a muted band + "~" prefix + hover title + sr-only text when scoreProvisional is true (HIGH-tier score)', async () => {
    // 82 under variant='coverage' (>=55 threshold) is a HIGH-tier score — the
    // exact case CodeRabbit flagged: MatchBand's `subtle` prop deliberately
    // keeps High bright, so the provisional marker must use `muted` (mutes
    // ALL tiers) instead, or a provisional HIGH would misleadingly stay
    // full-color. The mock recomputes muting from the REAL scoreTier, so this
    // assertion only passes if AutopilotCard passes `muted`, not `subtle`.
    const job = { ...makeJob('https://example.com/job/prov', 82), scoreProvisional: true };
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    // The native hover hint (title) carries BOTH facts: what the tier claims,
    // and that the number behind it is only an estimate. They answer different
    // questions, so neither may be dropped.
    // The band's own nearest titled ancestor — not just any title on the card.
    const marker = screen.getByTestId('match-band').closest('[title]') as HTMLElement;
    expect(marker.title).toContain('jobs.matchBand.desc.coverage.High');
    expect(marker.title).toContain('autopilot.provisionalScoreHint');
    // Exactly ONE title on this marker — the band must not render its own
    // inside this wrapper, or the inner one wins on hover over the badge and
    // hides the provisional caveat entirely.
    expect(marker.querySelectorAll('[title]')).toHaveLength(0);
    // ...the "~" estimate prefix is visible...
    expect(screen.getByText('~')).toBeInTheDocument();
    // ...an always-present sr-only span carries the same words for screen
    // readers (a `title` alone isn't reliably announced — TrustBadge
    // precedent), and only ONE of them, not one per nested describer...
    const srOnly = marker.querySelectorAll('.sr-only');
    expect(srOnly).toHaveLength(1);
    expect(srOnly[0]?.textContent).toBe(`: ${marker.title}`);
    // The band itself must stay silent here — this wrapper speaks for it.
    expect(screen.getByTestId('match-band')).toHaveAttribute('data-describe', 'false');
    // ...the band IS the High tier (proving this is genuinely a HIGH-score case)...
    const band = screen.getByTestId('match-band');
    expect(band).toHaveAttribute('data-tier', 'High');
    // ...and still renders muted, unlike `subtle`'s High-stays-bright contract.
    expect(band).toHaveAttribute('data-muted', 'true');
  });

  it('renders a plain (non-muted) HIGH band with no marker when scoreProvisional is false', async () => {
    const job = { ...makeJob('https://example.com/job/exact', 82), scoreProvisional: false };
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.queryByTitle('autopilot.provisionalScoreHint')).not.toBeInTheDocument();
    expect(screen.queryByText('~')).not.toBeInTheDocument();
    expect(screen.queryByText(': autopilot.provisionalScoreHint')).not.toBeInTheDocument();
    const band = screen.getByTestId('match-band');
    expect(band).toHaveAttribute('data-tier', 'High');
    expect(band).toHaveAttribute('data-muted', 'false');
    // The band opts out of describing itself because its WRAPPER now owns the
    // richer copy (metric name + tier description) for every score, provisional
    // or not. The invariant that matters is unchanged: the badge is never a
    // bare, unexplained word — so assert the explanation is actually there,
    // exactly once, rather than which component happens to render it.
    expect(band).toHaveAttribute('data-describe', 'false');
    const marker = band.closest('[title]') as HTMLElement;
    expect(marker.title).toContain('jobs.matchBand.desc.coverage.High');
    expect(marker.querySelectorAll('[title]')).toHaveLength(0);
    expect(marker.querySelectorAll('.sr-only')).toHaveLength(1);
  });

  it('treats an absent scoreProvisional field (older records) as non-provisional', async () => {
    // makeJob() sets no scoreProvisional — the legacy record shape.
    const job = makeJob('https://example.com/job/legacy', 82);
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.queryByTitle('autopilot.provisionalScoreHint')).not.toBeInTheDocument();
    expect(screen.getByTestId('match-band')).toHaveAttribute('data-muted', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// scoreSource — the metric label flips to "Match %" ONLY for a job the backend
// actually re-ranked through the semantic kernel (ADR-020 addendum).
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — score metric label', () => {
  /** Expand the found-jobs panel and return the band + its titled wrapper. */
  async function renderScored(job: AutopilotFoundJob) {
    renderCard(makeAutopilot([job]));
    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });
    const band = screen.getByTestId('match-band');
    return { band, marker: band.closest('[title]') as HTMLElement };
  }

  it('labels a keyword-scored job "Keyword Coverage %" on the coverage scale', async () => {
    const job = { ...makeJob('https://example.com/job/kw', 60), scoreSource: 'keyword' as const };
    const { band, marker } = await renderScored(job);

    expect(marker.title).toContain('autopilot.scoreLabel.coverage');
    expect(marker.title).not.toContain('autopilot.scoreLabel.combined');
    expect(band).toHaveAttribute('data-variant', 'coverage');
    // 60 is High on the coverage scale (>=55) but only Medium on the combined
    // one (>=50) — so the tier here also proves the right cut points ran, not
    // just that the right word was printed.
    expect(band).toHaveAttribute('data-tier', 'High');
    expect(marker.querySelectorAll('.sr-only')[0]?.textContent).toBe(`: ${marker.title}`);
  });

  it('flips to "Match %" on the combined scale when the backend re-ranked the job', async () => {
    const job = { ...makeJob('https://example.com/job/sem', 60), scoreSource: 'combined' as const };
    const { band, marker } = await renderScored(job);

    expect(marker.title).toContain('autopilot.scoreLabel.combined');
    expect(marker.title).not.toContain('autopilot.scoreLabel.coverage');
    expect(band).toHaveAttribute('data-variant', 'combined');
    // Same 60, different metric → Medium, not High. A label-only flip that left
    // the variant on 'coverage' would still read High here and fail.
    expect(band).toHaveAttribute('data-tier', 'Medium');
    expect(marker.title).toContain('jobs.matchBand.desc.combined.Medium');
  });

  it('treats an absent scoreSource (every pre-existing record) as keyword coverage', async () => {
    // makeJob() sets no scoreSource — the legacy record shape, and also what a
    // run with semantic scoring OFF writes.
    const { band, marker } = await renderScored(makeJob('https://example.com/job/legacy', 60));

    expect(marker.title).toContain('autopilot.scoreLabel.coverage');
    expect(band).toHaveAttribute('data-variant', 'coverage');
  });

  // ── the mixed-scale affordance ──────────────────────────────────────────
  //
  // After a semantic re-rank the list holds TWO scales and is sorted in two
  // blocks, so a combined 58 legitimately sits above a keyword 62. The metric
  // was only ever in the tier colour and the sr-only text, which reads to a
  // sighted user as a sorting bug.

  /** Expand the found-jobs panel for a whole list. */
  async function renderList(jobs: AutopilotFoundJob[]) {
    renderCard(makeAutopilot(jobs));
    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });
  }

  const combined = (url: string, score: number): AutopilotFoundJob => ({
    ...makeJob(url, score),
    scoreSource: 'combined' as const,
  });
  const keyword = (url: string, score: number): AutopilotFoundJob => ({
    ...makeJob(url, score),
    scoreSource: 'keyword' as const,
  });

  it('names each row’s metric when the list mixes the two scales', async () => {
    // The exact reported shape: the re-ranked head scores LOWER than the
    // keyword tail, so without a visible metric the order looks broken.
    await renderList([combined('https://example.com/a', 58), keyword('https://example.com/b', 62)]);

    expect(screen.getByText('autopilot.scoreAbbr.combined')).toBeInTheDocument();
    expect(screen.getByText('autopilot.scoreAbbr.coverage')).toBeInTheDocument();
  });

  it('adds nothing when every score is on the same scale', async () => {
    // The overwhelmingly common case (semantic scoring off, or a run where
    // every job re-ranked): an identical label on every row is pure noise.
    await renderList([keyword('https://example.com/a', 62), keyword('https://example.com/b', 40)]);

    expect(screen.queryByText('autopilot.scoreAbbr.coverage')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.scoreAbbr.combined')).not.toBeInTheDocument();
  });

  it('ignores unscored rows when deciding whether the list mixes', async () => {
    // An unscored job renders no band at all, so it cannot be one of the two
    // scales — counting it would label a uniform list.
    await renderList([keyword('https://example.com/a', 62), makeJob('https://example.com/b')]);

    expect(screen.queryByText('autopilot.scoreAbbr.coverage')).not.toBeInTheDocument();
  });

  it('keeps the metric out of the accessible name, which already carries it', async () => {
    // aria-hidden: the sr-only span next to the band announces the FULL label
    // ("Keyword Coverage %"), so an announced abbreviation would be a second,
    // shorter duplicate of the same fact.
    await renderList([combined('https://example.com/a', 58), keyword('https://example.com/b', 62)]);

    expect(screen.getByText('autopilot.scoreAbbr.coverage')).toHaveAttribute('aria-hidden', 'true');
  });

  it('keeps the provisional caveat alongside the flipped label', async () => {
    // A re-ranked aggregator job is BOTH semantic and snippet-derived: the
    // label flips, and the "~"/muted/caveat treatment must survive.
    const job = {
      ...makeJob('https://example.com/job/both', 60),
      scoreSource: 'combined' as const,
      scoreProvisional: true,
    };
    const { band, marker } = await renderScored(job);

    expect(marker.title).toContain('autopilot.scoreLabel.combined');
    expect(marker.title).toContain('autopilot.provisionalScoreHint');
    expect(screen.getByText('~')).toBeInTheDocument();
    expect(band).toHaveAttribute('data-muted', 'true');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// assistantNotes — Phase 4 AI note (read-only, plain text)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — assistantNotes', () => {
  it('renders the AI note when job.assistantNotes is present', async () => {
    const job = {
      ...makeJob('https://example.com/job/noted'),
      assistantNotes: 'Great fit — highlight your Rust experience.',
    };
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.getByText('Great fit — highlight your Rust experience.')).toBeInTheDocument();
    expect(screen.getByRole('note', { name: 'autopilot.aiNote' })).toBeInTheDocument();
  });

  it('does NOT render an AI note block when job.assistantNotes is absent', async () => {
    const job = makeJob('https://example.com/job/no-note');
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.queryByRole('note')).not.toBeInTheDocument();
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

// ─────────────────────────────────────────────────────────────────────────────
// focusedJobUrl — scroll-to-row + transient highlight (Back-navigation fix)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — focusedJobUrl scroll + highlight', () => {
  let scrollSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    scrollSpy = vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(() => {});
  });

  afterEach(() => {
    scrollSpy.mockRestore();
    vi.useRealTimers();
  });

  it('scrolls the row matching focusedJobUrl into view, not the header', () => {
    const jobUrl = 'https://example.com/job/42';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: jobUrl });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(row).not.toBeNull();
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    const instance = scrollSpy.mock.instances[0];
    if (!instance) throw new Error('scrollIntoView was not called');
    expect(instance).toBe(row);
    expect(scrollSpy).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: 'smooth', block: 'center' })
    );
  });

  it('applies the transient highlight ring to the targeted row', () => {
    const jobUrl = 'https://example.com/job/highlight';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: jobUrl });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(row).toHaveClass('ring-brand/60');
  });

  it('fades the highlight after ~1.5s', () => {
    vi.useFakeTimers();
    const jobUrl = 'https://example.com/job/fade';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: jobUrl });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(row).toHaveClass('ring-brand/60');

    act(() => {
      vi.advanceTimersByTime(1500);
    });

    expect(row).not.toHaveClass('ring-brand/60');
  });

  it('calls onFocusHandled once the row has been scrolled to', () => {
    const jobUrl = 'https://example.com/job/handled';
    const onFocusHandled = vi.fn();
    renderCard(makeAutopilot([makeJob(jobUrl)]), {
      focused: true,
      focusedJobUrl: jobUrl,
      onFocusHandled,
    });

    expect(onFocusHandled).toHaveBeenCalledTimes(1);
  });

  it('falls back to centering the header when focusedJobUrl is absent', () => {
    const jobUrl = 'https://example.com/job/no-focus-url';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: null });

    const header = document.querySelector('[aria-expanded]');
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    const instance = scrollSpy.mock.instances[0];
    if (!instance) throw new Error('scrollIntoView was not called');
    expect(instance).toBe(header);
  });

  it('scrolls via the rAF fallback when the panel is already expanded (no enter animation fires)', async () => {
    // Sync rAF stub — jsdom's real rAF is timer-based; this makes the fallback
    // resolve synchronously within the test's act() calls.
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0);
        return 0;
      });
    const onFocusHandled = vi.fn();
    const jobUrl = 'https://example.com/job/already-expanded';
    const autopilot = makeAutopilot([makeJob(jobUrl)]);
    const { rerender } = renderCard(autopilot, { focused: false });

    // Manually expand via the header — NOT via `focused` — so the found-jobs
    // panel's enter animation (and its onAnimationComplete) has already fired
    // and settled before focus arrives.
    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });
    expect(header).toHaveAttribute('aria-expanded', 'true');
    expect(scrollSpy).not.toHaveBeenCalled();

    // Focus now arrives while already expanded: `setShowFound(true)` is a
    // no-op, so onAnimationComplete never re-fires — only the rAF fallback
    // can resolve the pending scroll.
    await act(async () => {
      rerender(
        <AutopilotCard
          autopilot={autopilot}
          {...defaultProps}
          focused
          focusedJobUrl={jobUrl}
          onFocusHandled={onFocusHandled}
        />
      );
    });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    const instance = scrollSpy.mock.instances[0];
    if (!instance) throw new Error('scrollIntoView was not called');
    expect(instance).toBe(row);
    expect(onFocusHandled).toHaveBeenCalledTimes(1);

    rafSpy.mockRestore();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Persisted run-outcome badge (failed / completedWithErrors / interrupted)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — run-status badge', () => {
  it('renders the failed badge (red) when runStatus is failed', () => {
    renderCard(withRunStatus('failed'));
    expect(screen.getByText('autopilot.badge.failed')).toBeInTheDocument();
  });

  it('renders the partial-results badge when runStatus is completedWithErrors', () => {
    renderCard(withRunStatus('completedWithErrors'));
    expect(screen.getByText('autopilot.badge.completedWithErrors')).toBeInTheDocument();
  });

  it('renders the interrupted badge when runStatus is interrupted', () => {
    renderCard(withRunStatus('interrupted'));
    expect(screen.getByText('autopilot.badge.interrupted')).toBeInTheDocument();
  });

  it('renders NO badge for the happy completed status', () => {
    renderCard(withRunStatus('completed'));
    expect(
      screen.queryByText(/autopilot\.badge\.(failed|completedWithErrors|interrupted)/)
    ).not.toBeInTheDocument();
  });

  it('renders NO badge for an unknown/future status (graceful fallback, never a raw enum)', () => {
    renderCard(withRunStatus('someFutureStatus'));
    expect(
      screen.queryByText(/autopilot\.badge\.(failed|completedWithErrors|interrupted)/)
    ).not.toBeInTheDocument();
    // The raw enum value must never leak into the DOM.
    expect(screen.queryByText('someFutureStatus')).not.toBeInTheDocument();
  });

  it('hides the badge while a run is in progress', () => {
    renderCard(withRunStatus('failed'), { runState: 'scraping' });
    expect(screen.queryByText('autopilot.badge.failed')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Needs-configuration guard (PR B carry-over 2) + badge hover explainers
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — needs-configuration guard', () => {
  it('a failed run where every board was merely skipped shows a neutral needs-config badge, not red failed', () => {
    renderCard(
      withRun('failed', [
        { board: 'aggregator', count: 0, skipped: 'needs-keys' },
        { board: 'linkedin', count: 0, skipped: 'needs-login' },
      ])
    );
    expect(screen.getByText('autopilot.badge.needsConfig')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.failed')).not.toBeInTheDocument();
  });

  it('a failed run with a real board error keeps the red failed badge (not needs-config)', () => {
    renderCard(
      withRun('failed', [
        { board: 'linkedin', count: 0, error: '429 Too Many Requests' },
        { board: 'aggregator', count: 0, skipped: 'needs-keys' },
      ])
    );
    expect(screen.getByText('autopilot.badge.failed')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.needsConfig')).not.toBeInTheDocument();
  });

  it('the needs-config badge carries a hover explainer', () => {
    renderCard(withRun('failed', [{ board: 'aggregator', count: 0, skipped: 'needs-keys' }]));
    expect(screen.getByText('autopilot.badge.needsConfigHint')).toBeInTheDocument();
  });

  it('the partial-results badge carries a hover explainer', () => {
    renderCard(withRun('completedWithErrors', [{ board: 'linkedin', count: 0, error: 'boom' }]));
    expect(screen.getByText('autopilot.badge.completedWithErrorsHint')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Persisted per-board chip strip — survives the run ending
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — persisted per-board chips', () => {
  it('renders the last run per-board chips when not running', () => {
    renderCard(
      withRun('completedWithErrors', [
        { board: 'greenhouse', count: 4 },
        { board: 'linkedin', count: 0, error: 'blocked' },
      ])
    );
    expect(screen.getAllByTestId('chip').length).toBeGreaterThanOrEqual(2);
  });

  it('does NOT render persisted chips while a run is in progress (live log shown instead)', () => {
    renderCard(withRun('completed', [{ board: 'greenhouse', count: 4 }]), {
      runState: 'scraping',
    });
    expect(screen.queryAllByTestId('chip')).toHaveLength(0);
    // Asserted independently of the chips (not just implied by sharing one JSX
    // conditional) so a future refactor decoupling the two is still caught.
    expect(
      screen.queryByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).not.toBeInTheDocument();
  });

  it('shows an info button with a localized aria-label that reveals the chips when persisted summaries exist', () => {
    renderCard(
      withRun('completedWithErrors', [
        { board: 'greenhouse', count: 4 },
        { board: 'linkedin', count: 0, error: 'blocked' },
      ])
    );
    // The chips themselves stay in the DOM (behind the HoverPopover mock, which
    // renders trigger + content unconditionally) — the meaningful assertion is
    // that the on-demand trigger exists with a real, localized accessible name.
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toBeInTheDocument();
    expect(screen.getAllByTestId('chip').length).toBeGreaterThanOrEqual(2);
  });

  it('does NOT render the info button when there are no persisted summaries', () => {
    renderCard(makeAutopilot());
    expect(
      screen.queryByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).not.toBeInTheDocument();
  });

  it('escalates the info trigger to the degraded tone when a board is merely skipped beside a succeeding one, even though no colored badge fires', () => {
    // Plain `completed` + one skipped board: `RUN_STATUS_BADGE` has no entry
    // for `completed`, so no colored badge renders at all — the info
    // trigger's own tone is the ONLY surviving "something's off" signal.
    renderCard(
      withRun('completed', [
        { board: 'xing', count: 0, skipped: 'needs-login' },
        { board: 'linkedin', count: 5 },
      ])
    );
    expect(screen.queryByText('autopilot.badge.failed')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.completedWithErrors')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.needsConfig')).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toHaveAttribute('data-degraded', 'true');
  });

  it('keeps the resting (non-degraded) tone when every board succeeded', () => {
    renderCard(withRun('completed', [{ board: 'linkedin', count: 5 }]));
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toHaveAttribute('data-degraded', 'false');
  });

  it('does NOT escalate for an informational location note alone (no cry-wolf amber)', () => {
    renderCard(withRun('completed', [{ board: 'linkedin', count: 5, note: 'broadened:de' }]));
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toHaveAttribute('data-degraded', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Cross-board clustering (ADR-029) — one rendered row per cluster
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — cross-board clustering', () => {
  it('renders one row per cluster — the non-canonical member is hidden', async () => {
    const user = userEvent.setup();
    const jobs: AutopilotFoundJob[] = [
      {
        title: 'Canonical role',
        company: 'Acme',
        url: 'https://a.com/1',
        foundAt: 0,
        clusterCanonical: true,
      },
      {
        title: 'Hidden duplicate',
        company: 'Acme',
        url: 'https://b.com/2',
        foundAt: 0,
        clusterCanonical: false,
      },
    ];
    renderCard(makeAutopilot(jobs));

    const headerDiv = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(headerDiv);

    // Only the canonical member is listed; the found-count reflects clusters (1).
    expect(screen.getByText('Canonical role')).toBeInTheDocument();
    expect(screen.queryByText('Hidden duplicate')).not.toBeInTheDocument();
    expect(screen.getByText('autopilot.foundJobs · 1')).toBeInTheDocument();
  });

  it('always shows unclustered (legacy) rows — no cluster annotation', async () => {
    const user = userEvent.setup();
    renderCard(
      makeAutopilot([{ title: 'Legacy role', company: 'Acme', url: 'https://a.com/1', foundAt: 0 }])
    );

    const headerDiv = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(headerDiv);

    expect(screen.getByText('Legacy role')).toBeInTheDocument();
    expect(screen.getByText('autopilot.foundJobs · 1')).toBeInTheDocument();
  });

  it('split action fires markNotDuplicate with memberKey, otherKeys AND autopilotId', async () => {
    const user = userEvent.setup();
    const job: AutopilotFoundJob = {
      title: 'Clustered role',
      company: 'Acme',
      url: 'https://a.com/1',
      foundAt: 0,
      clusterCanonical: true,
      clusterId: 'k1',
      clusterMembers: [
        { key: 'k1', board: 'linkedin', url: 'https://a.com/1' },
        { key: 'k2', board: 'indeed', url: 'https://b.com/2' },
      ],
    };
    // makeAutopilot fixes _id: 'ap-1' — the autopilotId the split must carry.
    renderCard(makeAutopilot([job]));

    // Expand the found-jobs panel so the cluster sub-row (with the split) mounts.
    const headerDiv = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(headerDiv);

    await user.click(screen.getByTestId(TEST_IDS.jobs.clusterSplitButton));

    expect(mockSplitMutate).toHaveBeenCalledTimes(1);
    const arg = mockSplitMutate.mock.calls[0]?.[0] as
      { memberKey: string; otherKeys: string[]; autopilotId?: string } | undefined;
    // memberKey = canonical key; otherKeys = the rest; autopilotId scopes the
    // per-record recompute (ADR-029 §h — only this call site sends it).
    expect(arg).toEqual({ memberKey: 'k1', otherKeys: ['k2'], autopilotId: 'ap-1' });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// postedAt date chip — display precedent: same helper+namespace the Jobs page
// uses (PostingListItem/index.tsx:121-122); absolute-time title tooltip mirrors
// ApplicationRow:231. Several boards ship no publish date, so absence must
// render nothing, not "NaN ago".
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — postedAt date chip', () => {
  it('renders the relative time + an absolute-time title tooltip for a dated job', async () => {
    const postedAt = Date.now() - 5 * 60_000; // 5 minutes ago
    const job = { ...makeJob('https://example.com/job/dated'), postedAt };
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    // The identity t() mock echoes the resolved i18n key back verbatim.
    const chip = screen.getByText(/jobs\.timeMinutesAgo/);
    expect(chip).toHaveAttribute('title', new Date(postedAt).toLocaleString());
  });

  it('renders no chip at all when postedAt is absent (board ships no publish date)', async () => {
    const job = makeJob('https://example.com/job/undated'); // no postedAt
    renderCard(makeAutopilot([job]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await act(async () => {
      header.click();
    });

    expect(screen.queryByText(/jobs\.time/)).not.toBeInTheDocument();
  });

  // Regression (CodeRabbit round 1): `job.postedAt && (...)` is the classic
  // 0-&&-JSX footgun — a `postedAt: 0` (epoch) job would render a bare stray
  // "0" text node instead of the chip, AND it disagreed with
  // `sortFoundJobsByDate`, which already treats 0 as dated via
  // `typeof === 'number'`. One presence contract now covers both the render
  // guard and the sort banding.
  it('treats postedAt: 0 as dated — chip renders (no stray "0" text), and the row sorts in the dated band', async () => {
    const user = userEvent.setup();
    const epochJob = { ...makeJob('https://example.com/job/epoch'), postedAt: 0 };
    const undatedJob = makeJob('https://example.com/job/no-date');
    renderCard(makeAutopilot([undatedJob, epochJob]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);

    // Display half of the contract: the JSX branch renders (a real titled
    // <span>), not the OLD `job.postedAt && (...)` guard's failure mode —
    // `0 && (<span>…</span>)` short-circuits to the bare number `0`, which
    // React renders as a stray "0" text node instead of the chip. The
    // relative-time TEXT itself is a separate, pre-existing concern
    // (`useFormatRelativeTime`'s own `if (!timestamp)` falsy-check also
    // treats 0 as absent — out of scope here; `title` is computed straight
    // from `job.postedAt`, not through that hook, so it's still a precise
    // signal that this is the real chip, not the footgun's stray digit).
    const epochRow = document.querySelector('[data-job-url="https://example.com/job/epoch"]');
    const chip = epochRow?.querySelector('span[title]');
    expect(chip).not.toBeNull();
    expect(chip).toHaveAttribute('title', new Date(0).toLocaleString());
    // The footgun's tell: no bare "0" text node anywhere in the row.
    expect(
      within(epochRow as HTMLElement).queryByText('0', { exact: true })
    ).not.toBeInTheDocument();

    // Sort half of the contract: the epoch job must band as DATED (ahead of
    // the undated one), not fall through to the undated/trailing band.
    await user.click(screen.getByRole('button', { name: 'jobs.sortNewest' }));
    const order = Array.from(document.querySelectorAll('[data-job-url]')).map((el) =>
      el.getAttribute('data-job-url')
    );
    expect(order).toEqual(['https://example.com/job/epoch', 'https://example.com/job/no-date']);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// sortFoundJobsByDate — pure comparator: banding, tiebreak, non-mutation.
// Tested directly (not through the component) so the ADR-020 mutation
// invariant is pinned precisely: reverting `[...jobs].sort(...)` to an
// in-place `jobs.sort(...)` must turn the non-mutation assertions red;
// dropping the dated/undated banding must turn the banding assertions red.
// ─────────────────────────────────────────────────────────────────────────────

describe('sortFoundJobsByDate', () => {
  const dated = (url: string, postedAt: number): AutopilotFoundJob => ({
    title: url,
    company: 'Acme',
    url,
    foundAt: 0,
    postedAt,
  });
  const undated = (url: string): AutopilotFoundJob => ({
    title: url,
    company: 'Acme',
    url,
    foundAt: 0,
  });

  it('bands dated jobs before undated jobs regardless of input order (sortBy="newest")', () => {
    const input = [undated('u1'), dated('d1', 1000), undated('u2'), dated('d2', 2000)];
    const result = sortFoundJobsByDate(input, 'newest');
    expect(result.map((j) => j.url)).toEqual(['d2', 'd1', 'u1', 'u2']);
  });

  // A `postedAt ?? 0` fallback (instead of a real dated/undated branch) would
  // pass the "newest" banding case above by accident — timestamp 0 sorts last
  // in a descending comparator anyway — but breaks exactly here: ascending
  // "oldest" would sort the undated 0-fallback rows to the FRONT, not the
  // trailing band. This is the case that actually needs the explicit banding.
  it('bands dated jobs before undated jobs regardless of input order (sortBy="oldest")', () => {
    const input = [undated('u1'), dated('d1', 1000), undated('u2'), dated('d2', 2000)];
    const result = sortFoundJobsByDate(input, 'oldest');
    expect(result.map((j) => j.url)).toEqual(['d1', 'd2', 'u1', 'u2']);
  });

  // Pinpoint case for the CodeRabbit round-1 finding: `postedAt: 0` (epoch)
  // must band as DATED, matching the render guard's `typeof === 'number'`
  // contract — a falsy-0 check anywhere in this pipeline would sink it into
  // the undated/trailing band instead.
  it("treats postedAt: 0 as dated, not undated (shares the render guard's typeof contract)", () => {
    const input = [undated('u1'), dated('epoch', 0)];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['epoch', 'u1']);
  });

  it('orders the dated band newest-first for sortBy="newest"', () => {
    const input = [dated('a', 1000), dated('b', 3000), dated('c', 2000)];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['b', 'c', 'a']);
  });

  it('orders the dated band oldest-first for sortBy="oldest"', () => {
    const input = [dated('a', 1000), dated('b', 3000), dated('c', 2000)];
    expect(sortFoundJobsByDate(input, 'oldest').map((j) => j.url)).toEqual(['a', 'c', 'b']);
  });

  it('tiebreaks equal postedAt values by url, for a deterministic order across renders', () => {
    const input = [dated('zzz', 1000), dated('aaa', 1000)];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['aaa', 'zzz']);
  });

  it('tiebreaks two undated jobs by url', () => {
    const input = [undated('zzz'), undated('aaa')];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['aaa', 'zzz']);
  });

  it('does NOT mutate the input array (ADR-020: the persisted order feeds AI-note recipient selection)', () => {
    const input = [dated('b', 3000), dated('a', 1000), undated('c')];
    const originalOrder = input.map((j) => j.url);

    const result = sortFoundJobsByDate(input, 'newest');

    expect(input.map((j) => j.url)).toEqual(originalOrder); // input order untouched
    expect(result).not.toBe(input); // a fresh array was returned, not the input reordered in place
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// AutopilotCard — foundJobs honors its OWN local per-card sort state (owner
// correction: sort is per-autopilot, not a shared session-store field — two
// expanded cards must be sortable independently). The Dropdown lives in the
// found-jobs panel header; option buttons are the mocked `@ajh/ui` Dropdown
// above.
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — foundJobs honors its own per-card sort', () => {
  /** DOM render order via the row's existing `data-job-url` seam. */
  function domJobOrder(): (string | null)[] {
    return Array.from(document.querySelectorAll('[data-job-url]')).map((el) =>
      el.getAttribute('data-job-url')
    );
  }

  function mixedJobs(): AutopilotFoundJob[] {
    return [
      { ...makeJob('https://example.com/c'), postedAt: 1_000 },
      { ...makeJob('https://example.com/a'), postedAt: 3_000 },
      makeJob('https://example.com/b'), // undated
    ];
  }

  /** Expand the panel, then click the per-card Dropdown's "Newest" option. */
  async function expandAndSelectNewest(user: ReturnType<typeof userEvent.setup>) {
    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);
    await user.click(screen.getByRole('button', { name: 'jobs.sortNewest' }));
  }

  it("defaults to relevance (today's stored rank order) — no reordering", async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot(mixedJobs()));
    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    await user.click(header);

    expect(domJobOrder()).toEqual([
      'https://example.com/c',
      'https://example.com/a',
      'https://example.com/b',
    ]);
  });

  it('reorders newest-first (dated band leads, undated trails) once the user selects "Newest"', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot(mixedJobs()));
    await expandAndSelectNewest(user);

    expect(domJobOrder()).toEqual([
      'https://example.com/a', // postedAt 3000 — newest
      'https://example.com/c', // postedAt 1000
      'https://example.com/b', // undated — trailing band
    ]);
  });

  it('does NOT mutate ap.foundJobs after selecting "Newest" (ADR-020)', async () => {
    const user = userEvent.setup();
    const fixture = mixedJobs();
    const originalOrder = fixture.map((j) => j.url);

    renderCard(makeAutopilot(fixture));
    await expandAndSelectNewest(user);

    expect(fixture.map((j) => j.url)).toEqual(originalOrder);
  });

  // The acceptance bar for the per-card requirement: two cards rendered at
  // once must sort independently — selecting "Newest" on one must NOT affect
  // the other, which stays at its own default (relevance).
  it('two cards sort independently — card A "newest" leaves card B at "relevance"', async () => {
    const user = userEvent.setup();
    const jobsA: AutopilotFoundJob[] = [
      { ...makeJob('https://example.com/a-c'), postedAt: 1_000 },
      { ...makeJob('https://example.com/a-a'), postedAt: 3_000 },
    ];
    const jobsB: AutopilotFoundJob[] = [
      { ...makeJob('https://example.com/b-c'), postedAt: 1_000 },
      { ...makeJob('https://example.com/b-a'), postedAt: 3_000 },
    ];
    const autopilotA = makeAutopilot(jobsA);
    const autopilotB = { ...makeAutopilot(jobsB), _id: 'ap-2', name: 'Second Autopilot' };

    const { container } = render(
      <>
        <AutopilotCard autopilot={autopilotA} {...defaultProps} />
        <AutopilotCard autopilot={autopilotB} {...defaultProps} />
      </>
    );

    const headers = Array.from(container.querySelectorAll('[aria-expanded]'));
    expect(headers).toHaveLength(2);
    const [headerA, headerB] = headers;
    if (!headerA || !headerB) throw new Error('both card headers must be present');
    await user.click(headerA);
    await user.click(headerB);

    // Both cards' Dropdowns share the same accessible group name ("jobs.sort")
    // — scope to the FIRST one (card A, DOM/render order) so only its sort
    // changes.
    const groups = screen.getAllByRole('group', { name: 'jobs.sort' });
    expect(groups).toHaveLength(2);
    const [groupA] = groups;
    if (!groupA) throw new Error('card A sort group must be present');
    await user.click(within(groupA).getByRole('button', { name: 'jobs.sortNewest' }));

    const orderFor = (prefix: string) =>
      domJobOrder().filter((url) => url?.includes(prefix)) as string[];

    // Card A: reordered newest-first.
    expect(orderFor('/a-')).toEqual(['https://example.com/a-a', 'https://example.com/a-c']);
    // Card B: untouched — still its own stored (relevance) order.
    expect(orderFor('/b-')).toEqual(['https://example.com/b-c', 'https://example.com/b-a']);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Track B1 — board reliability is read LIVE, not off the stored record
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — board reliability', () => {
  it('badges a board using the CURRENT verdict, not one frozen into the record', async () => {
    const user = userEvent.setup();
    // The stored run summary carries NO health (it is stripped on persist) —
    // everything the badge shows must come from the live query.
    mockBoardHealth = new Map<string, unknown>([
      [
        'wwr',
        {
          status: 'failing',
          consecutiveFailures: 4,
          verifiedRuns: 9,
          failedRuns: 4,
          lastSuccessAt: Date.now() - 6 * 24 * 60 * 60 * 1000,
          failingSince: Date.now() - 5 * 24 * 60 * 60 * 1000,
        },
      ],
    ]);
    renderCard(withRun('completedWithErrors', [{ board: 'wwr', count: 0, error: 'HTTP 500' }]));

    await user.click(screen.getByLabelText('autopilot.boardResults.infoLabel'));
    expect(await screen.findByText(/health\.failingSince/)).toBeInTheDocument();
  });

  it('shows no reliability badge while the live verdict says the board is fine', async () => {
    const user = userEvent.setup();
    mockBoardHealth = new Map<string, unknown>([
      ['wwr', { status: 'healthy', consecutiveFailures: 0, verifiedRuns: 9, failedRuns: 0 }],
    ]);
    renderCard(withRun('completedWithErrors', [{ board: 'wwr', count: 0, error: 'HTTP 500' }]));

    await user.click(screen.getByLabelText('autopilot.boardResults.infoLabel'));
    // This run's own failure is still explained…
    expect(await screen.findByText(/HTTP 500/)).toBeInTheDocument();
    // …but nothing claims a standing outage.
    expect(screen.queryByText(/health\./)).toBeNull();
  });
});
