import { describe, expect, it } from 'vitest';

import type { MapNode } from '@/data/architecture-map';

import {
  CANVAS,
  edgePath,
  ensureVisible,
  fit,
  screenToVB,
  stageCenter,
  type Viewport,
  ZOOM_MAX,
  ZOOM_MIN,
  zoomAt,
} from './geometry';

function node(overrides: Partial<MapNode> = {}): MapNode {
  return {
    id: 'n',
    cluster: 'client',
    label: 'Node',
    sub: '',
    x: 0,
    y: 0,
    w: 100,
    h: 40,
    color: 'client',
    role: '',
    plain: '',
    path: '',
    notes: [],
    tag: [],
    ...overrides,
  };
}

/** Parses `M sx sy C c1x c1y, c2x c2y, ex ey` into its numeric parts. */
function parseD(d: string) {
  const match =
    /^M (-?[\d.]+) (-?[\d.]+) C (-?[\d.]+) (-?[\d.]+), (-?[\d.]+) (-?[\d.]+), (-?[\d.]+) (-?[\d.]+)$/.exec(
      d
    );
  if (!match) throw new Error(`unparseable path d: ${d}`);
  const [, sx, sy, c1x, c1y, c2x, c2y, ex, ey] = match;
  return {
    sx: Number(sx),
    sy: Number(sy),
    c1x: Number(c1x),
    c1y: Number(c1y),
    c2x: Number(c2x),
    c2y: Number(c2y),
    ex: Number(ex),
    ey: Number(ey),
  };
}

describe('edgePath', () => {
  it("exits at a's right-middle edge and enters at b's left-middle edge when b is to the right", () => {
    const a = node({ x: 0, y: 0, w: 200, h: 80 });
    const b = node({ x: 500, y: 100, w: 200, h: 60 });
    const { d, mx, my } = edgePath(a, b);
    const { sx, sy, ex, ey, c1y, c2y } = parseD(d);
    expect(sx).toBe(a.x + a.w);
    expect(sy).toBe(a.y + a.h / 2);
    expect(ex).toBe(b.x);
    expect(ey).toBe(b.y + b.h / 2);
    // it's a cubic that exits/enters horizontally
    expect(c1y).toBe(sy);
    expect(c2y).toBe(ey);
    expect(mx).toBe((sx + ex) / 2);
    expect(my).toBe((sy + ey) / 2);
  });

  it("routes a return edge (b to the left of a) out of a's left edge and into b's right edge", () => {
    const a = node({ x: 500, y: 0, w: 200, h: 80 });
    const b = node({ x: 0, y: 100, w: 200, h: 60 });
    const { sx, ex } = parseD(edgePath(a, b).d);
    expect(sx).toBe(a.x); // a's LEFT edge, not a.x + a.w
    expect(ex).toBe(b.x + b.w); // b's RIGHT edge, not b.x
  });

  it('degenerates to a single point for coincident zero-size rects', () => {
    const a = node({ x: 10, y: 10, w: 0, h: 0 });
    const { sx, sy, ex, ey } = parseD(edgePath(a, a).d);
    expect(sx).toBe(ex);
    expect(sy).toBe(ey);
  });

  it('enforces a minimum 40px control-point spread even for adjacent nodes', () => {
    const a = node({ x: 0, y: 0, w: 100, h: 40 }); // right edge at x=100
    const b = node({ x: 101, y: 0, w: 100, h: 40 }); // left edge at x=101, 1px gap
    const { sx, c1x } = parseD(edgePath(a, b).d);
    expect(Math.abs(c1x - sx)).toBe(40);
  });
});

describe('fit', () => {
  it('scales by the width ratio when width is the constraining dimension', () => {
    // CANVAS is 3060x1330 (an ~2.3:1 aspect). A tall/narrow rect is width-constrained.
    const vp = fit({ width: 1000, height: 2000 });
    const byWidth = (1000 / CANVAS.w) * 0.98;
    expect(vp.scale).toBeCloseTo(byWidth, 10);
  });

  it('scales by the height ratio when height is the constraining dimension', () => {
    // A short/wide rect is height-constrained.
    const vp = fit({ width: 5000, height: 500 });
    const byHeight = (500 / CANVAS.h) * 0.98;
    expect(vp.scale).toBeCloseTo(byHeight, 10);
  });

  it('centers CANVAS within the given rect, leaving ~2% margin on the constrained axis', () => {
    const rect = { width: 1000, height: 2000 };
    const vp = fit(rect);
    expect(vp.tx).toBe((rect.width - CANVAS.w * vp.scale) / 2);
    expect(vp.ty).toBe((rect.height - CANVAS.h * vp.scale) / 2);
    // the constrained dimension (width, here) fits within a whisker of the rect
    expect(CANVAS.w * vp.scale).toBeLessThanOrEqual(rect.width);
    expect(CANVAS.w * vp.scale).toBeGreaterThan(rect.width * 0.97);
  });

  it('does not throw on a zero-size rect', () => {
    const vp = fit({ width: 0, height: 0 });
    expect(vp.scale).toBe(0);
    expect(vp.tx).toBe(0);
    expect(vp.ty).toBe(0);
  });
});

