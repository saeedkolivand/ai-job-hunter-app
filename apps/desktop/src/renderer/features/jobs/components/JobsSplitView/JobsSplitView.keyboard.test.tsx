/**
 * JobsSplitView — keyboard navigation + aria-activedescendant (active-descendant
 * pattern), selection, Back button, and Show more.
 *
 * Focus model: the listbox container is the sole tab stop (tabIndex=0); option
 * items are always tabIndex=-1. Arrow keys on the container move
 * aria-activedescendant.
 *
 * noUncheckedIndexedAccess: all array accesses are guarded.
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { act, fireEvent, screen } from '@testing-library/react';

import { useSessionStore } from '@/store/session-store';

import {
  mockOnShowMore,
  mockScrollToIndex,
  renderSplit,
  resetSplit,
  selectJob,
} from './split-harness';

beforeEach(resetSplit);

async function pressKey(key: string) {
  await act(async () => {
    fireEvent.keyDown(screen.getByRole('listbox'), { key });
  });
}

const selectedId = () => useSessionStore.getState().jobs.selectedId;

describe('JobsSplitView — aria-activedescendant', () => {
  it('has no aria-activedescendant when no posting is selected', () => {
    renderSplit();
    const listbox = screen.getByRole('listbox');
    expect(listbox).not.toHaveAttribute('aria-activedescendant');
  });

  it('points aria-activedescendant at posting-<selectedId> when a posting is selected', async () => {
    await selectJob('b');
    renderSplit();
    const listbox = screen.getByRole('listbox');
    expect(listbox).toHaveAttribute('aria-activedescendant', 'posting-b');
  });
});

// Container is the sole tab stop (tabIndex=0); option items are always tabIndex=-1.
describe('JobsSplitView — active-descendant focus model', () => {
  it('listbox container has tabIndex=0 (sole tab stop)', () => {
    renderSplit();
    expect(screen.getByRole('listbox')).toHaveAttribute('tabindex', '0');
  });

  it('all option items have tabIndex=-1 when a posting is selected', async () => {
    await selectJob('b');
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

describe('JobsSplitView — ArrowDown/ArrowUp navigation', () => {
  it.each([
    ['ArrowDown moves selectedId to the next posting', 'ArrowDown', 'a', 'b'],
    ['ArrowUp moves selectedId to the previous posting', 'ArrowUp', 'c', 'b'],
    ['ArrowDown at the last item does not move past the end', 'ArrowDown', 'c', 'c'],
    ['ArrowUp at the first item does not move before the start', 'ArrowUp', 'a', 'a'],
  ])('%s', async (_name, key, from, to) => {
    await selectJob(from);
    renderSplit();

    await pressKey(key);

    expect(selectedId()).toBe(to);
  });

  it.each([
    // 'a' is at index 0, ArrowDown → index 1.
    ['ArrowDown calls virtualizer.scrollToIndex with the next index', 'ArrowDown', 'a'],
    // 'c' is at index 2, ArrowUp → index 1.
    ['ArrowUp calls virtualizer.scrollToIndex with the previous index', 'ArrowUp', 'c'],
  ])('%s', async (_name, key, from) => {
    await selectJob(from);
    renderSplit();

    await pressKey(key);

    expect(mockScrollToIndex).toHaveBeenCalledWith(1, { align: 'auto' });
  });

  it.each([
    ['Home selects the first posting', 'Home', 'c', 'a', 0],
    ['End selects the last posting', 'End', 'a', 'c', 2],
  ])('%s', async (_name, key, from, to, index) => {
    await selectJob(from);
    renderSplit();

    await pressKey(key);

    expect(selectedId()).toBe(to);
    expect(mockScrollToIndex).toHaveBeenCalledWith(index, { align: 'auto' });
  });

  it('keeps DOM focus on the listbox after a row click and each key (End, Home, Enter keep working)', async () => {
    renderSplit();
    const listbox = screen.getByRole('listbox');
    const row = screen.getByTestId('list-item-b');

    await act(async () => {
      row.focus();
      fireEvent.click(row);
    });
    expect(listbox).toHaveFocus();

    // Simulate virtualisation dropping focus to the row, then End/Home.
    await act(async () => {
      row.focus();
    });
    await pressKey('End');
    expect(selectedId()).toBe('c');
    expect(listbox).toHaveFocus();
    await pressKey('Home');
    expect(selectedId()).toBe('a');
    expect(listbox).toHaveFocus();
    await pressKey('Enter');
    expect(selectedId()).toBe('a');
  });

  it('other keys (e.g. Tab) do not move selection or scroll', async () => {
    await selectJob('b');
    renderSplit();

    await pressKey('Tab');

    expect(selectedId()).toBe('b');
    expect(mockScrollToIndex).not.toHaveBeenCalled();
  });
});

// Collapse/expand removed — detail always visible on desktop.
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
    await selectJob('a');
    renderSplit();

    const backBtn = screen.getByRole('button', { name: 'jobs.backToList' });
    await act(async () => {
      fireEvent.click(backBtn);
    });

    expect(selectedId()).toBeNull();
  });

  it('Back button is not rendered when no job is selected', () => {
    renderSplit();
    expect(screen.queryByRole('button', { name: 'jobs.backToList' })).not.toBeInTheDocument();
  });
});

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
