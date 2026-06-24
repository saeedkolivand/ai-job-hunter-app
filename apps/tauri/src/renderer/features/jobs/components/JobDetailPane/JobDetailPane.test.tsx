/**
 * JobDetailPane — viewed-on-mount, markdown render, score-on-open, EmptyState.
 *
 * Strategy:
 *  - Heavy deps (router, services, store) are stubbed.
 *  - useMatchScores is stubbed so scoreJob is a spy.
 *  - The viewed-on-mount effect uses a ref so re-renders with the SAME posting.id
 *    do not re-fire; switching posting.id fires exactly once more.
 *  - posting===null renders EmptyState with jobs.selectAJob.
 *  - ReactMarkdown renders description text; we assert the text content appears.
 *  - scoreJob is called ONCE on open after description is ready; NOT called for the
 *    whole list; NOT called when description is still loading.
 *
 * noUncheckedIndexedAccess: array accesses guarded throughout.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

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
  JobDescription: ({ markdown }: { markdown: string; className?: string }) => (
    <div data-testid="job-description">{markdown}</div>
  ),
  SourceBadge: () => null,
  Tag: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  transition: { fast: {} },
}));

// ── RowMatchScore ─────────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/RowMatchScore', () => ({
  RowMatchScore: () => <span data-testid="row-match-score" />,
}));

// ── @ajh/shared ───────────────────────────────────────────────────────────────

vi.mock('@ajh/shared', () => ({
  AGGREGATOR_BOARD_ID: 'aggregator',
}));

// ── useMatchScores — scoreJob spy ─────────────────────────────────────────────

const mockScoreJob = vi.fn();

vi.mock('@/features/jobs/providers', () => ({
  useMatchScores: () => ({
    scoreJob: mockScoreJob,
    hasResume: true,
  }),
}));

// ── useResolveJobUrl ──────────────────────────────────────────────────────────

const mockRefetch = vi.fn().mockResolvedValue(undefined);

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

vi.mock('@/services', () => ({
  useResolveJobUrl: (...args: unknown[]) => mockUseResolveJobUrl(...args),
  useUpdatePostingDescription: () => ({ mutateAsync: mockUpdateDescMutateAsync }),
}));

// ── trackInteraction spy ──────────────────────────────────────────────────────

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
  mockScoreJob.mockClear();
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
// Markdown rendering — description text appears in DOM
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — description rendering', () => {
  // JobDescription is from @ajh/ui (stub renders raw markdown string as text).
  // These tests verify the correct markdown string is passed through; the
  // actual GFM rendering is tested in @ajh/ui's own tests.

  it('passes plain description to JobDescription', async () => {
    const posting = makePosting('md-plain', { description: 'This is a great role.' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    expect(screen.getByTestId('job-description')).toHaveTextContent('This is a great role.');
  });

  it('passes bold markdown to JobDescription', async () => {
    const posting = makePosting('md-bold', { description: '**Strong skill** required.' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    expect(screen.getByTestId('job-description')).toHaveTextContent('**Strong skill** required.');
  });

  it('passes heading markdown to JobDescription', async () => {
    const posting = makePosting('md-heading', {
      description: '## Requirements\n\nFive years of experience.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    expect(screen.getByTestId('job-description')).toHaveTextContent('Requirements');
  });

  it('passes list markdown to JobDescription', async () => {
    const posting = makePosting('md-list', { description: '- TypeScript\n- React\n- Rust' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    const el = screen.getByTestId('job-description');
    expect(el).toHaveTextContent('TypeScript');
    expect(el).toHaveTextContent('React');
    expect(el).toHaveTextContent('Rust');
  });

  it('passes link markdown to JobDescription (no live <a> in stub)', async () => {
    const posting = makePosting('md-link', { description: '[Apply here](https://example.com)' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    expect(screen.getByTestId('job-description')).toHaveTextContent('Apply here');
    // Stub does not render an <a> — GFM link-as-span behavior tested in @ajh/ui
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// viewed dwell — fires after 5s, not on mount; keyed by posting.id
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — viewed dwell timer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    mockTrackInteraction.mockClear();
  });

  it('does NOT call trackInteraction("viewed") before 5s have elapsed', async () => {
    await act(async () => {
      render(
        <JobDetailPane posting={makePosting('job-1')} formatRelativeTime={formatRelativeTime} />
      );
    });
    // Advance to just under the threshold — must not have fired yet.
    await act(async () => {
      vi.advanceTimersByTime(4999);
    });
    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');
  });

  it('calls trackInteraction("viewed") exactly once after 5s dwell', async () => {
    await act(async () => {
      render(
        <JobDetailPane posting={makePosting('job-1')} formatRelativeTime={formatRelativeTime} />
      );
    });
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(mockTrackInteraction).toHaveBeenCalledTimes(1);
    expect(mockTrackInteraction).toHaveBeenCalledWith('viewed');
  });

  it('does NOT re-fire when re-rendered with the same posting.id after dwell', async () => {
    const posting = makePosting('job-1');
    const { rerender } = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    mockTrackInteraction.mockClear();

    // Re-render with same id (e.g. title update) — timer must NOT restart/refire.
    await act(async () => {
      rerender(
        <JobDetailPane
          posting={{ ...posting, title: 'Updated title' }}
          formatRelativeTime={formatRelativeTime}
        />
      );
    });
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');
  });

  it('cancels and restarts timer when posting.id changes; fires once per job', async () => {
    const { rerender } = render(
      <JobDetailPane posting={makePosting('job-1')} formatRelativeTime={formatRelativeTime} />
    );
    // Switch jobs before the dwell fires — old timer cancels.
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    await act(async () => {
      rerender(
        <JobDetailPane posting={makePosting('job-2')} formatRelativeTime={formatRelativeTime} />
      );
    });
    // Advance 5s from job-2 mount — only job-2's timer fires.
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    // job-1's timer was cancelled; only job-2 fires.
    expect(mockTrackInteraction).toHaveBeenCalledTimes(1);
    expect(mockTrackInteraction).toHaveBeenCalledWith('viewed');
  });

  it('unmount cancels the pending timer — "viewed" is never tracked after unmount', async () => {
    // Render a posting and advance 4s (timer is pending, has not fired yet).
    const { unmount } = render(
      <JobDetailPane posting={makePosting('job-unmount')} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      vi.advanceTimersByTime(4000);
    });
    // No call yet — still within the 5s dwell.
    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');

    // Unmount before the timer fires — clearTimeout in the cleanup must cancel it.
    unmount();

    // Advance well past the threshold; the callback must NOT fire post-unmount.
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Score on open — scoreJob called once when description is ready
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — score on open', () => {
  it('calls scoreJob(posting.id) once when description is immediately available', async () => {
    const posting = makePosting('score-ready', { description: 'Full description text.' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
  });

  it('does NOT call scoreJob while description is still loading (resolve in-flight)', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: true,
      isFetching: true,
      isFetched: false,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('score-loading', { description: '' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // Description is empty + resolve in-flight → scoreJob must not fire yet.
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('does NOT score in the pre-fetch window (isFetched=false, isFetching=false)', async () => {
    // idleStub has isFetched=false — the query has not started yet.
    // Before the isFetched guard, resolveSettled would be true here (both flags false)
    // and scoring would fire on the snippet before the resolve query even begins.
    const posting = makePosting('score-prefetch', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    // isFetched=false → resolveSettled=false → scoreJob must NOT fire.
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('non-aggregator full description — scores immediately, no persist', async () => {
    const posting = makePosting('score-full', { description: 'Full description text.' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
    // No persist needed for already-full descriptions.
    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
  });

  it('resolve-not-longer — scores immediately, no persist', async () => {
    // Resolve returns a SHORTER or equal description → resolvedLonger=false.
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Shorter.' },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('score-not-longer', {
      source: 'aggregator',
      description: 'Original snippet that is longer than resolved.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
  });

  it('persist-then-score ORDER — update resolves before scoreJob fires on resolved-longer', async () => {
    // Track call order: 'update' pushed when persist resolves, 'score' pushed when scoreJob called.
    const callOrder: string[] = [];

    mockUpdateDescMutateAsync.mockImplementation(async () => {
      callOrder.push('update');
      return undefined;
    });
    mockScoreJob.mockImplementation((_id: string) => {
      callOrder.push('score');
    });

    const fullDesc = 'This is the full job description fetched from the redirect target.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: fullDesc },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('score-order', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    // Let the async persist + scoreJob chain settle.
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
    // Critical ordering assertion: update must precede score.
    expect(callOrder).toEqual(['update', 'score']);
  });

  it('persist failure is non-fatal — scoreJob still fires after updateDescription rejects', async () => {
    mockUpdateDescMutateAsync.mockRejectedValue(new Error('persist failed'));

    const fullDesc = 'Full description from resolve endpoint.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: fullDesc },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('score-persist-fail', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await Promise.resolve();
    });

    // scoreJob fires even though persist failed.
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
  });

  it('one-shot guard — re-render with same posting does NOT call scoreJob again', async () => {
    const posting = makePosting('score-once', { description: 'Full description.' });
    const { rerender } = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockScoreJob).toHaveBeenCalledTimes(1);

    await act(async () => {
      rerender(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // Guard ref blocks a second call.
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
  });

  it('remount via new posting.id resets the guard and calls scoreJob for the new job', async () => {
    const postingA = makePosting('score-a', { description: 'Description A.' });
    const postingB = makePosting('score-b', { description: 'Description B.' });

    const { rerender } = render(
      <JobDetailPane posting={postingA} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockScoreJob).toHaveBeenCalledTimes(1);

    // Switch job — key={posting.id} remounts DetailContent, resetting scoredRef.
    await act(async () => {
      rerender(<JobDetailPane posting={postingB} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockScoreJob).toHaveBeenCalledTimes(2);
    expect(mockScoreJob).toHaveBeenLastCalledWith(postingB.id);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// useResolveJobUrl fallback — empty description triggers on-demand fetch
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

  it('uses the posting description directly when description is non-empty on a non-aggregator source', async () => {
    const posting = { ...makePosting('job-has-desc'), description: 'Original description' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('Original description')).toBeInTheDocument();
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Aggregator short-description gate
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — aggregator short-description gate', () => {
  it('shows updating hint (not full loading state) when aggregator has a snippet and resolve is in-flight', async () => {
    // New behaviour: existing snippet text is rendered immediately; a small
    // "Updating…" hint appears inline while the full text is being fetched.
    // The full "Loading description…" spinner is only shown when there is NO text.
    mockUseResolveJobUrl.mockReturnValue({
      data: undefined,
      isLoading: true,
      isFetching: true,
      isFetched: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-loading', {
      source: 'aggregator',
      description: 'Short Adzuna snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    // Snippet is rendered immediately (no flash to full spinner)
    expect(screen.getByText('Short Adzuna snippet.')).toBeInTheDocument();
    // Inline updating hint shown while fetching
    expect(screen.getByText('jobs.updatingDescription')).toBeInTheDocument();
    // Full "loading" spinner NOT shown (that is reserved for when there is no text)
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
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

    expect(mockUseResolveJobUrl).toHaveBeenCalledWith(posting.url, false);
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
    expect(screen.getByText('Short linkedin snippet.')).toBeInTheDocument();
  });

  it('does NOT fire resolve for an aggregator posting whose description exceeds the threshold', async () => {
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
// Keep-longer merge
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

  it('keeps the original snippet when resolve returns something shorter', async () => {
    const snippet = 'Original Adzuna snippet that is longer than the resolved result.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Tiny.' },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      refetch: mockRefetch,
    });

    const posting = makePosting('agg-degraded', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText(snippet)).toBeInTheDocument();
    expect(screen.queryByText('Tiny.')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// "Load full description" button
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — load full description button', () => {
  it('shows the button when aggregator posting has a short snippet and resolve has not fetched', async () => {
    const posting = makePosting('agg-btn', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
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

    // Snippet is shown immediately; inline updating hint visible; full-load button hidden.
    expect(screen.getByText('Short snippet.')).toBeInTheDocument();
    expect(screen.getByText('jobs.updatingDescription')).toBeInTheDocument();
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

  it('button is keyboard-reachable (role=button accessible)', async () => {
    const posting = makePosting('agg-btn-a11y', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    const btn = screen.getByRole('button', { name: /jobs\.loadFullDescription/i });
    expect(btn).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Error state (blocker 7)
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

    expect(screen.getByText('jobs.descriptionLoadError')).toBeInTheDocument();
  });

  it('retry button remains visible alongside the error hint', async () => {
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

    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
  });

  it('does NOT show error hint for a non-aggregator posting', async () => {
    mockUseResolveJobUrl.mockReturnValue({ ...idleStub(), isError: true });

    const posting = makePosting('non-agg-no-error-hint', {
      source: 'linkedin',
      description: 'A full linkedin description.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.queryByText('jobs.descriptionLoadError')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// updateDescription persist — fires after resolve upgrades description
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — updateDescription persist on upgrade', () => {
  it('calls updateDescription once when the resolved description is longer', async () => {
    const snippet = 'Short snippet.';
    const fullDesc = 'This is the much longer full job description fetched from the target page.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: fullDesc },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('persist-upgrade', { source: 'aggregator', description: snippet });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockUpdateDescMutateAsync).toHaveBeenCalledWith({
      id: posting.id,
      description: fullDesc,
    });
  });

  it('one-shot guard — re-render does NOT call updateDescription again', async () => {
    const fullDesc = 'Much longer description than the snippet.';
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: fullDesc },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('persist-once', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    const { rerender } = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await act(async () => {
      await Promise.resolve();
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);

    await act(async () => {
      rerender(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('does NOT call updateDescription when the resolved description is NOT longer', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Tiny.' },
      isLoading: false,
      isFetching: false,
      isFetched: true,
      isError: false,
      refetch: mockRefetch,
    });

    const posting = makePosting('no-persist', {
      source: 'aggregator',
      description: 'A snippet longer than tiny.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// "Load full description" button click → refetch
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — load full description button calls refetch', () => {
  it('clicking the button calls resolved.refetch()', async () => {
    // idleStub: isFetched=false, isFetching=false — button is visible.
    // aggregator source + short snippet satisfies the showLoadButton gate.
    const posting = makePosting('btn-refetch', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    const btn = screen.getByRole('button', { name: /jobs\.loadFullDescription/i });
    expect(btn).toBeInTheDocument();

    await act(async () => {
      btn.click();
    });

    expect(mockRefetch).toHaveBeenCalledTimes(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RowMatchScore visibility — score renders in the detail header
// (score was removed from list rows; this guards it stays in the detail pane)
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — RowMatchScore renders in detail header', () => {
  it('renders RowMatchScore in the detail pane when a posting is open', async () => {
    const posting = makePosting('score-visible', { description: 'Full description.' });
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });
    expect(screen.getByTestId('row-match-score')).toBeInTheDocument();
  });

  it('does NOT render RowMatchScore when posting is null (empty state)', () => {
    render(<JobDetailPane posting={null} formatRelativeTime={formatRelativeTime} />);
    expect(screen.queryByTestId('row-match-score')).not.toBeInTheDocument();
  });
});
