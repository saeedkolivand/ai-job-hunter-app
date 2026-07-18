import type { BufferGeometry } from 'three';
import { describe, expect, it } from 'vitest';

import { DEFAULT_CORNER_TEAR, presplitCornerTear } from './presplit';

// Grid resolution is a fixed contract of the builder (96 x 128 segments, 2
// triangles per cell). Kept in sync with COLS/ROWS in presplit.ts.
const GRID_TRIANGLES = 96 * 128 * 2;

const ATTR_NAMES = [
  'position',
  'normal',
  'uv',
  'aSeamDist',
  'aSeamArc',
  'aEdgeFlag',
  'aNoise',
] as const;

function defined<T>(value: T | null | undefined, what: string): T {
  if (value === null || value === undefined) throw new Error(`missing ${what}`);
  return value;
}

function attr(geo: BufferGeometry, name: string) {
  return defined(geo.getAttribute(name), name);
}

function indexArray(geo: BufferGeometry): ArrayLike<number> {
  return defined(geo.getIndex(), 'index').array;
}

// -1 = identical, -2 = length mismatch, else the first differing element index.
function firstDiff(a: ArrayLike<number>, b: ArrayLike<number>): number {
  if (a.length !== b.length) return -2;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return i;
  }
  return -1;
}

function edgePoints(geo: BufferGeometry): Array<[number, number, number]> {
  const flag = attr(geo, 'aEdgeFlag');
  const pos = attr(geo, 'position');
  const out: Array<[number, number, number]> = [];
  for (let i = 0; i < flag.count; i++) {
    if (flag.getX(i) === 1) {
      const p: [number, number, number] = [pos.getX(i), pos.getY(i), pos.getZ(i)];
      out.push(p);
    }
  }
  return out;
}

function triangleCount(geo: BufferGeometry): number {
  return defined(geo.getIndex(), 'index').count / 3;
}

// Smallest triangle area across the whole piece (0 => a degenerate triangle).
// Positions are planar (z=0), so the 2D shoelace magnitude is the area.
function minTriangleArea(geo: BufferGeometry): number {
  const idx = defined(geo.getIndex(), 'index');
  const pos = attr(geo, 'position');
  let min = Infinity;
  for (let t = 0; t + 2 < idx.count; t += 3) {
    const a = idx.getX(t);
    const b = idx.getX(t + 1);
    const c = idx.getX(t + 2);
    const ax = pos.getX(a);
    const ay = pos.getY(a);
    const abx = pos.getX(b) - ax;
    const aby = pos.getY(b) - ay;
    const acx = pos.getX(c) - ax;
    const acy = pos.getY(c) - ay;
    const area = Math.abs(abx * acy - acx * aby) / 2;
    if (area < min) min = area;
  }
  return min;
}

function attrRange(geo: BufferGeometry, name: string): { min: number; max: number } {
  const a = attr(geo, name);
  let min = Infinity;
  let max = -Infinity;
  for (let i = 0; i < a.count; i++) {
    const v = a.getX(i);
    if (v < min) min = v;
    if (v > max) max = v;
  }
  return { min, max };
}

