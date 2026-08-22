/**
 * JobsCommandBar — the compact bar that replaced the Jobs hero.
 *
 * Covered:
 *  - Active-filter chips appear/disappear with the underlying session state and
 *    each chip's × removes only its own filter (per-chip remove + clear-all).
 *  - The chips row is absent entirely when nothing is filtered and there are no
 *    scrape diagnostics (it is a *conditional* second row, not a permanent one).
 *  - The view-mode SegmentedControl writes viewMode through setJobs.
 *  - The live scrape strip (progress label + Cancel) only exists while scraping —
 *    the drawer auto-closes, so this is the sole remaining cancel affordance.
 *  - The narrow-window contract: the control row wraps instead of clipping, and
 *    no descendant pins it to a no-wrap `shrink-0` strip.
 *
 * The real Zustand session store is used (no mock) so state flows naturally;
 * `@ajh/ui` is real so the chips' close buttons are the real controls.
 */
import type { ComponentProps } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { useSessionStore } from '@/store/session-store';

// t() renders "key[param=value,…]" so both key and params are assertable.
// `i18n.language` is required by the real BoardSummaryChips.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, unknown>) =>
      p
        ? `${k}[${Object.entries(p)
            .map(([key, val]) => `${key}=${String(val)}`)
            .join(',')}]`
        : k,
    i18n: { language: 'en' },
  }),
}));

import { JobsCommandBar } from './index';

type BarProps = ComponentProps<typeof JobsCommandBar>;

const baseProps: BarProps = {
  shownCount: 3,
  totalCount: 5,
  scraping: false,
  scrapeProgress: null,
  canClear: true,
  onClear: vi.fn(),
  onScrape: vi.fn(),
  onCancelScrape: vi.fn(),
  boardSummaries: [],
  failureNote: null,
  // Most existing tests below predate the visibility gate and assume the
  // work-type control renders; default it "on" so they keep testing what
  // they were written to test. The gate itself is covered by its own
  // describe block further down, which overrides this per-case.
  hasDeclaredWorkType: true,
};

function renderBar(overrides: Partial<BarProps> = {}) {
  return render(<JobsCommandBar {...baseProps} {...overrides} />);
}

function setJobs(patch: Parameters<ReturnType<typeof useSessionStore.getState>['setJobs']>[0]) {
  act(() => {
    useSessionStore.getState().setJobs(patch);
  });
}

beforeEach(() => {
  setJobs({ filter: '', sortBy: 'newest', viewMode: 'list', hideAgency: false, workTypes: [] });
});

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

describe('JobsCommandBar — status live region', () => {
  it('mounts the live region up front, empty, even with nothing to announce', () => {
    renderBar({ scraping: false });

    // A role="status" node created at the same instant as its text is
    // unreliably announced by NVDA/JAWS — the region has to pre-exist so the
    // change is what fires. Matters here because the strip it announces is the
    // only Cancel affordance once the drawer closes.
    const live = screen.getByTestId(TEST_IDS.jobs.scrapeStatusLive);
    expect(live).toHaveAttribute('role', 'status');
    expect(live).toHaveAttribute('aria-live', 'polite');
    expect(live.className).toContain('sr-only');
    expect(live).toHaveTextContent('');
  });

  it('writes the scrape status into the SAME region rather than mounting a new one', () => {
    const { rerender } = renderBar({ scraping: false });
    const live = screen.getByTestId(TEST_IDS.jobs.scrapeStatusLive);

    rerender(<JobsCommandBar {...baseProps} scraping scrapeProgress={0.42} />);

    // Same DOM node, new text — that is what makes the announcement reliable.
    expect(screen.getByTestId(TEST_IDS.jobs.scrapeStatusLive)).toBe(live);
    expect(live).toHaveTextContent('jobs.scanningPercent[percent=42]');
  });

  it('announces a scrape failure through the same region', () => {
    renderBar({ failureNote: 'connection refused' });
    expect(screen.getByTestId(TEST_IDS.jobs.scrapeStatusLive)).toHaveTextContent(
      'jobs.lastScrapeFailed[reason=connection refused]'
    );
  });

  it('hides the visual copies from AT so nothing is announced twice', () => {
    renderBar({ scraping: true, scrapeProgress: 0.42, failureNote: 'boom' });

    const strip = screen.getByTestId(TEST_IDS.jobs.scrapeStatusStrip);
    expect(within(strip).getByText('jobs.scanningPercent[percent=42]')).toHaveAttribute(
      'aria-hidden',
      'true'
    );
    // The Cancel button is a CONTROL, not status — it stays exposed.
    expect(within(strip).getByRole('button', { name: 'jobs.cancel' })).toBeInTheDocument();
    expect(screen.getByText('jobs.lastScrapeFailed[reason=boom]')).toHaveAttribute(
      'aria-hidden',
      'true'
    );
  });
});

