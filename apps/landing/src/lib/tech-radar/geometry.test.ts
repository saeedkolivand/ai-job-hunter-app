import { describe, expect, it } from 'vitest';

import { RADAR, type TechRadarEntry } from '@/data/tech-radar';

import {
  HIT_RADIUS,
  layoutBlips,
  minPairwiseDistance,
  RING_BANDS,
  SVG_MIN_SHOWN_WIDTH_PX,
  VIEW_SIZE,
} from './geometry';

// The narrowest CSS-px scale the canvas is ever actually rendered at — see
// SVG_MIN_SHOWN_WIDTH_PX's own comment in geometry.ts for why this is a
// floor, not a guess. Every WCAG 2.5.8-flavored assertion below converts
// through this, not through VIEW_SIZE directly (a viewBox unit is not a px).
const WORST_CASE_SCALE = SVG_MIN_SHOWN_WIDTH_PX / VIEW_SIZE;
const WCAG_MIN_TARGET_PX = 24;

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

// Two independent WCAG 2.5.8-flavored guarantees RadarSvg.tsx's comment
// claims — kept as tests, not just prose, so a future change to HIT_RADIUS,
// SVG_MIN_SHOWN_WIDTH_PX, RADIUS_FRACTIONS, or WEDGE_PAD_DEG fails loudly
// here instead of silently, matching what the tech-radar staleness argument
// asks of everything else on this page.
describe('target size at the narrowest width the canvas is shown', () => {
  it('every blip clears the WCAG 2.5.8 24x24px floor on its own, independent of neighbors', () => {
    const hitDiameterPx = HIT_RADIUS * 2 * WORST_CASE_SCALE;
    expect(hitDiameterPx).toBeGreaterThanOrEqual(WCAG_MIN_TARGET_PX);
  });

  it("clears the floor with real margin, not by a rounding hair (today's constants, not a fluke)", () => {
    const hitDiameterPx = HIT_RADIUS * 2 * WORST_CASE_SCALE;
    expect(hitDiameterPx).toBeGreaterThanOrEqual(WCAG_MIN_TARGET_PX + 4);
  });
});

describe('adjacent-blip spacing in a crowded ring+quadrant cell', () => {
  it("the real radar's full layout — every cell AND every quadrant boundary — keeps every blip individually tappable", () => {
    // Runs layoutBlips against ALL 40 real entries at once (not one cell in
    // isolation): that's what actually caught the regression this test
    // guards against — two ADJACENT quadrants at the same ring (renderer-ui
    // and backend-data, both ring: 'adopt') placing their nearest blips
    // close enough across the quadrant boundary to collide, which no
    // single-cell check would ever see. Fails loudly the day a cell (or a
    // boundary pairing) grows past what WEDGE_PAD_DEG/RADIUS_FRACTIONS can
    // support — a human has to look and retune, not silently ship
    // overlapping targets.
    const positions = layoutBlips(RADAR);
    const distancePx = minPairwiseDistance(positions) * WORST_CASE_SCALE;
    expect(distancePx).toBeGreaterThanOrEqual(WCAG_MIN_TARGET_PX);
  });

  it("holds for a synthetic cell at today's real max size (10) — the layout algorithm itself, not just current data volume", () => {
    // NOT 12: numerically verified (see the PR that added this comment) that
    // 12 same-cell entries in the Adopt ring — the worst ring, smallest
    // inner radius — cannot clear the 24px floor with ANY (pad, lane-count,
    // fraction-spread) this design's numeric search found, even in
    // isolation with no adjacent-quadrant interference. 11 clears with a
    // ~0.7px margin; 10 (today's actual max, build-ship-trust/adopt) clears
    // with ~2px. This test pins the achievable ceiling instead of asserting
    // a number the geometry can't actually deliver — a check that can't
    // pass is worse than no check.
    const entries = Array.from({ length: 10 }, (_, i) =>
      entry({ id: `synthetic-${i}`, ring: 'adopt', quadrant: 'build-ship-trust' })
    );
    const distancePx = minPairwiseDistance(layoutBlips(entries)) * WORST_CASE_SCALE;
    expect(distancePx).toBeGreaterThanOrEqual(WCAG_MIN_TARGET_PX);
  });

  it('two adjacent quadrants at the same ring stay apart across the boundary (the regression this file caught)', () => {
    const entries = [
      entry({ id: 'near-boundary-a', ring: 'adopt', quadrant: 'renderer-ui' }),
      entry({ id: 'near-boundary-b', ring: 'adopt', quadrant: 'backend-data' }),
    ];
    const distancePx = minPairwiseDistance(layoutBlips(entries)) * WORST_CASE_SCALE;
    expect(distancePx).toBeGreaterThanOrEqual(WCAG_MIN_TARGET_PX);
  });

  it("Adopt is the worst ring (smallest inner radius) — the synthetic cases above must use it, or they're not testing the worst case", () => {
    const [adoptInner] = RING_BANDS.adopt;
    for (const ring of ['trial', 'assess', 'hold'] as const) {
      expect(RING_BANDS[ring][0]).toBeGreaterThan(adoptInner);
    }
  });
});
