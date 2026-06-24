/**
 * JobsSplitView — keyboard navigation + aria-activedescendant (active-descendant pattern).
 *
 * Focus model: the listbox container is the sole tab stop (tabIndex=0); option items
 * are always tabIndex=-1. Arrow keys on the container move aria-activedescendant.
 *
 * Strategy:
 *  - useSessionStore is the real Zustand store (no mock) so state flows naturally.
 *  - PostingListItem and JobDetailPane are stubbed — only the list container
 *    and its keyboard behaviour matter here.
 *  - Virtualizer is stubbed to render all items in order synchronously.
 *  - useRowMatchScore is stubbed (no scoring provider needed).
 *  - scrollToIndex is a spy captured from the virtualizer stub.
 *
 * noUncheckedIndexedAccess: all array accesses are guarded.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { Posting } from '@/features/jobs/types';
import { useSessionStore } from '@/store/session-store';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @ajh/ui — minimal stubs ───────────────────────────────────────────────────

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

// ── lucide-react ──────────────────────────────────────────────────────────────

vi.mock('lucide-react', () => ({
  ChevronLeft: () => null,
  Plus: () => null,
}));

// ── PostingListItem — renders a div with the posting id for easy querying ─────
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

// ── JobDetailPane stub ────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/JobDetailPane', () => ({
  JobDetailPane: ({ posting }: { posting: Posting | null }) => (
    <div data-testid="job-detail">{posting?.id ?? 'empty'}</div>
  ),
}));

// ── useRowMatchScore ──────────────────────────────────────────────────────────

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: () => ({ score: undefined }),
}));

// ── Virtualizer stub — captures scrollToIndex spy ────────────────────────────

const mockScrollToIndex = vi.fn();

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

// ── component under test ──────────────────────────────────────────────────────

import { JobsSplitView } from './index';

// ── fixtures ──────────────────────────────────────────────────────────────────

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
const formatRelativeTime = () => '';
const mockOnShowMore = vi.fn();

function renderSplit(display = POSTINGS) {
  return render(
    <JobsSplitView
      display={display}
      formatRelativeTime={formatRelativeTime}
      scraping={false}
      onShowMore={mockOnShowMore}
    />
  );
}

// ── reset — use real Zustand store, reset to defaults each test ───────────────

const initialState = useSessionStore.getState();

beforeEach(() => {
  useSessionStore.setState(initialState, true);
  mockScrollToIndex.mockClear();
  mockOnShowMore.mockClear();
});

// ─────────────────────────────────────────────────────────────────────────────
// aria-activedescendant
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsSplitView — aria-activedescendant', () => {
  it('has no aria-activedescendant when no posting is selected', () => {
    renderSplit();
    const listbox = screen.getByRole('listbox');
    expect(listbox).not.toHaveAttribute('aria-activedescendant');
  });

  it('points aria-activedescendant at posting-<selectedId> when a posting is selected', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'b' } }));
    });
    renderSplit();
    const listbox = screen.getByRole('listbox');
    expect(listbox).toHaveAttribute('aria-activedescendant', 'posting-b');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Active-descendant focus model
// Container is the sole tab stop (tabIndex=0); option items are always tabIndex=-1.
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsSplitView — active-descendant focus model', () => {
  it('listbox container has tabIndex=0 (sole tab stop)', () => {
    renderSplit();
    expect(screen.getByRole('listbox')).toHaveAttribute('tabindex', '0');
  });

  it('all option items have tabIndex=-1 when a posting is selected', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'b' } }));
    });
    renderSplit();

    expect(screen.getByTestId('list-item-a')).toHaveAttribute('tabindex', '-1');
    expect(screen.getByTestId('list-item-b')).toHaveAttribute('tabindex', '-1');
    expect(screen.getByTestId('list-item-c')).toHaveAttribute('tabindex', '-1');
  });

  it('all option items have tabIndex=-1 when nothing is selected', () => {
    renderSplit();
    expect(screen.getByTestId('list-item-a')).toHaveAttribute('tabindex', '-1');
    expect(screen.getByTestId('list-item-b')).toHaveAttribute('tabindex', '-1');
    expect(screen.getByTestId('list-item-c')).toHaveAttribute('tabindex', '-1');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ArrowDown / ArrowUp keyboard navigation
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsSplitView — ArrowDown/ArrowUp navigation', () => {
  it('ArrowDown moves selectedId to the next posting', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'a' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'ArrowDown' });
    });

    expect(useSessionStore.getState().jobs.selectedId).toBe('b');
  });

  it('ArrowUp moves selectedId to the previous posting', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'c' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'ArrowUp' });
    });

    expect(useSessionStore.getState().jobs.selectedId).toBe('b');
  });

  it('ArrowDown at the last item does not move past the end', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'c' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'ArrowDown' });
    });

    expect(useSessionStore.getState().jobs.selectedId).toBe('c');
  });

  it('ArrowUp at the first item does not move before the start', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'a' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'ArrowUp' });
    });

    expect(useSessionStore.getState().jobs.selectedId).toBe('a');
  });

  it('ArrowDown calls virtualizer.scrollToIndex with the next index', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'a' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'ArrowDown' });
    });

    // 'a' is at index 0, ArrowDown → index 1.
    expect(mockScrollToIndex).toHaveBeenCalledWith(1, { align: 'auto' });
  });

  it('ArrowUp calls virtualizer.scrollToIndex with the previous index', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'c' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'ArrowUp' });
    });

    // 'c' is at index 2, ArrowUp → index 1.
    expect(mockScrollToIndex).toHaveBeenCalledWith(1, { align: 'auto' });
  });

  it('other keys (e.g. Enter) do not move selection or scroll', async () => {
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'b' } }));
    });
    renderSplit();

    const listbox = screen.getByRole('listbox');
    await act(async () => {
      fireEvent.keyDown(listbox, { key: 'Enter' });
    });

    expect(useSessionStore.getState().jobs.selectedId).toBe('b');
    expect(mockScrollToIndex).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Selection and Back button (collapse/expand removed — detail always visible on desktop)
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsSplitView — selection and Back button', () => {
  it('clicking a list item sets selectedId (no detailCollapsed — field removed)', async () => {
    renderSplit();

    await act(async () => {
      fireEvent.click(screen.getByTestId('list-item-b'));
    });

    const { jobs } = useSessionStore.getState();
    expect(jobs.selectedId).toBe('b');
  });

  it('Back button sets selectedId to null (narrow-screen back navigation)', async () => {
    // Select a posting so the detail section renders and Back button appears.
    await act(async () => {
      useSessionStore.setState((s) => ({ jobs: { ...s.jobs, selectedId: 'a' } }));
    });
    renderSplit();

    const backBtn = screen.getByRole('button', { name: 'jobs.backToList' });
    await act(async () => {
      fireEvent.click(backBtn);
    });

    expect(useSessionStore.getState().jobs.selectedId).toBeNull();
  });

  it('Back button is not rendered when no job is selected', () => {
    renderSplit();
    expect(screen.queryByRole('button', { name: 'jobs.backToList' })).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Show more button
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsSplitView — Show more button', () => {
  it('renders a "Show more" button in the list pane', () => {
    renderSplit();
    expect(screen.getByRole('button', { name: /jobs\.showMore/i })).toBeInTheDocument();
  });

  it('clicking "Show more" calls onShowMore', async () => {
    renderSplit();
    const btn = screen.getByRole('button', { name: /jobs\.showMore/i });
    await act(async () => {
      fireEvent.click(btn);
    });
    expect(mockOnShowMore).toHaveBeenCalledTimes(1);
  });
});
