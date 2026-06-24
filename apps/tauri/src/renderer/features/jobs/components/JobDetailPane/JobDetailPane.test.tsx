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
  Save: () => null,
  Wand2: () => null,
}));

// ── @ajh/ui ───────────────────────────────────────────────────────────────────

vi.mock('@ajh/ui', () => ({
  ActionMenu: () => null,
  Button: ({ children, onClick }: { children?: React.ReactNode; onClick?: () => void }) => (
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

// ── useResolveJobUrl — vi.fn() so tests can override per-call ────────────────

const mockUseResolveJobUrl = vi.fn().mockReturnValue({ data: undefined, isLoading: false });

vi.mock('@/services', () => ({
  useResolveJobUrl: (...args: unknown[]) => mockUseResolveJobUrl(...args),
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

function makePosting(id: string): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/job/${id}`,
    title: `Job ${id}`,
    company: 'Acme',
    description: 'A great role.',
    capturedAt: 0,
  };
}

const formatRelativeTime = () => '2d ago';

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockTrackInteraction.mockClear();
  mockUseResolveJobUrl.mockReturnValue({ data: undefined, isLoading: false });
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
// useResolveJobUrl fallback — empty description triggers on-demand fetch (#6)
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — useResolveJobUrl fallback', () => {
  it('shows loading text when description is empty and resolve is in-flight', async () => {
    mockUseResolveJobUrl.mockReturnValue({ data: undefined, isLoading: true });

    const posting = { ...makePosting('job-load'), description: '' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();
  });

  it('shows fetched description when resolve data arrives and original description was empty', async () => {
    mockUseResolveJobUrl.mockReturnValue({
      data: { description: 'Fetched job description text' },
      isLoading: false,
    });

    const posting = { ...makePosting('job-fetched'), description: '' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('Fetched job description text')).toBeInTheDocument();
  });

  it('shows loading text when description is whitespace-only', async () => {
    mockUseResolveJobUrl.mockReturnValue({ data: undefined, isLoading: true });

    const posting = { ...makePosting('job-ws'), description: '   ' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('jobs.loadingDescription')).toBeInTheDocument();
  });

  it('uses the posting description directly (no loading) when description is non-empty', async () => {
    // mockUseResolveJobUrl default is { data: undefined, isLoading: false }.
    // The component should not call it with enabled=true when description exists.
    const posting = { ...makePosting('job-has-desc'), description: 'Original description' };
    await act(async () => {
      render(<JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />);
    });

    expect(screen.getByText('Original description')).toBeInTheDocument();
    expect(screen.queryByText('jobs.loadingDescription')).not.toBeInTheDocument();
  });
});
