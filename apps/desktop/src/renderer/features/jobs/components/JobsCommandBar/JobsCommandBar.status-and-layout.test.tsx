/**
 * JobsCommandBar — status live region, the live scrape strip (progress label +
 * Cancel, the sole remaining cancel affordance once the drawer auto-closes),
 * the hybrid-search trigger, and the narrow-window layout contract (the control
 * row wraps instead of clipping; no descendant pins it to a no-wrap strip).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { renderBar, rerenderBar, resetBar, setJobs } from './bar-harness';

beforeEach(resetBar);

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
    const view = renderBar({ scraping: false });
    const live = screen.getByTestId(TEST_IDS.jobs.scrapeStatusLive);

    rerenderBar(view, { scraping: true, scrapeProgress: 0.42 });

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

describe('JobsCommandBar — hybrid search trigger', () => {
  it('disables the search button while the filter is empty', () => {
    renderBar({ searchState: 'idle' });
    expect(screen.getByTestId(TEST_IDS.jobs.searchButton)).toBeDisabled();
  });

  it('Enter in the filter box commits a search, not just the instant substring filter', async () => {
    const user = userEvent.setup();
    const onSubmitSearch = vi.fn();
    setJobs({ filter: 'rust engineer' });
    renderBar({ onSubmitSearch });

    await user.type(
      screen.getByRole('textbox', { name: 'jobs.commandBar.filterLabel' }),
      '{Enter}'
    );
    expect(onSubmitSearch).toHaveBeenCalledTimes(1);
  });

  it('the search button itself also commits a search', async () => {
    const user = userEvent.setup();
    const onSubmitSearch = vi.fn();
    setJobs({ filter: 'rust engineer' });
    renderBar({ onSubmitSearch });

    await user.click(screen.getByTestId(TEST_IDS.jobs.searchButton));
    expect(onSubmitSearch).toHaveBeenCalledTimes(1);
  });

  it('shows a spinner on the search button while searching', () => {
    setJobs({ filter: 'rust engineer' });
    renderBar({ searchState: 'searching' });
    const button = screen.getByTestId(TEST_IDS.jobs.searchButton);
    expect(button.querySelector('.animate-spin')).not.toBeNull();
  });

  it('disables sort while a ranked search governs the list, re-enables once cleared', () => {
    const view = renderBar({ searchState: 'results' });
    expect(screen.getByRole('button', { name: 'jobs.sort' })).toBeDisabled();

    rerenderBar(view, { searchState: 'idle' });
    expect(screen.getByRole('button', { name: 'jobs.sort' })).not.toBeDisabled();
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
