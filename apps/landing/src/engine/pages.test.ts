import { describe, expect, it } from 'vitest';

import {
  activePageFor,
  CORNER_TEAR_PAGE,
  EXIT_START,
  PAGE_COUNT,
  pageProgress,
  PAGES,
} from './pages';

// t that lands page `i` at local progress `p` (inverse of pageProgress's core:
// p = t * PAGE_COUNT - i, so t = (i + p) / PAGE_COUNT).
const tFor = (i: number, p: number): number => (i + p) / PAGE_COUNT;

describe('pageProgress', () => {
  it('is zero at the very start (t=0, page 0)', () => {
    const { p, exitP } = pageProgress(0, 0);
    expect(p).toBe(0);
    expect(exitP).toBe(0);
  });

  it('is fully played and fully exited at the very end (t=1, page 8)', () => {
    const { p, exitP } = pageProgress(1, PAGE_COUNT - 1);
    expect(p).toBe(1);
    expect(exitP).toBe(1);
  });

  it('hands off cleanly at each page boundary i/PAGE_COUNT', () => {
    // At t=i/9, page i-1 is fully played (p=1) and page i is just starting (p=0).
    for (let i = 1; i < PAGE_COUNT; i++) {
      const t = i / PAGE_COUNT;
      expect(pageProgress(t, i - 1).p).toBeCloseTo(1, 6);
      expect(pageProgress(t, i).p).toBeCloseTo(0, 6);
    }
  });

  it('runs local p from 0 to 1 across each page slice', () => {
    for (let i = 0; i < PAGE_COUNT; i++) {
      expect(pageProgress(tFor(i, 0), i).p).toBeCloseTo(0, 6);
      expect(pageProgress(tFor(i, 1), i).p).toBeCloseTo(1, 6);
    }
  });

  // --- the EXIT_START (0.72) play/exit split ---

  it('keeps exitP at 0 while p is below the exit split', () => {
    const { p, exitP } = pageProgress(tFor(1, 0.5), 1);
    expect(p).toBeCloseTo(0.5, 6);
    expect(exitP).toBe(0);
  });

  it('keeps exitP at 0 exactly at the exit split (p === EXIT_START)', () => {
    // i=0, t=0.08 yields p === 0.72 exactly; the split is inclusive (p <= 0.72).
    const t = 0.08;
    const { p, exitP } = pageProgress(t, 0);
    expect(p).toBe(EXIT_START);
    expect(exitP).toBe(0);
  });

  it('remaps the exit sub-slice so mid-exit p=0.86 -> exitP=0.5', () => {
    // (0.86 - 0.72) / (1 - 0.72) = 0.14 / 0.28 = 0.5.
    const { p, exitP } = pageProgress(tFor(1, 0.86), 1);
    expect(p).toBeCloseTo(0.86, 6);
    expect(exitP).toBeCloseTo(0.5, 6);
  });

  it('reaches exitP=1 when the page is fully played (p=1)', () => {
    expect(pageProgress(tFor(1, 1), 1).exitP).toBe(1);
  });

  it('clamps p and exitP to 0 below the page slice', () => {
    const { p, exitP } = pageProgress(tFor(1, -0.5), 1);
    expect(p).toBe(0);
    expect(exitP).toBe(0);
  });

  it('clamps p and exitP to 1 above the page slice', () => {
    const { p, exitP } = pageProgress(tFor(1, 4), 1);
    expect(p).toBe(1);
    expect(exitP).toBe(1);
  });
});

describe('PAGES registry', () => {
  it('has exactly PAGE_COUNT entries', () => {
    expect(PAGES.length).toBe(PAGE_COUNT);
  });

  it('marks the corner-tear page at CORNER_TEAR_PAGE', () => {
    expect(PAGES[CORNER_TEAR_PAGE]?.exit).toBe('corner-tear');
  });
});

describe('activePageFor', () => {
  it('is page 0 at t=0 and page 8 at t=1', () => {
    expect(activePageFor(0)).toBe(0);
    expect(activePageFor(1)).toBe(PAGE_COUNT - 1);
  });

  it('clamps t outside [0,1] to the end pages', () => {
    expect(activePageFor(-0.5)).toBe(0);
    expect(activePageFor(1.5)).toBe(PAGE_COUNT - 1);
  });

  it('transitions to page i exactly at and above each boundary i/PAGE_COUNT', () => {
    const eps = 1e-9;
    for (let i = 1; i < PAGE_COUNT; i++) {
      const b = i / PAGE_COUNT;
      expect(activePageFor(b - eps)).toBe(i - 1);
      expect(activePageFor(b)).toBe(i);
      expect(activePageFor(b + eps)).toBe(i);
    }
  });

  it('never returns an out-of-range page index', () => {
    for (const t of [0, 0.2, 0.5, 0.999, 1, 2]) {
      const page = activePageFor(t);
      expect(page).toBeGreaterThanOrEqual(0);
      expect(page).toBeLessThan(PAGE_COUNT);
    }
  });
});