describe('JobsCommandBar — live scrape strip', () => {
  it('is absent while idle', () => {
    renderBar({ scraping: false });
    expect(screen.queryByTestId(TEST_IDS.jobs.scrapeStatusStrip)).not.toBeInTheDocument();
  });

  it('shows an indeterminate label + Cancel before the first board completes', async () => {
    const user = userEvent.setup();
    const onCancelScrape = vi.fn();
    renderBar({ scraping: true, scrapeProgress: null, onCancelScrape });

    const strip = screen.getByTestId(TEST_IDS.jobs.scrapeStatusStrip);
    // Same copy as the results skeleton, so the two progress surfaces never
    // word the same state differently.
    expect(within(strip).getByText('jobs.scanning')).toBeInTheDocument();

    await user.click(within(strip).getByRole('button', { name: 'jobs.cancel' }));
    expect(onCancelScrape).toHaveBeenCalledTimes(1);
  });

  it('meets the light-scheme contrast floor on the row carrying the only Cancel', () => {
    renderBar({ scraping: true });
    const strip = screen.getByTestId(TEST_IDS.jobs.scrapeStatusStrip);
    // The light-legibility remap in utilities.css lifts ONLY /20…/50, so /55
    // renders lighter than /50 and measured 3.67:1 — below AA.
    expect(strip.className).toContain('text-foreground/70');
    expect(strip.className).not.toContain('text-foreground/55');
  });

  it('shows a rounded percentage once progress is known', () => {
    renderBar({ scraping: true, scrapeProgress: 0.666 });
    const strip = screen.getByTestId(TEST_IDS.jobs.scrapeStatusStrip);
    expect(within(strip).getByText('jobs.scanningPercent[percent=67]')).toBeInTheDocument();
  });

  it('hides the destructive Clear action while a scrape runs (canClear=false)', () => {
    renderBar({ scraping: true, canClear: false });
    expect(screen.queryByRole('button', { name: /jobs\.clear$/ })).not.toBeInTheDocument();
  });
});

describe('JobsCommandBar — narrow-window layout contract', () => {
  it('wraps the control row instead of clipping it, and never scrolls itself', () => {
    renderBar();

    const bar = screen.getByTestId(TEST_IDS.jobs.commandBar);
    // The bar itself owns no overflow — the old bounded `overflow-y-auto`
    // wrapper is what produced the stray horizontal scrollbar.
    expect(bar.className).not.toContain('overflow');
    expect(bar.className).toContain('shrink-0');

    // Anchored on the title rather than `firstElementChild` — the sr-only live
    // region is the first child, and positional lookups silently retarget.
    const controlRow = screen.getByRole('heading', { level: 1 }).parentElement;
    expect(controlRow?.className).toContain('flex-wrap');
    // A `shrink-0` on the row would re-pin it at max-content and reintroduce the
    // clip. Token match, not substring: `group-hover:shrink-0` etc. must not
    // read as a hit.
    expect(controlRow?.className.split(/\s+/)).not.toContain('shrink-0');
  });

  it('keeps the chips row to a single line, scrolling sideways instead of growing taller', () => {
    setJobs({ filter: 'rust' });
    renderBar({
      boardSummaries: [
        { board: 'linkedin', count: 4 },
        { board: 'indeed', count: 0, skipped: 'needs-login' },
        { board: 'xing', count: 0, error: 'rate limited' },
      ],
    });

    const chips = screen.getByTestId(TEST_IDS.jobs.filterChips);
    // Height is the scarce resource at the 900×600 floor (German runs ~30%
    // longer); a wrapping diagnostics row ate the results list.
    expect(chips.className).toContain('flex-nowrap');
    expect(chips.className).toContain('overflow-x-auto');
    expect(chips.className).not.toContain('flex-wrap');
    // Children must not shrink, or "nowrap" would just squash them instead.
    for (const child of Array.from(chips.children)) {
      expect(child.className).toContain('shrink-0');
    }
  });
});
