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
  setJobs({ filter: '', sortBy: 'newest', viewMode: 'list', hideAgency: false });
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

    await user.click(
      screen.getByRole('button', {
        name: 'jobs.filters.remove[name=jobs.filters.searchChip[query=rust]]',
      })
    );

    expect(useSessionStore.getState().jobs.filter).toBe('');
    expect(useSessionStore.getState().jobs.hideAgency).toBe(true);
  });

  it("the hide-agency chip's × clears only hideAgency, leaving the text filter alone", async () => {
    const user = userEvent.setup();
    setJobs({ filter: 'rust', hideAgency: true });
    renderBar();

    await user.click(
      screen.getByRole('button', {
        name: 'jobs.filters.remove[name=jobs.filters.hideAgency]',
      })
    );

    expect(useSessionStore.getState().jobs.hideAgency).toBe(false);
    expect(useSessionStore.getState().jobs.filter).toBe('rust');
  });

  it('clear-all removes both filters in one go', async () => {
    const user = userEvent.setup();
    setJobs({ filter: 'rust', hideAgency: true });
    renderBar();

    await user.click(screen.getByRole('button', { name: 'jobs.filters.clearAll' }));

    expect(useSessionStore.getState().jobs.filter).toBe('');
    expect(useSessionStore.getState().jobs.hideAgency).toBe(false);
  });

  it('offers no clear-all when only scrape diagnostics (not filters) populate the row', () => {
    renderBar({ boardSummaries: [{ board: 'linkedin', count: 4 }] });

    expect(screen.getByTestId(TEST_IDS.jobs.filterChips)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'jobs.filters.clearAll' })).not.toBeInTheDocument();
  });

  it('renders the sanitized failure note in the chips row when one is passed', () => {
    renderBar({ failureNote: 'connection refused' });
    expect(
      screen.getByText('jobs.lastScrapeFailed[reason=connection refused]')
    ).toBeInTheDocument();
  });
});

describe('JobsCommandBar — view mode + count', () => {
  it('shows the terse "shown / total" count with a spelled-out accessible name', () => {
    renderBar({ shownCount: 3, totalCount: 5 });
    const count = screen.getByLabelText('jobs.commandBar.shownCount[shown=3,total=5]');
    expect(count).toHaveTextContent('3 / 5');
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
    expect(within(strip).getByText('jobs.scraping')).toBeInTheDocument();

    await user.click(within(strip).getByRole('button', { name: 'jobs.cancel' }));
    expect(onCancelScrape).toHaveBeenCalledTimes(1);
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
  it('wraps the control row instead of clipping it, and never scrolls horizontally', () => {
    renderBar();

    const bar = screen.getByTestId(TEST_IDS.jobs.commandBar);
    // The bar itself owns no overflow — the old bounded `overflow-y-auto`
    // wrapper is what produced the stray horizontal scrollbar.
    expect(bar.className).not.toContain('overflow');
    expect(bar.className).toContain('shrink-0');
    // Container-query context so children size off the page column, not the viewport.
    expect(bar.className).toContain('@container');

    const controlRow = bar.firstElementChild;
    expect(controlRow?.className).toContain('flex-wrap');
    // A `shrink-0` on the row would re-pin it at max-content and reintroduce the clip.
    expect(controlRow?.className).not.toContain('shrink-0');
  });
});
