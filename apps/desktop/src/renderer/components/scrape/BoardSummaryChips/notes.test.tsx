/**
 * BoardSummaryChips — informational `note` chips: location (broadened /
 * guessed-market), location-filtered:<n>, work-type-filtered:<n> (the zero-drop
 * case collapses across boards) and the partial-ATS tokens (slugs-invalid /
 * rows-dropped / companies-failed).
 */

import { describe, expect, it, vi } from 'vitest';

import { chips, renderChips, renderFirstChip, texts } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@ajh/ui', async () => (await import('./test-mocks')).uiMock());

describe('BoardSummaryChips — location note chips', () => {
  it.each([
    ['broadened:de', 'jobs.boardSummary.note.broadened', 3],
    ['guessed-market:gb', 'jobs.boardSummary.note.guessed', 2],
  ])('maps "%s" to the informational (processing) label %s', (note, key, count) => {
    const chip = renderFirstChip({ board: 'aggregator', count, notes: [note] });
    expect(chip?.getAttribute('data-color')).toBe('processing');
    expect(chip?.textContent).toContain(key);
    // The raw machine token must never leak into the UI.
    expect(chip?.textContent).not.toContain(note);
  });

  it('tolerates an unknown/future note token — falls through to the plain success chip', () => {
    const chip = renderFirstChip({ board: 'aggregator', count: 4, notes: ['future-token:de'] });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).toContain('jobs.boardSummary.count:4');
    expect(chip?.textContent).not.toContain('future-token');
  });

  it('ignores a malformed (colon-less) note token', () => {
    const chip = renderFirstChip({ board: 'aggregator', count: 4, notes: ['mystery'] });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).not.toContain('mystery');
  });

  it('ignores a malformed multi-colon token instead of rendering the trailing garbage as a country', () => {
    const chip = renderFirstChip({ board: 'aggregator', count: 4, notes: ['broadened:de:extra'] });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).not.toContain('DE:EXTRA');
    expect(chip?.textContent).not.toContain('extra');
  });

  it('precedence: error wins over a co-present note', () => {
    const chip = renderFirstChip({
      board: 'aggregator',
      count: 0,
      error: 'boom',
      notes: ['broadened:de'],
    });
    expect(chip?.getAttribute('data-color')).toBe('error');
  });

  it('precedence: truncated wins over a co-present note', () => {
    const chip = renderFirstChip({
      board: 'aggregator',
      count: 5,
      truncated: 'page 2 failed',
      notes: ['broadened:de'],
    });
    expect(chip?.getAttribute('data-color')).toBe('warning');
  });

  it('precedence: a valid note wins over the plain success count', () => {
    const chip = renderFirstChip({ board: 'aggregator', count: 6, notes: ['broadened:de'] });
    expect(chip?.getAttribute('data-color')).toBe('processing');
    expect(chip?.textContent).not.toContain('jobs.boardSummary.count');
  });
});