describe('screenToVB', () => {
  it('subtracts the containing rect origin from the screen point', () => {
    expect(screenToVB({ left: 10, top: 20 }, 15, 25)).toEqual({ x: 5, y: 5 });
  });

  it("composed with the viewport's inverse transform, recovers the original canvas point", () => {
    const rect = { left: 40, top: 12 };
    const vp: Viewport = { tx: -120, ty: 30, scale: 1.6 };
    const canvasPoint = { x: 900, y: 250 };

    // Forward: canvas -> local stage coords (as the SVG's translate/scale would render it)
    const localX = canvasPoint.x * vp.scale + vp.tx;
    const localY = canvasPoint.y * vp.scale + vp.ty;
    // ...and local -> screen, as the browser would report it in a pointer event
    const screenPoint = { x: rect.left + localX, y: rect.top + localY };

    const local = screenToVB(rect, screenPoint.x, screenPoint.y);
    expect(local).toEqual({ x: localX, y: localY });

    // Inverse: local stage coords -> canvas coords
    const recovered = { x: (local.x - vp.tx) / vp.scale, y: (local.y - vp.ty) / vp.scale };
    expect(recovered.x).toBeCloseTo(canvasPoint.x, 10);
    expect(recovered.y).toBeCloseTo(canvasPoint.y, 10);
  });
});

describe('stageCenter', () => {
  it.each([
    [{ width: 800, height: 600 }, [400, 300]],
    [{ width: 0, height: 0 }, [0, 0]],
    [{ width: 1, height: 1 }, [0.5, 0.5]],
  ] as const)('stageCenter(%j) === %j', (rect, expected) => {
    expect(stageCenter(rect)).toEqual(expected);
  });
});

describe('zoomAt', () => {
  const base: Viewport = { tx: 0, ty: 0, scale: 1 };

  it.each([
    [1, 2, 2, 'mid-range zoom-in is unclamped'],
    [4, 1, ZOOM_MAX, 'already at max, factor 1 stays at max'],
    [4, 2, ZOOM_MAX, 'zooming in past max clamps to ZOOM_MAX'],
    [3, 3, ZOOM_MAX, 'a factor that would overshoot max clamps to it exactly'],
    [ZOOM_MIN, 1, ZOOM_MIN, 'already at min, factor 1 stays at min'],
    [0.2, 0.1, ZOOM_MIN, 'zooming out past min clamps to ZOOM_MIN'],
    [ZOOM_MIN, 0.5, ZOOM_MIN, 'a factor that would undershoot min clamps to it exactly'],
  ] as const)(
    'scale=%d, factor=%d -> next.scale === %d (%s)',
    (scale, factor, expectedScale, _why) => {
      const next = zoomAt({ ...base, scale }, 100, 50, factor);
      expect(next.scale).toBe(expectedScale);
    }
  );

  it('keeps the focal point fixed under the transform (standard zoom-at-cursor math)', () => {
    const vp: Viewport = { tx: 0, ty: 0, scale: 1 };
    const next = zoomAt(vp, 100, 50, 2);
    expect(next.scale).toBe(2);
    const k = next.scale / vp.scale;
    expect(next.tx).toBe(100 - (100 - vp.tx) * k);
    expect(next.ty).toBe(50 - (50 - vp.ty) * k);
  });

  it('recomputes tx/ty from the clamped scale, not the raw requested one', () => {
    const vp: Viewport = { tx: 10, ty: -10, scale: 3 };
    const next = zoomAt(vp, 0, 0, 3); // raw = 9, clamped to ZOOM_MAX (4)
    const k = ZOOM_MAX / vp.scale;
    expect(next.scale).toBe(ZOOM_MAX);
    expect(next.tx).toBe(0 - (0 - vp.tx) * k);
    expect(next.ty).toBe(0 - (0 - vp.ty) * k);
  });
});

describe('ensureVisible', () => {
  const stage = { width: 1000, height: 800 };

  it('returns the same viewport instance (no-op) when the node is already comfortably visible', () => {
    const vp: Viewport = { tx: 0, ty: 0, scale: 1 };
    const n = node({ x: 400, y: 300, w: 50, h: 50 }); // center at (425, 325), well within margins
    expect(ensureVisible(vp, stage, n)).toBe(vp);
  });

  it('pans right when the node center is left of the margin', () => {
    const vp: Viewport = { tx: 0, ty: 0, scale: 1 };
    const n = node({ x: -100, y: 300, w: 20, h: 20 }); // center x = -90, left of the 90px margin
    const next = ensureVisible(vp, stage, n);
    expect(next.tx).toBeGreaterThan(vp.tx);
    expect(next.ty).toBe(vp.ty);
  });

  it('pans left when the node center is right of the margin', () => {
    const vp: Viewport = { tx: 0, ty: 0, scale: 1 };
    const n = node({ x: 950, y: 300, w: 20, h: 20 }); // center x = 960, past width(1000)-margin(90)=910
    const next = ensureVisible(vp, stage, n);
    expect(next.tx).toBeLessThan(vp.tx);
  });

  it('pans on both axes when the node is off both a horizontal and vertical edge', () => {
    const vp: Viewport = { tx: 0, ty: 0, scale: 1 };
    const n = node({ x: -100, y: -100, w: 20, h: 20 });
    const next = ensureVisible(vp, stage, n);
    expect(next.tx).toBeGreaterThan(vp.tx);
    expect(next.ty).toBeGreaterThan(vp.ty);
  });
});