describe('presplitCornerTear', () => {
  describe('determinism', () => {
    it('produces byte-identical geometry for the same seed', () => {
      const a = presplitCornerTear(7);
      const b = presplitCornerTear(7);
      for (const name of ATTR_NAMES) {
        expect(firstDiff(attr(a.held, name).array, attr(b.held, name).array)).toBe(-1);
        expect(firstDiff(attr(a.free, name).array, attr(b.free, name).array)).toBe(-1);
      }
      expect(firstDiff(indexArray(a.held), indexArray(b.held))).toBe(-1);
      expect(firstDiff(indexArray(a.free), indexArray(b.free))).toBe(-1);
    });

    it('produces different geometry for a different seed (seed is actually used)', () => {
      const a = presplitCornerTear(7);
      const c = presplitCornerTear(99);
      // At least one of the seam-driven arrays must differ.
      const posDiff = firstDiff(attr(a.held, 'position').array, attr(c.held, 'position').array);
      expect(posDiff).not.toBe(-1);
    });
  });

  describe('watertightness', () => {
    it('every held torn-edge vertex has a positionally-identical free partner', () => {
      const split = presplitCornerTear(1);
      const held = edgePoints(split.held);
      const free = edgePoints(split.free);

      expect(held.length).toBeGreaterThan(0);
      // Both pieces duplicate the same snapped seam vertices.
      expect(held.length).toBe(free.length);

      const eps = 1e-5;
      for (const h of held) {
        const matched = free.some(
          (f) => Math.hypot(f[0] - h[0], f[1] - h[1], f[2] - h[2]) <= eps,
        );
        expect(matched).toBe(true);
      }
    });
  });

  describe('completeness', () => {
    it('held + free triangles account for the whole grid', () => {
      const split = presplitCornerTear(1);
      const held = triangleCount(split.held);
      const free = triangleCount(split.free);
      expect(held).toBeGreaterThan(0);
      expect(free).toBeGreaterThan(0);
      expect(held + free).toBe(GRID_TRIANGLES);
    });

    it('contains no degenerate (zero-area) triangles', () => {
      const split = presplitCornerTear(1);
      expect(minTriangleArea(split.held)).toBeGreaterThan(1e-9);
      expect(minTriangleArea(split.free)).toBeGreaterThan(1e-9);
    });
  });

  describe('non-default style', () => {
    it('splits watertight for a straight-chord (jag=0) style and differs from default', () => {
      const style = { ...DEFAULT_CORNER_TEAR, jag: 0 };
      const split = presplitCornerTear(1, style);

      // Still watertight: every torn-edge held vertex has a free partner.
      const held = edgePoints(split.held);
      const free = edgePoints(split.free);
      expect(held.length).toBeGreaterThan(0);
      expect(held.length).toBe(free.length);
      const eps = 1e-5;
      for (const h of held) {
        const matched = free.some(
          (f) => Math.hypot(f[0] - h[0], f[1] - h[1], f[2] - h[2]) <= eps,
        );
        expect(matched).toBe(true);
      }

      // Still complete: every grid triangle assigned to exactly one piece.
      expect(triangleCount(split.held) + triangleCount(split.free)).toBe(GRID_TRIANGLES);

      // A different style yields a different seam -> different geometry.
      const dflt = presplitCornerTear(1);
      expect(
        firstDiff(attr(split.held, 'position').array, attr(dflt.held, 'position').array),
      ).not.toBe(-1);
    });
  });

  describe('attribute sanity', () => {
    it('aSeamArc stays within [0,1] on both pieces', () => {
      const split = presplitCornerTear(1);
      for (const geo of [split.held, split.free]) {
        const { min, max } = attrRange(geo, 'aSeamArc');
        expect(min).toBeGreaterThanOrEqual(-1e-6);
        expect(max).toBeLessThanOrEqual(1 + 1e-6);
      }
    });

    it('aNoise stays within [0,1)', () => {
      const split = presplitCornerTear(1);
      for (const geo of [split.held, split.free]) {
        const { min, max } = attrRange(geo, 'aNoise');
        expect(min).toBeGreaterThanOrEqual(0);
        expect(max).toBeLessThan(1);
      }
    });

    it('aSeamDist sign matches the piece: held >= 0, free <= 0', () => {
      const split = presplitCornerTear(1);
      const held = attrRange(split.held, 'aSeamDist');
      const free = attrRange(split.free, 'aSeamDist');
      // Held vertices sit on the positive (kept) side; free on the negative side;
      // snapped seam vertices are ~0. Allow a tiny epsilon for float noise.
      expect(held.min).toBeGreaterThanOrEqual(-1e-6);
      expect(free.max).toBeLessThanOrEqual(1e-6);
      // Each piece actually carries seam distance (not collapsed to all zeros).
      expect(held.max).toBeGreaterThan(0);
      expect(free.min).toBeLessThan(0);
    });
  });
});
