/**
 * JobDetailPane — key={posting.id} remount guard.
 *
 * Regression test for the stale interaction-state bug: without key={posting.id}
 * on <DetailContent>, switching postings in split view keeps the same
 * usePostingActions instance alive, so job A's saved/viewed state leaks into B.
 *
 * Strategy:
 *  - usePostingActions is NOT mocked — the real hook runs so useState's lazy
 *    initializer executes on each mount (which is what key= ensures).
 *  - All IPC services are stubbed (no Tauri bridge in tests).
 *  - Render posting A (bookmarked → saved=true → shows "jobs.view" button).
 *  - Rerender with posting B (no interactions → saved=false → shows "applications.save" button).
 *  - Assert B shows the Save button, NOT the View button (no state leak from A).
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
    disabled?: boolean;
    loading?: boolean;
    variant?: string;
    title?: string;
  }) => (
    <div role="button" onClick={onClick}>
      {children}
    </div>
  ),
  EmptyState: ({ title }: { title: string }) => <div data-testid="empty-state">{title}</div>,
  SourceBadge: () => null,
  Tag: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  transition: { fast: {} },
  useNotification: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}));

// ── router ────────────────────────────────────────────────────────────────────

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
}));

// ── RowMatchScore ─────────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/RowMatchScore', () => ({
  RowMatchScore: () => null,
}));

// ── session-store ─────────────────────────────────────────────────────────────

vi.mock('@/store/session-store', () => ({
  useSessionStore: () => ({ setApplicationApply: vi.fn() }),
}));

// ── match-score provider ──────────────────────────────────────────────────────

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: () => ({ score: undefined }),
}));

// ── match-level util ──────────────────────────────────────────────────────────

vi.mock('@/lib/match-level', () => ({
  scoreToLevel: () => null,
}));

// ── IPC services — all mutations are no-ops ───────────────────────────────────

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  usePersistJob: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useResolveJobUrl: () => ({
    data: undefined,
    isLoading: false,
    isFetching: false,
    isFetched: false,
    isError: false,
    refetch: vi.fn().mockResolvedValue(undefined),
  }),
  useUpdatePostingDescription: () => ({ mutateAsync: vi.fn().mockResolvedValue(false) }),
  useInvalidateMatchBatch: () => vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/services/use-applications', () => ({
  useSaveFromPosting: () => ({
    mutateAsync: vi.fn().mockResolvedValue({ id: 'app-1' }),
    isPending: false,
  }),
}));

// ── component under test ──────────────────────────────────────────────────────
// usePostingActions is intentionally NOT mocked — the real hook runs.

import type { Posting } from '@/features/jobs/types';

import { JobDetailPane } from './index';

// ── fixtures ──────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// key={posting.id} remount — no interaction state leak across jobs
// ─────────────────────────────────────────────────────────────────────────────

describe('JobDetailPane — key remount prevents interaction-state leak', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows Save (not View) for job B after switching from bookmarked job A', async () => {
    // Job A is already bookmarked — its interactions seed saved=true in usePostingActions.
    const postingA = makePosting('job-a', {
      interactions: [
        {
          interactionType: 'bookmarked',
          jobId: 'job-a',
          timestamp: 0,
          title: 'T',
          company: 'C',
          url: 'u',
          source: 's',
        },
      ],
    });
    // Job B has no interactions — saved should be false.
    const postingB = makePosting('job-b');

    const { rerender } = render(
      <JobDetailPane posting={postingA} formatRelativeTime={formatRelativeTime} />
    );

    // Sanity: job A shows the "View" button (saved=true → button text is jobs.view).
    expect(screen.getByText('jobs.view')).toBeInTheDocument();
    expect(screen.queryByText('applications.save')).not.toBeInTheDocument();

    // Switch to job B — the key={posting.id} remount resets usePostingActions.
    await act(async () => {
      rerender(<JobDetailPane posting={postingB} formatRelativeTime={formatRelativeTime} />);
    });

    // Job B must show "Save", NOT the stale "View" from job A.
    expect(screen.getByText('applications.save')).toBeInTheDocument();
    expect(screen.queryByText('jobs.view')).not.toBeInTheDocument();
  });

  it('shows applied badge for job A but NOT for job B after switching', async () => {
    // Job A has the applied interaction — its badge must NOT leak into job B.
    const postingA = makePosting('job-a', {
      interactions: [
        {
          interactionType: 'applied',
          jobId: 'job-a',
          timestamp: 0,
          title: 'T',
          company: 'C',
          url: 'u',
          source: 's',
        },
      ],
    });
    // Job B has no interactions at all.
    const postingB = makePosting('job-b');

    const { rerender } = render(
      <JobDetailPane posting={postingA} formatRelativeTime={formatRelativeTime} />
    );

    // Sanity: job A shows the "applied" badge.
    expect(screen.getByText('jobs.applied')).toBeInTheDocument();

    // Switch to job B — key={posting.id} remounts DetailContent, resetting
    // usePostingActions' lazy useState. B has no applied interaction so the badge
    // must not appear (unlike 'viewed' which is added by the viewed-on-mount effect).
    await act(async () => {
      rerender(<JobDetailPane posting={postingB} formatRelativeTime={formatRelativeTime} />);
    });

    // The applied badge must NOT bleed from job A to job B.
    expect(screen.queryByText('jobs.applied')).not.toBeInTheDocument();
  });
});
