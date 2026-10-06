/**
 * Shared harness for the JobDetailPane suites.
 *
 *  - Heavy deps (router, services, store) are stubbed by the `vi.mock` calls here,
 *    which are hoisted above the `./index` import below. Suites import the subject
 *    from this module (never `./index`), so the mocks always apply.
 *  - useMatchScores is stubbed so `mockScoreJob` is a spy.
 *  - `usePostingActions` is stubbed so `mockTrackInteraction` is a spy.
 */

import React, { useEffect, useRef } from 'react';
import { type Mock, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import type { Posting } from '@/features/jobs/types';

import { JobDetailPane as Subject } from './index';

/** The component under test, as loaded after this module's mocks registered. */
export const JobDetailPane = Subject;

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

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
  resolveTransition: (t: unknown) => t,
  useNotification: () => mockNotify,
  variants: {
    fadeSlideUp: { initial: {}, animate: {}, exit: {} },
    fadeSlideDown: { initial: {}, animate: {}, exit: {} },
  },
}));

// The header chips are covered in their own suites; stubbed so these suites
// focus on description/score/split behaviour.
vi.mock('@/components/job/ClusterSourceChips', () => ({
  ClusterSourceChips: () => null,
}));

vi.mock('@/components/job/AgencyChip', () => ({
  AgencyChip: () => null,
}));

vi.mock('@/features/jobs/components/RowMatchScore', () => ({
  RowMatchScore: () => <span data-testid="row-match-score" />,
}));

vi.mock('@ajh/shared', () => ({
  AGGREGATOR_BOARD_ID: 'aggregator',
}));

export const mockScoreJob: Mock = vi.fn();

vi.mock('@/features/jobs/providers', () => ({
  useMatchScores: () => ({
    scoreJob: mockScoreJob,
    hasResume: true,
  }),
}));

export const mockRefetch: Mock = vi.fn().mockResolvedValue(undefined);

interface ResolveStub {
  data: { description: string } | undefined;
  isLoading: boolean;
  isFetching: boolean;
  isFetched: boolean;
  isError: boolean;
  refetch: Mock;
}

export function idleStub(): ResolveStub {
  return {
    data: undefined,
    isLoading: false,
    isFetching: false,
    isFetched: false,
    isError: false,
    refetch: mockRefetch,
  };
}

export const mockUseResolveJobUrl: Mock = vi.fn().mockReturnValue(idleStub());
export const mockUpdateDescMutateAsync: Mock = vi.fn().mockResolvedValue(false);

// Cluster split (ADR-029) + external-open spies + notification container.
export const mockSplitMutate: Mock = vi.fn();
const mockOpenExternal: Mock = vi.fn();
export const mockNotify: Record<'success' | 'error', Mock> = { success: vi.fn(), error: vi.fn() };

vi.mock('@/services', () => ({
  useResolveJobUrl: (...args: unknown[]) => mockUseResolveJobUrl(...args),
  useUpdatePostingDescription: () => ({ mutateAsync: mockUpdateDescMutateAsync }),
  // Like React Query, per-call callbacks are dropped once the calling component unmounts.
  useMarkNotDuplicate: () => {
    const mounted = useRef(true);
    useEffect(
      () => () => {
        mounted.current = false;
      },
      []
    );
    return {
      mutate: (req: unknown, opts?: { onSuccess?: () => void; onError?: () => void }) =>
        mockSplitMutate(req, {
          onSuccess: () => mounted.current && opts?.onSuccess?.(),
          onError: () => mounted.current && opts?.onError?.(),
        }),
      isPending: false,
    };
  },
  useOpenExternal: () => ({ mutate: mockOpenExternal }),
}));

export const mockTrackInteraction: Mock = vi.fn().mockResolvedValue(undefined);

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

export function makePosting(id: string, overrides: Partial<Posting> = {}): Posting {
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

export const formatRelativeTime = () => '2d ago';

/** Make `useResolveJobUrl` return the idle stub with `overrides` applied. */
export function resolveReturns(overrides: Partial<ResolveStub> = {}) {
  mockUseResolveJobUrl.mockReturnValue({ ...idleStub(), ...overrides });
}

/** Resolve settled with `description` as the fetched text. */
export function resolveSettled(description: string) {
  resolveReturns({ data: { description }, isFetched: true });
}

/** Resolve in flight (nothing fetched yet). */
export function resolveInFlight() {
  resolveReturns({ isLoading: true, isFetching: true });
}

/** Call from `beforeEach`: clears every spy and restores the idle resolve stub. */
export function resetPaneMocks() {
  mockTrackInteraction.mockClear();
  mockRefetch.mockClear();
  mockUpdateDescMutateAsync.mockReset().mockResolvedValue(false);
  mockScoreJob.mockReset();
  mockSplitMutate.mockReset();
  mockOpenExternal.mockClear();
  mockNotify.success.mockClear();
  mockNotify.error.mockClear();
  mockUseResolveJobUrl.mockReturnValue(idleStub());
}

/** Render the pane for `posting` inside `act` so effects/queries settle. */
export async function openPane(posting: Posting | null) {
  let utils!: ReturnType<typeof render>;
  await act(async () => {
    utils = render(<Subject posting={posting} formatRelativeTime={formatRelativeTime} />);
  });
  return utils;
}

/** Re-render the pane with another posting inside `act`. */
export async function rerenderPane(utils: ReturnType<typeof render>, posting: Posting | null) {
  await act(async () => {
    utils.rerender(<Subject posting={posting} formatRelativeTime={formatRelativeTime} />);
  });
}

/** Flush one microtask turn (persist-then-score chains need two). */
export async function flushMicrotasks(turns = 1) {
  for (let i = 0; i < turns; i++) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}
