/**
 * Shared mocks + fixtures for the AutopilotCard suites.
 *
 * Strategy:
 *  - All service hooks and heavy sub-components are stubbed at module level.
 *  - motion/react AnimatePresence is shimmed so animated panels appear
 *    synchronously in jsdom (no CSS transitions).
 *  - useInteractions returns controlled data (`state`) so viewedUrls can be exercised.
 *  - usePersistJob and useOpenExternal are spies — tests assert call args.
 *  - The header div carries role="button" when foundJobs.length > 0.
 *
 * This module registers every `vi.mock` (hoisted above its own imports), so a
 * suite must import the card FROM HERE, never from `./index`.
 */

import React from 'react';
import { beforeEach, type Mock, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import type { Autopilot, AutopilotFoundJob, BoardScrapeSummary } from '@ajh/shared';

import type * as MatchBandModule from '@/lib/match-band';

import { AutopilotCard as CardUnderTest } from './index';

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
// boolean prop was passed — a naive echo would pass the suites even if the
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

vi.mock('@/lib/time', () => ({ timeAgo: () => '3 min ago' }));

vi.mock('@/lib/machines/autopilot-run.machine', () => ({
  RUN_STATE_LABEL: { idle: 'Idle', scraping: 'Scraping', ranking: 'Ranking', error: 'Error' },
}));

// ── service hooks — spies controlled per-test ─────────────────────────────────

export const mockOpenExternal: Mock = vi.fn().mockResolvedValue(undefined);
export const mockPersistJobAsync: Mock = vi.fn().mockResolvedValue(undefined);
export const mockSplitMutate: Mock = vi.fn();

export const state: {
  viewed: { url?: string }[];
  opened: { url?: string }[];
  /** Board id → live health, as `useBoardsHealth` returns it. */
  boardHealth: Map<string, unknown>;
} = { viewed: [], opened: [], boardHealth: new Map() };

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: mockOpenExternal, mutateAsync: mockOpenExternal }),
  usePersistJob: () => ({ mutateAsync: mockPersistJobAsync }),
  useMarkNotDuplicate: () => ({ mutate: mockSplitMutate, isPending: false }),
  useInteractions: (type: string) => ({ data: type === 'viewed' ? state.viewed : state.opened }),
  // Track B1 — the card reads the LIVE per-board reliability verdict rather than
  // taking it off the stored run record. Empty by default here; the health
  // suite overrides it.
  useBoardsHealth: () => ({ data: state.boardHealth }),
}));

// Cluster/agency chips are covered in their own suites; stubbed here so the
// fixtures (no cluster data) don't need extra provider wiring.
vi.mock('@/components/job/ClusterSourceChips', () => ({ ClusterSourceChips: () => null }));
vi.mock('@/components/job/AgencyChip', () => ({ AgencyChip: () => null }));

// ── fixtures ──────────────────────────────────────────────────────────────────

/** The card under test — a wrapper, so the (mock-hoisted) import is read at render time. */
export const AutopilotCard = (props: React.ComponentProps<typeof CardUnderTest>) => (
  <CardUnderTest {...props} />
);

export function makeAutopilot(foundJobs: AutopilotFoundJob[] = []): Autopilot {
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

export function makeJob(url = 'https://example.com/job/1', score?: number): AutopilotFoundJob {
  return { title: 'Software Engineer', company: 'Acme', url, foundAt: 0, score };
}

// Build an autopilot with a persisted run outcome (and, optionally, its per-board
// summaries so the chip strip + needs-configuration guard can be exercised). Takes
// a plain `string` so an unknown/future status can be exercised (the
// graceful-fallback path).
export function withRun(status: string, summaries?: BoardScrapeSummary[]): Autopilot {
  return {
    ...makeAutopilot(),
    runStatus: status as Autopilot['runStatus'],
    ...(summaries && { lastRunSummaries: summaries }),
  };
}

export const defaultProps: Pick<
  React.ComponentProps<typeof CardUnderTest>,
  'runState' | 'stepLogs' | 'onRun' | 'onTogglePause' | 'onEdit' | 'onDelete' | 'onApply'
> = {
  runState: 'idle',
  stepLogs: [],
  onRun: vi.fn(),
  onTogglePause: vi.fn(),
  onEdit: vi.fn(),
  onDelete: vi.fn(),
  onApply: vi.fn(),
};

export function renderCard(autopilot: Autopilot, extraProps = {}) {
  return render(<AutopilotCard autopilot={autopilot} {...defaultProps} {...extraProps} />);
}

/** The card's header — the element carrying `aria-expanded` (present when jobs exist). */
export const cardHeader = () => document.querySelector('[aria-expanded]') as HTMLElement;

/** Expand the found-jobs panel via the header. */
export async function expandCard() {
  await act(async () => {
    cardHeader().click();
  });
}

beforeEach(() => {
  mockOpenExternal.mockClear();
  mockPersistJobAsync.mockClear();
  mockSplitMutate.mockClear();
  state.viewed = [];
  state.opened = [];
  state.boardHealth = new Map<string, unknown>();
});
