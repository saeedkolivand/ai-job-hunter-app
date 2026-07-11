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
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { Autopilot, AutopilotFoundJob, BoardScrapeSummary } from '@ajh/shared';

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
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
    'aria-label'?: string;
    title?: string;
    'data-degraded'?: boolean;
  }) =>
    // Use createElement to avoid the JSXOpeningElement[name="button"] lint rule.
    // A native <button> is required so disabled + keyboard behavior are real.
    // `data-degraded` is forwarded (not the raw className) as the seam for the
    // amber-tone assertion — a data-* seam over a Tailwind class string, per the
    // jsdom-CSS-parsing lesson.
    React.createElement(
      'button',
      { onClick, 'aria-label': ariaLabel, title, disabled, 'data-degraded': dataDegraded },
      children
    ),
  ConfirmModal: () => null,
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
    }: {
      value: number;
      variant?: 'combined' | 'coverage';
      subtle?: boolean;
      muted?: boolean;
    }) => {
      const tier = actual.scoreTier(value, variant ?? 'combined').key;
      const isMutedStyle = Boolean(muted) || (Boolean(subtle) && tier !== 'High');
      return (
        <span
          data-testid="match-band"
          data-value={value}
          data-variant={variant ?? 'combined'}
          data-tier={tier}
          data-muted={isMutedStyle ? 'true' : 'false'}
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

    // The native hover hint (title) is present...
    expect(screen.getByTitle('autopilot.provisionalScoreHint')).toBeInTheDocument();
    // ...the "~" estimate prefix is visible...
    expect(screen.getByText('~')).toBeInTheDocument();
    // ...an always-present sr-only span carries the same hint for screen
    // readers (a `title` alone isn't reliably announced — TrustBadge precedent)...
    expect(screen.getByText(': autopilot.provisionalScoreHint')).toHaveClass('sr-only');
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
