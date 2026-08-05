import { describe, expect, it } from 'vitest';

import type { TechRadarEntry } from '@/data/tech-radar';

import { layoutBlips, RING_BANDS, VIEW_SIZE } from './geometry';

const entry = (
  overrides: Partial<TechRadarEntry> & Pick<TechRadarEntry, 'id'>
): TechRadarEntry => ({
  name: overrides.id,
  ring: 'adopt',
  quadrant: 'renderer-ui',
  subjectKind: 'technique',
  summary: 's',
  rationale: 'r',
  lastReviewed: '2026-08-05',
  ...overrides,
});

describe('layoutBlips', () => {
  it('returns one position per entry, keyed by id', () => {
    const entries = [
      entry({ id: 'a' }),
      entry({ id: 'b', quadrant: 'backend-data' }),
      entry({ id: 'c', ring: 'hold' }),
    ];
    const positions = layoutBlips(entries);
    expect(positions.size).toBe(3);
    for (const e of entries) expect(positions.has(e.id)).toBe(true);
  });

  it('places every blip inside the canvas, within its ring band radius from center', () => {
    const entries: TechRadarEntry[] = [
      entry({ id: 'a', ring: 'adopt', quadrant: 'renderer-ui' }),
      entry({ id: 'b', ring: 'trial', quadrant: 'backend-data' }),
      entry({ id: 'c', ring: 'assess', quadrant: 'documents-export' }),
      entry({ id: 'd', ring: 'hold', quadrant: 'build-ship-trust' }),
    ];
    const positions = layoutBlips(entries);
    const center = VIEW_SIZE / 2;
    for (const e of entries) {
      const pos = positions.get(e.id);
      if (!pos) throw new Error(`no position for ${e.id}`);
      const dist = Math.hypot(pos.x - center, pos.y - center);
      const [innerR, outerR] = RING_BANDS[e.ring];
      expect(dist).toBeGreaterThanOrEqual(innerR - 0.01);
      expect(dist).toBeLessThanOrEqual(outerR + 0.01);
      expect(pos.x).toBeGreaterThanOrEqual(0);
      expect(pos.x).toBeLessThanOrEqual(VIEW_SIZE);
      expect(pos.y).toBeGreaterThanOrEqual(0);
      expect(pos.y).toBeLessThanOrEqual(VIEW_SIZE);
    }
  });

  it('spreads multiple entries in the same ring+quadrant cell to distinct positions', () => {
    const entries = [
      entry({ id: 'a' }),
      entry({ id: 'b' }),
      entry({ id: 'c' }),
      entry({ id: 'd' }),
    ];
    const positions = layoutBlips(entries);
    const coords = entries.map((e) => positions.get(e.id));
    const unique = new Set(coords.map((p) => `${p?.x.toFixed(3)},${p?.y.toFixed(3)}`));
    expect(unique.size).toBe(entries.length);
  });

  it('is deterministic — same input always yields the same layout', () => {
    const entries = [entry({ id: 'a' }), entry({ id: 'b', ring: 'hold' })];
    const first = layoutBlips(entries);
    const second = layoutBlips(entries);
    for (const e of entries) {
      expect(first.get(e.id)).toEqual(second.get(e.id));
    }
  });

  it('keeps each quadrant in its own 90-degree wedge (no cross-quadrant overlap in angle)', () => {
    // renderer-ui starts at 0deg (top, going clockwise toward 3 o'clock);
    // build-ship-trust starts at 270deg (the wedge just before it, 9 o'clock
    // to 12). A renderer-ui blip should sit at x >= center; a
    // build-ship-trust blip at the same ring should sit at x <= center.
    const entries = [
      entry({ id: 'right', quadrant: 'renderer-ui', ring: 'trial' }),
      entry({ id: 'left', quadrant: 'build-ship-trust', ring: 'trial' }),
    ];
    const positions = layoutBlips(entries);
    const center = VIEW_SIZE / 2;
    const right = positions.get('right');
    const left = positions.get('left');
    if (!right || !left) throw new Error('missing position');
    expect(right.x).toBeGreaterThan(center);
    expect(left.x).toBeLessThan(center);
  });
});