describe('BoardSummaryChips — location-filtered note chips', () => {
  it('maps "location-filtered:<n>" to the pluralized informational (processing) label', () => {
    const chip = renderFirstChip({ board: 'greenhouse', count: 6, notes: ['location-filtered:5'] });
    expect(chip?.getAttribute('data-color')).toBe('processing');
    // The identity mock echoes `${key}:${count}` so the count is threaded through.
    expect(chip?.textContent).toContain('jobs.boardSummary.note.locationFiltered:5');
    // The raw machine token must never leak into the UI.
    expect(chip?.textContent).not.toContain('location-filtered:5');
  });

  it('tolerates a non-numeric n — falls through to the plain success chip', () => {
    const chip = renderFirstChip({
      board: 'greenhouse',
      count: 6,
      notes: ['location-filtered:abc'],
    });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).toContain('jobs.boardSummary.count:6');
    expect(chip?.textContent).not.toContain('location-filtered');
  });

  it('maps "location-filtered:0" to the plain marker label (engine now emits n=0 too)', () => {
    const chip = renderFirstChip({ board: 'greenhouse', count: 6, notes: ['location-filtered:0'] });
    expect(chip?.getAttribute('data-color')).toBe('processing');
    expect(chip?.textContent).toContain('jobs.boardSummary.note.locationFilteredNone');
    // The pluralized hidden-count key must NOT be used for the zero case.
    expect(chip?.textContent).not.toContain('jobs.boardSummary.note.locationFiltered:');
  });

  it('tolerates an empty n (bare "location-filtered:") — falls through to success', () => {
    const chip = renderFirstChip({ board: 'lever', count: 3, notes: ['location-filtered:'] });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).not.toContain('locationFiltered');
  });

  it('tolerates a fractional / negative n (no chip)', () => {
    const chip = renderFirstChip({
      board: 'greenhouse',
      count: 6,
      notes: ['location-filtered:2.5'],
    });
    expect(chip?.getAttribute('data-color')).toBe('success');
  });

  it('precedence: an error still wins over a co-present location-filtered note', () => {
    const chip = renderFirstChip({
      board: 'greenhouse',
      count: 0,
      error: 'boom',
      notes: ['location-filtered:4'],
    });
    expect(chip?.getAttribute('data-color')).toBe('error');
  });
});

// 25 of 26 boards don't support the work-type filter (location's is 4 of 26), so
// the zero-drop case collapses across boards.
describe('BoardSummaryChips — work-type-filtered:0 collapse', () => {
  it('a single board with the zero-drop note still gets its own per-board chip', () => {
    const all = renderChips([{ board: 'greenhouse', count: 6, notes: ['work-type-filtered:0'] }]);
    expect(all).toHaveLength(1);
    expect(all[0]?.getAttribute('data-color')).toBe('processing');
    expect(all[0]?.textContent).toContain('label(greenhouse)');
    expect(all[0]?.textContent).toContain('jobs.boardSummary.note.workTypeFilteredNone');
  });

  it('collapses 2+ boards carrying only the zero-drop note into ONE summary chip', () => {
    const all = renderChips([
      { board: 'greenhouse', count: 4, notes: ['work-type-filtered:0'] },
      { board: 'lever', count: 2, notes: ['work-type-filtered:0'] },
      { board: 'personio', count: 1, notes: ['work-type-filtered:0'] },
    ]);
    expect(all).toHaveLength(1);
    expect(all[0]?.getAttribute('data-color')).toBe('processing');
    // The collapsed chip carries no board name — same "board: ''" shape as the
    // all-ok summary chip.
    expect(all[0]?.textContent).toBe('jobs.boardSummary.note.workTypeFilteredNoneSummary:3');
    expect(all[0]?.textContent).not.toContain('label(greenhouse)');
  });

  it('does NOT collapse a non-zero work-type-filtered count — it stays a per-board chip', () => {
    const all = renderChips([
      { board: 'greenhouse', count: 4, notes: ['work-type-filtered:0'] },
      { board: 'lever', count: 2, notes: ['work-type-filtered:3'] },
    ]);
    // One collapsed-solo chip for greenhouse (only 1 zero-drop board) + one
    // per-board chip for lever's n=3.
    expect(all).toHaveLength(2);
    expect(
      texts().some((t) => t.includes('label(lever)') && t.includes('workTypeFiltered:3'))
    ).toBe(true);
  });

  it('does not regress location-filtered — it keeps one chip per board even at 2+', () => {
    const all = renderChips([
      { board: 'greenhouse', count: 4, notes: ['location-filtered:0'] },
      { board: 'lever', count: 2, notes: ['location-filtered:0'] },
    ]);
    expect(all).toHaveLength(2);
    expect(all[0]?.textContent).toContain('label(greenhouse)');
    expect(all[1]?.textContent).toContain('label(lever)');
  });

  it('a board with BOTH location-filtered and a zero-drop work-type note renders both: its own location chip plus a share of the work-type collapse', () => {
    renderChips([
      { board: 'greenhouse', count: 4, notes: ['location-filtered:2', 'work-type-filtered:0'] },
      { board: 'lever', count: 2, notes: ['work-type-filtered:0'] },
    ]);
    // greenhouse: its own location-filtered chip; both boards' zero-drop work
    // type notes fold into one summary chip.
    expect(chips()).toHaveLength(2);
    expect(
      texts().some((t) => t.includes('label(greenhouse)') && t.includes('locationFiltered:2'))
    ).toBe(true);
    expect(texts().some((t) => t === 'jobs.boardSummary.note.workTypeFilteredNoneSummary:2')).toBe(
      true
    );
  });

  it('precedence: an error on one board excludes it from the collapse pool', () => {
    const all = renderChips([
      { board: 'greenhouse', count: 0, error: 'boom', notes: ['work-type-filtered:0'] },
      { board: 'lever', count: 2, notes: ['work-type-filtered:0'] },
    ]);
    // greenhouse's error chip + lever's solo zero-drop chip (only 1 board in
    // the collapse pool — greenhouse's note never entered it).
    expect(all).toHaveLength(2);
    expect(all[0]?.getAttribute('data-color')).toBe('error');
    expect(all[1]?.textContent).toContain('label(lever)');
    expect(all[1]?.textContent).toContain('workTypeFilteredNone');
  });
});

