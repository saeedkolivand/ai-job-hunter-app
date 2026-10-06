/**
 * JobsCommandBar — active-filter chips, view mode + count controls, and the
 * work-type control visibility gate.
 *
 *  - Chips appear/disappear with the underlying session state and each chip's ×
 *    removes only its own filter.
 *  - The chips row is absent entirely when nothing is filtered and there are no
 *    scrape diagnostics (it is a *conditional* second row, not a permanent one).
 *  - The view-mode SegmentedControl writes viewMode through setJobs.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { useSessionStore } from '@/store/session-store';

import { renderBar, resetBar, setJobs } from './bar-harness';

beforeEach(resetBar);

describe('JobsCommandBar — active filter chips', () => {
  it('renders no chips row at all when nothing is filtered and there are no diagnostics', () => {
    renderBar();
    expect(screen.queryByTestId(TEST_IDS.jobs.filterChips)).not.toBeInTheDocument();
  });

  it('shows a search chip carrying the current text filter', () => {
    setJobs({ filter: 'rust' });
    renderBar();

    const chips = screen.getByTestId(TEST_IDS.jobs.filterChips);
    expect(within(chips).getByText('jobs.filters.searchChip[query=rust]')).toBeInTheDocument();
  });

  it('ignores a whitespace-only filter (no chip, no clear-all)', () => {
    setJobs({ filter: '   ' });
    renderBar();
    expect(screen.queryByTestId(TEST_IDS.jobs.filterChips)).not.toBeInTheDocument();
  });

  it("the search chip's × clears only the text filter, leaving hideAgency alone", async () => {
    const user = userEvent.setup();
    setJobs({ filter: 'rust', hideAgency: true });
    renderBar();

    // The remove label is the BARE query — prefixing it with the chip's own
    // "Search:" label produced a double colon in the announcement.
    await user.click(screen.getByRole('button', { name: 'jobs.filters.remove[name=rust]' }));

    expect(useSessionStore.getState().jobs.filter).toBe('');
    expect(useSessionStore.getState().jobs.hideAgency).toBe(true);
  });

  it('does NOT chip hide-agency — its toggle is already visible in the row above', () => {
    setJobs({ hideAgency: true });
    renderBar();

    // Duplicating an always-visible control as a chip cost a whole extra line of
    // bar height, which is the scarce resource at the 900×600 floor in German.
    expect(screen.queryByTestId(TEST_IDS.jobs.filterChips)).not.toBeInTheDocument();
    // The toggle itself still reflects the state.
    expect(
      within(screen.getByTestId(TEST_IDS.jobs.hideAgencyToggle)).getByRole('button')
    ).toHaveAttribute('aria-pressed', 'true');
  });

  it('makes the scrolling chips row itself a named, reachable tab stop', () => {
    setJobs({ filter: 'rust' });
    renderBar({
      boardSummaries: [
        { board: 'linkedin', count: 4 },
        { board: 'indeed', count: 0, skipped: 'needs-login' },
        { board: 'xing', count: 0, error: 'rate limited' },
      ],
    });

    // The row scrolls (see the single-line contract below), so everything past
    // its right edge is keyboard-unreachable unless the CONTAINER is focusable —
    // only the leftmost chip's × is a tab stop otherwise. axe stays silent on
    // this: one focusable descendant satisfies scrollable-region-focusable.
    const chips = screen.getByTestId(TEST_IDS.jobs.filterChips);
    expect(chips).toBe(screen.getByRole('group', { name: 'jobs.filters.activeLabel' }));
    expect(chips).toHaveAttribute('tabindex', '0');
    // A focusable div renders no ring by default.
    expect(chips.className).toContain('focus-visible:ring-2');

    chips.focus();
    expect(document.activeElement).toBe(chips);
  });

  it('keeps scrape diagnostics beside the filter chips, not nested in their own group', () => {
    setJobs({ filter: 'rust' });
    renderBar({ boardSummaries: [{ board: 'linkedin', count: 4 }] });

    // Diagnostics are OUTPUT, not applied filters; they keep their own group
    // label rather than being absorbed into "active filters".
    const chips = screen.getByTestId(TEST_IDS.jobs.filterChips);
    expect(
      within(chips).getByRole('button', { name: 'jobs.filters.remove[name=rust]' })
    ).toBeInTheDocument();
    expect(screen.getByRole('group', { name: 'jobs.boardSummary.label' })).toBeInTheDocument();
  });

  it('renders the sanitized failure note in the chips row when one is passed', () => {
    renderBar({ failureNote: 'connection refused' });
    const chips = screen.getByTestId(TEST_IDS.jobs.filterChips);
    expect(
      within(chips).getByText('jobs.lastScrapeFailed[reason=connection refused]')
    ).toBeInTheDocument();
  });

  it('names the row for what it actually holds, not always "active filters"', () => {
    // Diagnostics-only: calling this "Active filters" describes a row that has
    // no filters in it.
    renderBar({ boardSummaries: [{ board: 'linkedin', count: 4 }] });
    expect(screen.getByTestId(TEST_IDS.jobs.filterChips)).toBe(
      screen.getByRole('group', { name: 'jobs.commandBar.statusRow' })
    );
    expect(
      screen.queryByRole('group', { name: 'jobs.filters.activeLabel' })
    ).not.toBeInTheDocument();
  });

  it('switches the row name to "active filters" once a filter is applied', () => {
    setJobs({ filter: 'rust' });
    renderBar({ boardSummaries: [{ board: 'linkedin', count: 4 }] });
    expect(screen.getByTestId(TEST_IDS.jobs.filterChips)).toBe(
      screen.getByRole('group', { name: 'jobs.filters.activeLabel' })
    );
  });
});

describe('JobsCommandBar — view mode + count', () => {
  it('shows the terse "shown / total" count with a spelled-out screen-reader form', () => {
    renderBar({ shownCount: 3, totalCount: 5 });

    // `aria-label` on a bare span is a prohibited-and-dropped ARIA mapping, so
    // the accessible form has to be REAL (visually hidden) text.
    const terse = screen.getByText('3 / 5');
    expect(terse).toHaveAttribute('aria-hidden', 'true');

    const spelled = screen.getByText('jobs.commandBar.shownCount[shown=3,total=5]');
    expect(spelled).toBeInTheDocument();
    expect(spelled.className).toContain('sr-only');
  });

  it('gives the sort dropdown a name that says what it does, not just its value', () => {
    renderBar();
    // Without this the trigger's only accessible name is the selected option
    // ("Newest first"), which never mentions sorting.
    expect(screen.getByRole('button', { name: 'jobs.sort' })).toBeInTheDocument();
  });

  it('names the filter input with a short label, not its instructional placeholder', () => {
    renderBar();
    // The placeholder ("Filter by title, company, location…") reads as an
    // instruction in a rotor's control list, not as a name.
    const input = screen.getByRole('textbox', { name: 'jobs.commandBar.filterLabel' });
    expect(input).toHaveAttribute('placeholder', 'jobs.searchPlaceholder');
  });

  it('the segmented control switches viewMode to split', async () => {
    const user = userEvent.setup();
    renderBar();

    await user.click(screen.getByRole('radio', { name: 'jobs.viewSplit' }));
    expect(useSessionStore.getState().jobs.viewMode).toBe('split');
  });

  it('the hide-agency toggle writes hideAgency', async () => {
    const user = userEvent.setup();
    renderBar();

    await user.click(
      within(screen.getByTestId(TEST_IDS.jobs.hideAgencyToggle)).getByRole('button')
    );
    expect(useSessionStore.getState().jobs.hideAgency).toBe(true);
  });

  it('the work-type chips toggle workTypes independently', async () => {
    const user = userEvent.setup();
    renderBar();

    const group = screen.getByRole('group', { name: 'jobs.workType.label' });
    await user.click(within(group).getByRole('button', { name: 'jobs.workType.remote' }));
    expect(useSessionStore.getState().jobs.workTypes).toEqual(['remote']);

    await user.click(within(group).getByRole('button', { name: 'jobs.workType.hybrid' }));
    expect(useSessionStore.getState().jobs.workTypes).toEqual(['remote', 'hybrid']);

    await user.click(within(group).getByRole('button', { name: 'jobs.workType.remote' }));
    expect(useSessionStore.getState().jobs.workTypes).toEqual(['hybrid']);
  });

  it('shows visible "any" microcopy next to the work-type chips when the set is empty', () => {
    renderBar();
    const group = screen.getByRole('group', { name: 'jobs.workType.label' });
    expect(within(group).getByText('jobs.workType.any')).toBeInTheDocument();
  });

  it('hides the "any" microcopy once at least one work type is picked', () => {
    act(() => {
      useSessionStore.getState().setJobs({ workTypes: ['remote'] });
    });
    renderBar();
    const group = screen.getByRole('group', { name: 'jobs.workType.label' });
    expect(within(group).queryByText('jobs.workType.any')).toBeNull();
  });

  it('does NOT chip the work-type filter — same "already a visible control" rule as hideAgency', () => {
    act(() => {
      useSessionStore.getState().setJobs({ workTypes: ['remote'] });
    });
    renderBar();
    expect(screen.queryByTestId(TEST_IDS.jobs.filterChips)).not.toBeInTheDocument();
  });
});

describe('JobsCommandBar — work-type control visibility gate', () => {
  it('hides the control when nothing on screen declares a workType and no selection is active', () => {
    renderBar({ hasDeclaredWorkType: false });
    expect(screen.queryByRole('group', { name: 'jobs.workType.label' })).not.toBeInTheDocument();
  });

  it('shows the control when at least one visible posting declares a workType', () => {
    renderBar({ hasDeclaredWorkType: true });
    expect(screen.getByRole('group', { name: 'jobs.workType.label' })).toBeInTheDocument();
  });

  // The trap: an active selection from a PREVIOUS search must stay visible
  // (and clearable) even after a new search whose results declare nothing —
  // a naive `hasDeclaredWorkType` gate alone would hide the only control that
  // shows/clears a filter that is still applied, on a page silently showing
  // fewer results than it should.
  it('keeps the control visible when a selection is active, even if nothing currently declares a workType', () => {
    act(() => {
      useSessionStore.getState().setJobs({ workTypes: ['remote'] });
    });
    renderBar({ hasDeclaredWorkType: false });

    const group = screen.getByRole('group', { name: 'jobs.workType.label' });
    expect(group).toBeInTheDocument();
    const remote = within(group).getByRole('button', { name: 'jobs.workType.remote' });
    expect(remote).toHaveAttribute('aria-pressed', 'true');
  });
});
