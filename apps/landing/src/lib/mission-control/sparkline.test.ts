import { describe, expect, it } from 'vitest';

import { sparklineGeometry } from './sparkline';

describe('sparklineGeometry', () => {
  it('returns null for an empty series', () => {
    expect(sparklineGeometry([], 120, 32)).toBeNull();
  });

  it('centers a single point and draws a valid move command', () => {
    const g = sparklineGeometry([5], 120, 32);
    expect(g).not.toBeNull();
    expect(g?.points).toHaveLength(1);
    expect(g?.points[0]?.x).toBe(60);
    expect(g?.line.startsWith('M')).toBe(true);
  });

  it('spreads N points across the full width, left to right', () => {
    const g = sparklineGeometry([1, 2, 3, 4, 5], 120, 32);
    expect(g?.points).toHaveLength(5);
    expect(g?.points[0]?.x).toBe(0);
    expect(g?.points.at(-1)?.x).toBe(120);
    // Rising series ⇒ y strictly decreases (higher value = higher on screen).
    const ys = g?.points.map((p) => p.y) ?? [];
    for (let i = 1; i < ys.length; i += 1) {
      expect(ys[i]).toBeLessThan(ys[i - 1] ?? Infinity);
    }
  });

  it('keeps a flat series on a single horizontal line (no divide-by-zero)', () => {
    const g = sparklineGeometry([7, 7, 7], 100, 40);
    const ys = g?.points.map((p) => p.y) ?? [];
    expect(new Set(ys).size).toBe(1);
    expect(Number.isFinite(ys[0])).toBe(true);
  });

  it('closes the area path back down to the baseline', () => {
    const g = sparklineGeometry([2, 8], 50, 20);
    expect(g?.area.endsWith('Z')).toBe(true);
    expect(g?.area).toContain(' L');
  });
});
