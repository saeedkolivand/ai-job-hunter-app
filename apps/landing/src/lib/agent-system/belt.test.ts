import { describe, expect, it } from 'vitest';

import { STATIONS } from '@/data/agent-fleet';

import { activeStationX, beltProgress, nearestStationIndex, STATION_COUNT } from './belt';

describe('STATION_COUNT', () => {
  it('is derived from STATIONS, never a hardcoded literal', () => {
    expect(STATION_COUNT).toBe(STATIONS.length);
  });
});

describe('beltProgress', () => {
  it.each([
    // [sectionTop, sectionHeight, viewportHeight, expected, why]
    [0, 500, 500, 0, 'zero runway (height === viewport) never divides by zero'],
    [0, 400, 500, 0, 'negative runway (section shorter than viewport) also stays 0'],
    [50, 600, 500, 0, 'not yet scrolled into view (positive top) clamps to 0'],
    [0, 600, 500, 0, 'exactly at the top of the runway is 0'],
    [-50, 600, 500, 0.5, 'linear interior point of a 100px runway'],
    [-100, 600, 500, 1, 'exactly at the end of the runway is 1'],
    [-1000, 600, 500, 1, 'scrolled well past the runway clamps to 1'],
  ] as const)(
    'beltProgress(%d, %d, %d) === %d (%s)',
    (sectionTop, sectionHeight, viewportHeight, expected, _why) => {
      expect(beltProgress(sectionTop, sectionHeight, viewportHeight)).toBe(expected);
    }
  );
});

describe('activeStationX', () => {
  it('interpolates linearly between the first and last station centers', () => {
    expect(activeStationX(0, 100, 0)).toBe(0);
    expect(activeStationX(0, 100, 1)).toBe(100);
    expect(activeStationX(0, 100, 0.5)).toBe(50);
  });

  it('is not itself clamped — callers pass an already-clamped beltProgress', () => {
    expect(activeStationX(0, 100, 2)).toBe(200);
    expect(activeStationX(0, 100, -1)).toBe(-100);
  });
});

describe('nearestStationIndex', () => {
  it.each([
    [[0, 100, 200], -50, 0, 'out-of-bounds x below the first center snaps to it'],
    [[0, 100, 200], 1000, 2, 'out-of-bounds x above the last center snaps to it'],
    [[0, 100, 200], 100, 1, 'exact hit on a center'],
    [[0, 100], 50, 0, 'exactly between two stations picks the first (strict <, first wins ties)'],
    [[42], -999, 0, 'a single station is always the answer'],
    [[42], 999, 0, 'a single station is always the answer'],
  ] as const)('nearestStationIndex(%j, %d) === %d (%s)', (centers, activeX, expected, _why) => {
    expect(nearestStationIndex(centers, activeX)).toBe(expected);
  });

  it('always returns a valid index into a non-empty centers array', () => {
    const centers = [0, 50, 120, 121, 500];
    for (const activeX of [-Infinity, -1e6, -1, 0, 60, 120.5, 1e6, Infinity]) {
      const index = nearestStationIndex(centers, activeX);
      expect(index).toBeGreaterThanOrEqual(0);
      expect(index).toBeLessThan(centers.length);
    }
  });

  it('returns 0 on an empty centers array (documented current behavior, not a valid index)', () => {
    // centers.forEach never runs, so `step` never advances past its initial 0 —
    // but an empty array has no index 0. Never hit in practice since STATION_COUNT
    // (and therefore every centers array built from it) is never empty.
    expect(nearestStationIndex([], 42)).toBe(0);
  });
});