// Partial ATS visibility: `companies-failed` is the ONLY user-visible signal that
// a Lever/Ashby run (some companies fetched, some 404/403/429/over-cap) was
// incomplete — the board still returns Ok.
describe('BoardSummaryChips — partial ATS note chips', () => {
  it.each([
    ['greenhouse', 6, 'slugs-invalid:3', 'jobs.boardSummary.note.slugsInvalid:3'],
    ['rippling', 8, 'rows-dropped:2', 'jobs.boardSummary.note.rowsDropped:2'],
    ['lever', 12, 'companies-failed:2', 'jobs.boardSummary.note.companiesFailed:2'],
  ])(
    'maps %s "%s" to the pluralized informational (processing) label',
    (board, count, note, label) => {
      const chip = renderFirstChip({ board, count, notes: [note] });
      expect(chip?.getAttribute('data-color')).toBe('processing');
      // The identity mock echoes `${key}:${count}` so the count is threaded through.
      expect(chip?.textContent).toContain(label);
      // The raw machine token must never leak into the UI.
      expect(chip?.textContent).not.toContain(note);
    }
  );

  it('rejects n=0 (these tokens are only emitted for n>0) — falls through to the success chip', () => {
    const chip = renderFirstChip({ board: 'greenhouse', count: 6, notes: ['slugs-invalid:0'] });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).toContain('jobs.boardSummary.count:6');
    expect(chip?.textContent).not.toContain('slugsInvalid');
  });

  it.each(['slugs-invalid:-1', 'rows-dropped:2.5', 'slugs-invalid:abc'])(
    'rejects a negative / fractional / non-numeric n (%s) — falls through to success',
    (note) => {
      const chip = renderFirstChip({ board: 'greenhouse', count: 6, notes: [note] });
      expect(chip?.getAttribute('data-color')).toBe('success');
    }
  );

  it('tolerates a bare "slugs-invalid:" (empty n) — falls through to success', () => {
    const chip = renderFirstChip({ board: 'lever', count: 3, notes: ['slugs-invalid:'] });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).not.toContain('slugsInvalid');
  });

  it('precedence: an error still wins over a co-present partial note', () => {
    const chip = renderFirstChip({
      board: 'greenhouse',
      count: 0,
      error: 'boom',
      notes: ['slugs-invalid:2'],
    });
    expect(chip?.getAttribute('data-color')).toBe('error');
  });
});
