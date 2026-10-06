/**
 * JobsResults — split-mode: container-query host, auto-select, and selection
 * preservation across re-scrapes and show-more.
 *
 * Auto-select fires only when !selectionInDisplay (selectedId not in filtered).
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { screen } from '@testing-library/dom';

import {
  mockSetJobs,
  posting,
  renderResults,
  rerenderResults,
  resetResults,
  STORE_STATE,
} from './results-harness';

beforeEach(resetResults);

const split = (selectedId: string | null) => {
  STORE_STATE.jobs = { viewMode: 'split', selectedId };
};

/** `setJobs` calls that write a selection other than `keep`. */
const overridesOf = (keep: string) =>
  mockSetJobs.mock.calls.filter((args) => (args[0] as { selectedId?: string }).selectedId !== keep);

describe('JobsResults — split-mode container host', () => {
  it('marks the results card as a @container so the split can size off pane width', () => {
    split('a');
    renderResults({ filtered: [posting('a', 'A')], resumeId: null });

    // JobsSplitView gates two-pane on `@3xl`. A container-query variant with NO
    // `@container` ancestor silently never fires (docs/PATTERNS.md §15), so the
    // split would collapse to one column at every width without this class.
    const card = screen.getByTestId('jobs-split-view').parentElement;
    expect(card?.className).toContain('@container');
    expect(card?.className).toContain('surface-card');
  });
});

describe('JobsResults — split-mode auto-select', () => {
  it('selects display[0] immediately when split mode has results and no current selection', () => {
    split(null);

    renderResults({ filtered: [posting('a', 'A'), posting('b', 'B')], resumeId: null });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('does NOT clobber a valid manual selection on a plain re-render', () => {
    split('b');

    renderResults({ filtered: [posting('a', 'A'), posting('b', 'B')], resumeId: null });

    const autoSelectCalls = mockSetJobs.mock.calls.filter(
      (args) => (args[0] as { selectedId?: string }).selectedId === 'a'
    );
    expect(autoSelectCalls).toHaveLength(0);
  });

  it('re-selects display[0] when the selected job is filtered OUT of display after mount', () => {
    split('b');
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    const view = renderResults({ filtered: [p1, p2], resumeId: null });
    mockSetJobs.mockClear();

    rerenderResults(view, [p1], false);

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('re-selects display[0] when a fresh scrape finishes (filtered empty→populated, scraping true→false)', () => {
    // waiting = scraping && filtered.length === 0.
    // Fresh search: start with empty filtered + scraping=true (waiting=true),
    // then results arrive → !selectionInDisplay fires auto-select.
    // Show-more does NOT trigger this path — it starts with items already present.
    split(null);

    // Initial render: no results yet, scraping in progress → waiting=true
    const view = renderResults({ filtered: [], resumeId: null, scraping: true });
    mockSetJobs.mockClear();

    // Scrape finishes: results arrive + scraping done → !selectionInDisplay → auto-select topId
    rerenderResults(view, [posting('x', 'X'), posting('y', 'Y')], false);

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'x' });
  });

  it('does not auto-select in list mode', () => {
    STORE_STATE.jobs = { viewMode: 'list', selectedId: null };

    renderResults({ filtered: [posting('a', 'A')], resumeId: null });

    const autoSelectCalls = mockSetJobs.mock.calls.filter(
      (args) => (args[0] as { selectedId?: string }).selectedId === 'a'
    );
    expect(autoSelectCalls).toHaveLength(0);
  });
});

describe('JobsResults — selection preserved across re-scrapes and show-more', () => {
  it('preserves the selected job when show-more completes (scraping true→false, selection stays in list)', () => {
    // auto-select must NOT fire when selectionInDisplay is already true.
    split('b');
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    // Initial render: list is already populated (show-more scenario, not fresh search).
    const view = renderResults({ filtered: [p1, p2], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    // Show-more completes: more items arrive prepended, scraping=false.
    // The previously selected 'b' is still in the list.
    rerenderResults(view, [posting('new', 'New'), p1, p2], false);

    // Selection 'b' is still valid — setJobs must NOT be called to replace it.
    expect(overridesOf('b')).toHaveLength(0);
  });

  it('preserves the selected job when new items are live-prepended during an active scrape', () => {
    // Simulates live prepend: scraping stays true, new items arrive at top.
    split('original');
    const original = posting('original', 'Original');

    const view = renderResults({ filtered: [original], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    // New items are prepended while scraping continues. 'original' stays in list.
    rerenderResults(
      view,
      [posting('newer2', 'Newer 2'), posting('newer1', 'Newer 1'), original],
      true
    );

    // topId is now 'newer2', but selectedId 'original' is still in the list.
    // setJobs must NOT fire to replace the user's selection.
    expect(overridesOf('original')).toHaveLength(0);
  });

  it('auto-selects the new topId when a re-scrape returns results but selection is absent (null)', () => {
    // Complementary case: selection was null going into the re-scrape (e.g. user
    // cleared it). When results arrive the detail pane must not stay blank.
    split(null);

    const view = renderResults({ filtered: [], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    rerenderResults(view, [posting('a', 'A'), posting('b', 'B')], false);

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('auto-selects topId when selection is filtered out during show-more rerender', () => {
    // Edge case: show-more changes the active filter, removing the selected job.
    // The effect must fall back to topId (not leave detail pane blank).
    split('b');
    const p1 = posting('a', 'A');

    const view = renderResults({
      filtered: [p1, posting('b', 'B')],
      scraping: true,
      resumeId: null,
    });
    mockSetJobs.mockClear();

    // After show-more, 'b' is no longer in the filtered list.
    rerenderResults(view, [p1, posting('c', 'C')], false);

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('does NOT auto-select in list mode during show-more rerenders', () => {
    // List mode is always a no-op regardless of scraping transitions.
    STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
    const p1 = posting('a', 'A');

    const view = renderResults({ filtered: [p1], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    rerenderResults(view, [p1, posting('b', 'B')], false);

    expect(mockSetJobs).not.toHaveBeenCalled();
  });
});
