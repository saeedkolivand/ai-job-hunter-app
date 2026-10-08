/**
 * Enter on the listbox acts on the ACTIVE descendant. Uses the REAL
 * PostingListItem (the harness stubs it): the bug was its per-row Enter handler
 * re-selecting the row that last took DOM focus (the clicked one) instead of the
 * row arrow keys had moved to.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { Posting } from '@/features/jobs/types';
import { useSessionStore } from '@/store/session-store';
import { createMockClient, withProviders } from '@/test-support';

import { JobsSplitView } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 76,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ key: index, index, start: index * 76 })),
    measureElement: () => {},
    scrollToIndex: () => {},
  }),
}));
vi.mock('@/features/jobs/components/JobDetailPane', () => ({ JobDetailPane: () => null }));
vi.mock('@/features/jobs/providers', () => ({ useRowMatchScore: () => ({ score: undefined }) }));

const post = (id: string): Posting => ({
  id,
  source: 'linkedin',
  externalId: id,
  url: `https://example.com/${id}`,
  title: `Job ${id}`,
  company: 'Acme',
  description: '',
  capturedAt: 0,
});

describe('JobsSplitView — Enter after keyboard navigation', () => {
  it('click row 1, ArrowDown twice, Enter keeps row 3 (not row 1) selected', () => {
    const rows = [post('1'), post('2'), post('3')];
    const Wrapper = withProviders(createMockClient());
    render(
      <Wrapper>
        <JobsSplitView
          display={rows}
          formatRelativeTime={() => ''}
          scraping={false}
          onShowMore={() => {}}
        />
      </Wrapper>
    );
    const options = screen.getAllByRole('option');
    const first = options[0] as HTMLElement;
    fireEvent.click(first);
    first.focus(); // the clicked row keeps DOM focus
    fireEvent.keyDown(first, { key: 'ArrowDown' });
    fireEvent.keyDown(first, { key: 'ArrowDown' });
    expect(useSessionStore.getState().jobs.selectedId).toBe('3');

    fireEvent.keyDown(first, { key: 'Enter' });

    expect(useSessionStore.getState().jobs.selectedId).toBe('3');
  });
});
