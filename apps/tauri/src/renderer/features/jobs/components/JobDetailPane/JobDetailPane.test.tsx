/**
 * JobDetailPane — viewed-on-mount + EmptyState when no posting.
 *
 * Strategy:
 *  - The component is isolated: heavy deps (router, services, store) are stubbed.
 *  - usePostingActions is mocked so trackInteraction is a spy.
 *  - The viewed-on-mount effect uses a ref so re-renders with the SAME posting.id
 *    do not re-fire; switching posting.id fires exactly once more.
 *  - posting===null renders EmptyState with jobs.selectAJob.
 *
 * noUncheckedIndexedAccess: array accesses guarded throughout.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── motion/react ──────────────────────────────────────────────────────────────

vi.mock('motion/react', () => ({
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
  Bookmark: () => null,
  Briefcase: () => null,
  CircleCheck: () => null,
  Copy: () => null,
  ExternalLink: () => null,
  Eye: () => null,
  Loader2: () => null,
  MapPin: () => null,
  RefreshCw: () => null,
  Save: () => null,
  Wand2: () => null,
}));

// ── @ajh/ui ───────────────────────────────────────────────────────────────────

vi.mock('@ajh/ui', () => ({
  ActionMenu: () => null,
  Button: ({
    children,
    onClick,
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    className?: string;
    variant?: string;
    title?: string;
    disabled?: boolean;
    loading?: boolean;
  }) => (
    <div role="button" onClick={onClick}>
      {children}
    </div>
  ),
  EmptyState: ({
    title,
    icon: _icon,
  }: {
    title: string;
    icon?: React.ElementType;
    className?: string;
  }) => <div data-testid="empty-state">{title}</div>,
  SourceBadge: () => null,
  Tag: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  transition: { fast: {} },
}));

// ── RowMatchScore ─────────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/RowMatchScore', () => ({
  RowMatchScore: () => null,
}));

// ── @ajh/shared ───────────────────────────────────────────────────────────────

vi.mock('@ajh/shared', () => ({
  AGGREGATOR_BOARD_ID: 'aggregator',
}));

// ── useResolveJobUrl — vi.fn() so tests can override per-call ────────────────

const mockRefetch = vi.fn().mockResolvedValue(undefined);

/** Default stub: idle — not fetching, no data yet, no error. */
function idleStub() {
  return {
    data: undefined,
    isLoading: false,
    isFetching: false,
    isFetched: false,
    isError: false,
    refetch: mockRefetch,
  };
}

const mockUseResolveJobUrl = vi.fn().mockReturnValue(idleStub());

const mockUpdateDescMutateAsync = vi.fn().mockResolvedValue(false);
const mockInvalidateMatchBatch = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useResolveJobUrl: (...args: unknown[]) => mockUseResolveJobUrl(...args),
  useUpdatePostingDescription: () => ({ mutateAsync: mockUpdateDescMutateAsync }),
  useInvalidateMatchBatch: () => mockInvalidateMatchBatch,
}));

// ── trackInteraction spy — reset per test ─────────────────────────────────────

const mockTrackInteraction = vi.fn().mockResolvedValue(undefined);

vi.mock('@/features/jobs/hooks/usePostingActions', () => ({
  usePostingActions: () => ({
    has: () => false,
    trackInteraction: mockTrackInteraction,
    handleOpen: vi.fn(),
    handleCopyLink: vi.fn(),
    handleTailor: vi.fn(),
    handleView: vi.fn(),
    handleSave: vi.fn(),
    saved: false,
    pending: false,
  }),
}));

// ── component under test ──────────────────────────────────────────────────────

import type { Posting } from '@/features/jobs/types';

import { JobDetailPane } from './index';

// ── fixture ───────────────────────────────────────────────────────────────────

