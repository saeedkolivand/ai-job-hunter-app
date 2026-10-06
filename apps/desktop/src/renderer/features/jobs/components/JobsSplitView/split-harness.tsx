/**
 * Harness for the JobsSplitView suites.
 *
 *  - useSessionStore is the real Zustand store (no mock) so state flows naturally.
 *  - PostingListItem and JobDetailPane are stubbed — only the list container and
 *    its behaviour matter here.
 *  - Virtualizer is stubbed to render all items in order synchronously;
 *    `mockScrollToIndex` is the spy captured from it.
 *  - useRowMatchScore is stubbed (no scoring provider needed).
 */

import type React from 'react';
import { type Mock, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import type { Posting } from '@/features/jobs/types';
import { useSessionStore } from '@/store/session-store';

import { JobsSplitView } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  Button: ({
    children,
    onClick,
    'aria-label': ariaLabel,
    className,
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    'aria-label'?: string;
    className?: string;
  }) => (
    <div role="button" onClick={onClick} aria-label={ariaLabel} className={className}>
      {children}
    </div>
  ),
}));

vi.mock('lucide-react', () => ({
  ChevronLeft: () => null,
  Plus: () => null,
}));

// Active-descendant pattern: items always tabIndex={-1}; container is the tab stop.
vi.mock('@/features/jobs/components/PostingListItem', () => ({
  PostingListItem: ({
    posting,
    selected,
    onSelect,
  }: {
    posting: Posting;
    selected: boolean;
    onSelect: (p: Posting) => void;
    formatRelativeTime: (t?: number) => string;
  }) => (
    <div
      id={`posting-${posting.id}`}
      role="option"
      aria-selected={selected}
      tabIndex={-1}
      data-testid={`list-item-${posting.id}`}
      onClick={() => onSelect(posting)}
    >
      {posting.title}
    </div>
  ),
}));

vi.mock('@/features/jobs/components/JobDetailPane', () => ({
  JobDetailPane: ({ posting }: { posting: Posting | null }) => (
    <div data-testid="job-detail">{posting?.id ?? 'empty'}</div>
  ),
}));

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: () => ({ score: undefined }),
}));

export const mockScrollToIndex: Mock = vi.fn();

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({
    count,
    getItemKey,
  }: {
    count: number;
    getItemKey: (i: number) => string;
  }) => ({
    getTotalSize: () => count * 72,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        key: getItemKey(index),
        index,
        start: index * 72,
      })),
    measureElement: () => {},
    scrollToIndex: mockScrollToIndex,
  }),
}));

function makePosting(id: string): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: `Job ${id}`,
    company: 'Acme',
    description: '',
    capturedAt: 0,
  };
}

const POSTINGS = [makePosting('a'), makePosting('b'), makePosting('c')];
export const mockOnShowMore: Mock = vi.fn();

export function renderSplit(display = POSTINGS) {
  return render(
    <JobsSplitView
      display={display}
      formatRelativeTime={() => ''}
      scraping={false}
      onShowMore={mockOnShowMore}
    />
  );
}

/** Seed `jobs.selectedId` in the real store (inside `act`). */
export async function selectJob(selectedId: string | null) {
  await act(async () => {
    useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId } }));
  });
}

const initialState = useSessionStore.getState();

/** Call from `beforeEach`: real store back to defaults, spies cleared. */
export function resetSplit() {
  useSessionStore.setState(initialState, true);
  mockScrollToIndex.mockClear();
  mockOnShowMore.mockClear();
}