function makePosting(id: string, overrides: Partial<Posting> = {}): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/job/${id}`,
    title: `Job ${id}`,
    company: 'Acme',
    description: 'A great role.',
    capturedAt: 0,
    ...overrides,
  };
}

const formatRelativeTime = () => '2d ago';

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockTrackInteraction.mockClear();
  mockRefetch.mockClear();
  mockUpdateDescMutateAsync.mockClear();
  mockInvalidateMatchBatch.mockClear();
  mockUseResolveJobUrl.mockReturnValue(idleStub());
});

// ─────────────────────────────────────────────────────────────────────────────
// EmptyState when posting is null
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — null posting', () => {
  it('renders EmptyState with jobs.selectAJob when posting is null', () => {
    render(<JobDetailPane posting={null} formatRelativeTime={formatRelativeTime} />);
    expect(screen.getByTestId('empty-state')).toHaveTextContent('jobs.selectAJob');
  });

  it('does not call trackInteraction when posting is null', () => {
    render(<JobDetailPane posting={null} formatRelativeTime={formatRelativeTime} />);
    expect(mockTrackInteraction).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// viewed-on-mount — keyed by posting.id
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — viewed-on-mount', () => {
  it('calls trackInteraction("viewed") exactly once when a posting is displayed', async () => {
    await act(async () => {
      render(
        <JobDetailPane posting={makePosting('job-1')} formatRelativeTime={formatRelativeTime} />
      );
    });
    expect(mockTrackInteraction).toHaveBeenCalledTimes(1);
    expect(mockTrackInteraction).toHaveBeenCalledWith('viewed');
  });

  it('does NOT re-fire when re-rendered with the same posting.id', async () => {
    const posting = makePosting('job-1');
    const { rerender } = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );

    await act(async () => {
      // Re-render with a new object reference but same id — effect must not re-fire.
      rerender(
        <JobDetailPane
          posting={{ ...posting, title: 'Updated title' }}
          formatRelativeTime={formatRelativeTime}
        />
      );
    });

    expect(mockTrackInteraction).toHaveBeenCalledTimes(1);
  });

  it('fires again when posting.id changes to a different value', async () => {
    const { rerender } = render(
      <JobDetailPane posting={makePosting('job-1')} formatRelativeTime={formatRelativeTime} />
    );

    await act(async () => {
      rerender(
        <JobDetailPane posting={makePosting('job-2')} formatRelativeTime={formatRelativeTime} />
      );
    });

    // Once on mount (job-1), once on id switch (job-2).
    expect(mockTrackInteraction).toHaveBeenCalledTimes(2);
    const calls = mockTrackInteraction.mock.calls;
    expect(calls[0]?.[0]).toBe('viewed');
    expect(calls[1]?.[0]).toBe('viewed');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// useResolveJobUrl fallback — empty description triggers on-demand fetch
// Loading state now gates on isFetching (not isLoading) so any refetch,
// including the manual retry, drives the spinner consistently.
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — useResolveJobUrl fallback', () => {
  it('shows loading text when description is empty and resolve is in-flight', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: true,
      isFetching: true,
      isFetched: false,
      refetch: mockRefetch,
    });

    const posting = { ...makePosting('job-load'), description: '' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();

    // a11y: the loading container must announce itself to screen readers via
    // role="status" (an implicit aria-live="polite" region) and signal that the
    // region is actively updating with aria-busy="true". A future refactor that
    // strips these attributes would silently regress AT users.
    // Note: there is also a visually-hidden AT sentinel with role="status" —
    // pick the one with aria-busy="true" (the spinner container).
    const statusEls = screen.getAllByRole('status');
    const busyEl = statusEls.find((el) => el.getAttribute('aria-busy') === 'true');
    expect(busyEl).toBeInTheDocument();
    expect(busyEl).toHaveAttribute('aria-busy', 'true');
  });

  it('shows fetched description when resolve data arrives and original description was empty', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Fetched job description text' },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      refetch: mockRefetch,
    });

    const posting = { ...makePosting('job-fetched'), description: '' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('Fetched job description text')).toBeInTheDocument();
  });

  it('shows loading text when description is whitespace-only', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: true,
      isFetching: true,
      isFetched: false,
      refetch: mockRefetch,
    });

    const posting = { ...makePosting('job-ws'), description: '   ' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();
  });

  it('uses the posting description directly (no loading) when description is non-empty on a non-aggregator source', async () => {
    // Default stub: isFetching=false. Component should not show the loading state.
    const posting = { ...makePosting('job-has-desc'), description: 'Original description' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('Original description')).toBeInTheDocument();
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Aggregator short-description gate — fires resolve for short aggregator snippets
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — aggregator short-description gate', () => {
  it('shows loading state when aggregator posting has a short snippet and resolve is in-flight', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: true,
      isFetching: true,
      isFetched: false,
      refetch: mockRefetch,
    });

    // A snippet shorter than SHORT_DESCRIPTION_CHARS (700) on the aggregator board.
    const posting = makePosting('agg-loading', {
      source: 'aggregator',
      description: 'Short Adzuna snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();
    // Resolve must have been called with enabled=true for this posting's URL.
    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, true);
  });

  it('does NOT fire resolve for a non-aggregator posting with a short description', async () => {
    const posting = makePosting('non-agg-short', {
      source: 'linkedin',
      description: 'Short linkedin snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // enabled=false → resolve called with false (gate not open).
    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, false);
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
    expect(screen.getByText('Short linkedin snippet.')).toBeInTheDocument();
  });

  it('does NOT fire resolve for an aggregator posting whose description exceeds the threshold', async () => {
    // Build a description longer than 700 chars.
    const longDesc = 'x'.repeat(750);
    const posting = makePosting('agg-long', { source: 'aggregator', description: longDesc });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, false);
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Keep-longer merge — never degrade the pane below the original snippet
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — keep-longer merge', () => {
  it('shows the resolved description when it is longer than the snippet', async () => {
    const snippet = 'Short Adzuna snippet.';
    const fullDesc = 'This is the full job description fetched from the target page.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: fullDesc },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-resolved', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText(fullDesc)).toBeInTheDocument();
    expect(screen.queryByText(snippet)).not.toBeInTheDocument();
  });

  it('keeps the original snippet when resolve returns something shorter (e.g. 429/generic)', async () => {
    const snippet = 'Original Adzuna snippet that is longer than the resolved result.';
    const degraded = 'Tiny.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: degraded },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-degraded', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // The snippet must survive — the degraded result must not replace it.
    expect(screen.getByText(snippet)).toBeInTheDocument();
    expect(screen.queryByText(degraded)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// "Load full description" button — visibility + click behaviour
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — load full description button', () => {
  it('shows the button when aggregator posting has a short snippet and resolve has not fetched', async () => {
    // idle: isFetching=false, no data — button must be visible.
    const posting = makePosting('agg-btn', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
  });

  it('shows the button when resolve settled but returned nothing longer than the snippet', async () => {
    const snippet = 'Original snippet that resolve could not beat.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Tiny.' },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-btn-retry', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
  });

  it('hides the button while resolve is in-flight (isFetching=true)', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: true,
      isFetching: true,
      isFetched: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-btn-fetching', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // Loading indicator is shown; the retry button must NOT appear alongside it.
    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();
    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
  });

  it('hides the button once resolve returns a longer description', async () => {
    const snippet = 'Short snippet.';
    const fullDesc = 'This is the much longer full job description from the redirect target.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: fullDesc },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-btn-hidden', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
    expect(screen.getByText(fullDesc)).toBeInTheDocument();
  });

  it('does NOT show the button for non-aggregator postings with a full description', async () => {
    const posting = makePosting('non-agg-full', {
      source: 'linkedin',
      description: 'A complete job description.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
  });

  it('clicking the button calls refetch()', async () => {
    // idle stub: button visible.
    const posting = makePosting('agg-btn-click', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    const btn = screen.getByText('jobs.loadFullDescription');
    expect(btn).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(btn);
    });

    expect(mockRefetch).toHaveBeenCalledTimes(1);
  });

  it('button is keyboard-reachable (role=button accessible)', async () => {
    const posting = makePosting('agg-btn-a11y', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // The @ajh/ui Button stub renders as role="button" — accessible by keyboard users.
    const btn = screen.getByRole('button', { name: /jobs\.loadFullDescription/i });
    expect(btn).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Error state — resolve failure shows hint and keeps retry button (blocker 7)
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — resolve error state', () => {
  it('shows error hint when resolve fails for an aggregator short-snippet posting', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-error', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // Error hint must be visible.
    expect(screen.getByText('jobs.descriptionLoadError')).toBeInTheDocument();
  });

  it('retry button remains visible alongside the error hint so the user can try again', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-error-retry', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // showLoadButton is still true (not fetching, not resolvedLonger) → button present.
    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
  });

  it('does NOT show error hint for a non-aggregator posting (resolve not triggered)', async () => {
    // isError=true but shouldResolve=false for a non-aggregator with a description.
    mockUseResolveJobUrl.mockReturnValue({
      ...idleStub(),
      isError: true,
    });

    const posting = makePosting('non-agg-no-error-hint', {
      source: 'linkedin',
      description: 'A full linkedin description.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // Error hint must NOT appear — the resolve gate was not triggered.
    expect(screen.queryByText('jobs.descriptionLoadError')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Re-score on open — updateDescription + match-batch invalidation (Part B)
// ─────────────────────────────────────────────────────────────────────────────

/** Resolved-longer stub used by re-score tests. */
function resolvedLongerStub(fullDesc: string) {
  return {
    data: { description: fullDesc },
    isLoading: false,
    isFetching: false,
    isFetched: true,
    isError: false,
    refetch: mockRefetch,
  };
}

describe('JobDetailPane — re-score on open', () => {
  it('invalidate fires AFTER updateDescription resolves (call-order invariant)', async () => {
    // Instrument call order so we can assert sequence, not just both-called.
    const callOrder: string[] = [];
    mockUpdateDescMutateAsync.mockImplementation(async () => {
      callOrder.push('update');
      return false;
    });
    mockInvalidateMatchBatch.mockImplementation(async () => {
      callOrder.push('invalidate');
    });

    const snippet = 'Short snippet.';
    const fullDesc = 'This is the much longer full job description fetched from the target page.';
    mockUseResolveJobUrl.mockReturnValue(resolvedLongerStub(fullDesc));

    const posting = makePosting('agg-rescore-order', {
      source: 'aggregator',
      description: snippet,
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    // Flush the .then() microtask so invalidate has run.
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockUpdateDescMutateAsync).toHaveBeenCalledWith({
      id: posting.id,
      description: fullDesc,
    });
    expect(mockInvalidateMatchBatch).toHaveBeenCalledTimes(1);
    // The key ordering assertion — invalidate must come AFTER update.
    expect(callOrder).toEqual(['update', 'invalidate']);
  });

  it('one-shot guard — re-render with same posting does NOT fire updateDescription again', async () => {
    const snippet = 'Short snippet.';
    const fullDesc = 'This is the much longer full job description fetched from the target page.';
    mockUseResolveJobUrl.mockReturnValue(resolvedLongerStub(fullDesc));

    const posting = makePosting('agg-rescore-once', {
      source: 'aggregator',
      description: snippet,
    });
    const { rerender } = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);

    // Re-render with same posting id — upgraded ref must block a second fire.
    await act(async () => {
      rerender(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('remount via new posting.id resets the guard and fires again for the new job', async () => {
    const fullDesc = 'This is the much longer full job description fetched from the target page.';
    mockUseResolveJobUrl.mockReturnValue(resolvedLongerStub(fullDesc));

    const postingA = makePosting('agg-rescore-a', {
      source: 'aggregator',
      description: 'Short A.',
    });
    const postingB = makePosting('agg-rescore-b', {
      source: 'aggregator',
      description: 'Short B.',
    });

    // Render posting A — fires once.
    const { rerender } = render(
      <JobDetailPane posting={postingA} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);

    // Switch to posting B — key={posting.id} remounts DetailContent, resetting
    // the upgraded ref so the guard fires again for job B.
    await act(async () => {
      rerender(<JobDetailPane posting={postingB} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(2);
    // Second call must be for posting B.
    expect(mockUpdateDescMutateAsync).toHaveBeenLastCalledWith({
      id: postingB.id,
      description: fullDesc,
    });
  });

  it('does NOT call updateDescription when the resolved description is NOT longer', async () => {
    const snippet = 'A full linkedin description with plenty of text.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Tiny.' },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('non-upgrade', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
    expect(mockInvalidateMatchBatch).not.toHaveBeenCalled();
  });

  it('does NOT call updateDescription for a non-aggregator posting (resolvedLonger always false)', async () => {
    const posting = makePosting('linkedin-full', {
      source: 'linkedin',
      description: 'A complete description.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
    expect(mockInvalidateMatchBatch).not.toHaveBeenCalled();
  });
});
